//! Private session cookies. Parsing, expiry and URL matching belong to cookie_store.
use anyhow::{Result, ensure};
use cookie_store::{Cookie, CookieStore};
use serde::Serialize;
use std::sync::Mutex;
use url::Url;

const MAX_COOKIES: usize = 512;
const MAX_COOKIE: usize = 8192;
const MAX_TOTAL: usize = 256 * 1024;
#[derive(Default)]
struct State {
    store: CookieStore,
    enabled: bool,
    generation: u64,
}
#[derive(Default)]
pub struct CookieJar(Mutex<State>);
#[derive(Serialize)]
pub struct CookieEntry {
    pub domain: String,
    pub path: String,
    pub name: String,
    pub value: String,
    pub host_only: bool,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: Option<String>,
    pub expires: String,
}
#[derive(Serialize)]
pub struct CookieSnapshot {
    pub enabled: bool,
    pub cookies: Vec<CookieEntry>,
}
impl CookieJar {
    pub fn generation(&self) -> u64 {
        self.0.lock().unwrap().generation
    }
    /// Clear/disable fences responses from requests already in flight.
    pub fn configure(&self, enabled: bool, clear: bool) {
        let mut state = self.0.lock().unwrap();
        state.generation += 1;
        state.enabled = enabled;
        if clear {
            state.store.clear();
        }
    }
    pub fn snapshot(&self, reveal: bool) -> CookieSnapshot {
        let state = self.0.lock().unwrap();
        let mut cookies: Vec<_> = state
            .store
            .iter_unexpired()
            .map(|c| CookieEntry {
                domain: String::from(&c.domain),
                path: String::from(&c.path),
                name: c.name().into(),
                value: if reveal {
                    c.value().into()
                } else {
                    "[REDACTED]".into()
                },
                host_only: matches!(c.domain, cookie_store::CookieDomain::HostOnly(_)),
                secure: c.secure().unwrap_or(false),
                http_only: c.http_only().unwrap_or(false),
                same_site: c.same_site().map(|v| v.to_string()),
                expires: format!("{:?}", c.expires),
            })
            .collect();
        cookies.sort_by(|a, b| (&a.domain, &a.path, &a.name).cmp(&(&b.domain, &b.path, &b.name)));
        CookieSnapshot {
            enabled: state.enabled,
            cookies,
        }
    }
    pub fn insert(&self, url: &Url, raw: &str) -> Result<()> {
        let mut state = self.0.lock().unwrap();
        insert(&mut state.store, url, raw)?;
        state.generation += 1;
        Ok(())
    }
    pub fn remove(&self, domain: &str, path: &str, name: &str) {
        let mut state = self.0.lock().unwrap();
        state.generation += 1;
        state.store.remove(domain, path, name);
    }
    pub(crate) fn header(&self, url: &Url, generation: u64) -> Option<String> {
        let state = self.0.lock().unwrap();
        if !state.enabled || state.generation != generation {
            return None;
        }
        let mut cookies = state.store.matches(url);
        // More specific paths precede less specific ones (RFC6265 section5.4).
        cookies.sort_by(|a, b| {
            b.path
                .len()
                .cmp(&a.path.len())
                .then_with(|| a.name().cmp(b.name()))
        });
        let value = cookies
            .into_iter()
            .map(|c| format!("{}={}", c.name(), c.value()))
            .collect::<Vec<_>>()
            .join("; ");
        (!value.is_empty()).then_some(value)
    }
    pub(crate) fn receive(&self, url: &Url, raw: &str, generation: u64) {
        let mut state = self.0.lock().unwrap();
        if state.enabled && state.generation == generation {
            let _ = insert(&mut state.store, url, raw);
        }
    }
}
fn insert(store: &mut CookieStore, url: &Url, raw: &str) -> Result<()> {
    ensure!(raw.len() <= MAX_COOKIE, "Cookie exceeds 8 KiB");
    let c = Cookie::parse(raw, url)
        .map_err(|_| anyhow::anyhow!("Invalid cookie"))?
        .into_owned();
    if c.domain().is_some() {
        let domain = String::from(&c.domain);
        ensure!(
            psl::suffix_str(&domain) != Some(domain.as_str()),
            "Cookie domain is a public suffix"
        );
    }
    ensure!(
        c.secure() != Some(true) || url.scheme() == "https",
        "Secure cookie requires HTTPS"
    );
    if c.name().starts_with("__Secure-") || c.name().starts_with("__Host-") {
        ensure!(
            c.secure() == Some(true) && url.scheme() == "https",
            "Cookie prefix requires Secure and HTTPS"
        );
    }
    if c.name().starts_with("__Host-") {
        ensure!(
            c.domain().is_none() && c.path() == Some("/"),
            "Host cookie requires Path=/ and no Domain"
        );
    }
    ensure!(
        reqwest::header::HeaderValue::from_str(&format!("{}={}", c.name(), c.value())).is_ok(),
        "Invalid cookie header value"
    );
    let mut secure_url = url.clone();
    secure_url
        .set_scheme("https")
        .map_err(|_| anyhow::anyhow!("Invalid cookie URL"))?;
    ensure!(
        url.scheme() == "https"
            || !store.iter_unexpired().any(|old| old.secure() == Some(true)
                && old.name() == c.name()
                && old.domain.matches(&secure_url)),
        "HTTP cannot overwrite a secure cookie"
    );
    // Remove expired entries so attackers cannot accumulate expired names/paths indefinitely.
    let expired: Vec<_> = store
        .iter_any()
        .filter(|c| c.is_expired())
        .map(|c| {
            (
                String::from(&c.domain),
                String::from(&c.path),
                c.name().to_owned(),
            )
        })
        .collect();
    for (domain, path, name) in expired {
        store.remove(&domain, &path, &name);
    }
    let domain = String::from(&c.domain);
    let path = String::from(&c.path);
    let name = c.name().to_owned();
    let previous = store
        .get(&domain, &path, &name)
        .map(|c| c.clone().into_owned());
    if c.is_expired() {
        store.remove(&domain, &path, &name);
        return Ok(());
    }
    store
        .insert(c, url)
        .map_err(|_| anyhow::anyhow!("Cookie does not match URL"))?;
    let count = store.iter_unexpired().count();
    let bytes: usize = store
        .iter_unexpired()
        .map(|c| c.to_string().len() + c.domain.as_cow().map_or(0, |d| d.len()) + c.path.len())
        .sum();
    if count > MAX_COOKIES || bytes > MAX_TOTAL {
        store.remove(&domain, &path, &name);
        if let Some(previous) = previous {
            let _ = store.insert(previous, url);
        }
        anyhow::bail!("Cookie jar capacity reached (512 cookies / 256 KiB)");
    }
    Ok(())
}
