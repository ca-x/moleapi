use crate::Environment;
use url::Url;
pub fn redact_url(raw: &str, environment: Option<&Environment>) -> String {
    let Ok(mut url) = Url::parse(raw) else {
        return "[invalid URL]".into();
    };
    let secrets: Vec<&str> = environment
        .map(|e| {
            e.variables
                .iter()
                .filter(|v| v.enabled && (v.secret == Some(true) || v.local_value.is_some()))
                .flat_map(|v| {
                    [
                        v.value.as_str(),
                        v.local_value.as_deref().unwrap_or(&v.value),
                    ]
                })
                .filter(|value| !value.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let query: Vec<_> = url
        .query_pairs()
        .map(|(k, v)| {
            let sensitive = sensitive_query_key(&k) || secrets.iter().any(|s| v.contains(s));
            (
                k.into_owned(),
                if sensitive {
                    "[REDACTED]".into()
                } else {
                    v.into_owned()
                },
            )
        })
        .collect();
    if url.query().is_some() {
        url.set_query(None);
        let mut pairs = url.query_pairs_mut();
        for (k, v) in query {
            pairs.append_pair(&k, &v);
        }
    }
    url.to_string()
}

pub fn sensitive_query_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "key",
        "token",
        "secret",
        "password",
        "auth",
        "credential",
        "signature",
    ]
    .iter()
    .any(|part| key.contains(part))
}
