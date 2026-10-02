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
                .filter(|v| v.enabled && v.secret == Some(true) && !v.value.is_empty())
                .map(|v| v.value.as_str())
                .collect()
        })
        .unwrap_or_default();
    let query: Vec<_> = url
        .query_pairs()
        .map(|(k, v)| {
            let key = k.to_ascii_lowercase();
            let sensitive = [
                "key",
                "token",
                "secret",
                "password",
                "auth",
                "credential",
                "signature",
            ]
            .iter()
            .any(|s| key.contains(s))
                || secrets.iter().any(|s| v.contains(s));
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
