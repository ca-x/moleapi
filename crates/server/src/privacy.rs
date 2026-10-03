//! Private execution values are retained only in the live response.
use crate::ApiError;
use aho_corasick::{AhoCorasick, MatchKind};
use std::collections::BTreeSet;

pub(crate) struct HistoryPrivacy<'a> {
    pub values: &'a BTreeSet<String>,
    pub redact_failed_response: bool,
}

pub(crate) struct Redactor {
    matcher: Option<AhoCorasick>,
    withhold: bool,
}
const MAX_SECRET_BYTES: usize = 16 * 1024;
const MAX_SECRET_LENGTH: usize = 4096;
const MAX_PATTERN_BYTES: usize = 64 * 1024;
impl Redactor {
    pub fn new(secrets: &BTreeSet<String>) -> Result<Self, ApiError> {
        if secrets.iter().any(|value| value.len() > MAX_SECRET_LENGTH)
            || secrets.iter().map(String::len).sum::<usize>() > MAX_SECRET_BYTES
        {
            return Ok(Self {
                matcher: None,
                withhold: true,
            });
        }
        let mut patterns = BTreeSet::new();
        for secret in secrets.iter().filter(|s| !s.is_empty()) {
            patterns.insert(secret.clone());
            let form: String = url::form_urlencoded::byte_serialize(secret.as_bytes()).collect();
            patterns.insert(form.clone());
            // Query producers can use %20 rather than + for a space.
            patterns.insert(form.replace('+', "%20"));
            // Preserve privacy when endpoint JSON or URL paths escape a value.
            if let Ok(json) = serde_json::to_string(secret) {
                patterns.insert(json[1..json.len() - 1].to_owned());
            }
            if let Ok(mut url) = url::Url::parse("https://privacy.invalid/") {
                url.set_path(secret);
                patterns.insert(
                    url.path()
                        .strip_prefix('/')
                        .unwrap_or(url.path())
                        .to_owned(),
                );
            }
        }
        let patterns: Vec<_> = patterns.into_iter().filter(|s| !s.is_empty()).collect();
        if patterns.is_empty() {
            return Ok(Self {
                matcher: None,
                withhold: false,
            });
        }
        if patterns.iter().map(String::len).sum::<usize>() > MAX_PATTERN_BYTES {
            return Ok(Self {
                matcher: None,
                withhold: true,
            });
        }
        let matcher = AhoCorasick::builder()
            .match_kind(MatchKind::LeftmostLongest)
            .build(patterns)
            .map_err(|_| ApiError::internal())?;
        Ok(Self {
            matcher: Some(matcher),
            withhold: false,
        })
    }
    pub fn withholds_text(&self) -> bool {
        self.withhold
    }
    pub fn scrub(&self, value: &mut serde_json::Value) {
        match value {
            serde_json::Value::String(text) => {
                if self.withhold {
                    *text = "[REDACTED: history privacy limit]".into();
                    return;
                }
                let Some(matcher) = &self.matcher else {
                    return;
                };
                let mut result = String::new();
                let mut offset = 0;
                for found in matcher.find_iter(text.as_bytes()) {
                    let prefix = &text[offset..found.start()];
                    if result.len().saturating_add(prefix.len()).saturating_add(10)
                        > moleapi_core::MAX_BODY
                    {
                        *text = "[REDACTED: history size limit]".into();
                        return;
                    }
                    result.push_str(prefix);
                    result.push_str("[REDACTED]");
                    offset = found.end();
                }
                if result.len().saturating_add(text.len() - offset) > moleapi_core::MAX_BODY {
                    *text = "[REDACTED: history size limit]".into();
                    return;
                }
                result.push_str(&text[offset..]);
                *text = result;
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    self.scrub(item);
                }
            }
            serde_json::Value::Object(items) => {
                for (key, item) in items {
                    if !matches!(key.as_str(), "id" | "workspace_id" | "request_id") {
                        self.scrub(item);
                    }
                }
            }
            _ => {}
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn json_and_url_escaping_cannot_leave_private_values_in_history() {
        let secret = "quote\" and space".to_owned();
        let redactor = Redactor::new(&BTreeSet::from([secret.clone(), "script".into()])).unwrap();
        let encoded = serde_json::to_string(&secret).unwrap();
        let mut url = url::Url::parse("https://example.com/").unwrap();
        url.set_path(&secret);
        let mut value =
            serde_json::json!({"workspace_id":"script", "body":encoded, "url":url.as_str()});
        redactor.scrub(&mut value);
        assert_eq!(value["workspace_id"], "script");
        assert_eq!(value["body"], "\"[REDACTED]\"");
        assert!(value["url"].as_str().unwrap().contains("[REDACTED]"));
    }
    #[test]
    fn oversized_overlapping_patterns_are_withheld_without_compiling_an_automaton() {
        let secrets = (0..4)
            .map(|index| format!("{}{index}", "a".repeat(1048500)))
            .collect();
        let start = std::time::Instant::now();
        let redactor = Redactor::new(&secrets).unwrap();
        assert!(start.elapsed() < std::time::Duration::from_millis(100));
        assert!(redactor.withholds_text());
        let mut value = serde_json::json!("possibly sensitive text");
        redactor.scrub(&mut value);
        assert_eq!(value, "[REDACTED: history privacy limit]");
    }
    #[test]
    fn overlapping_values_and_replacement_growth_are_bounded() {
        let redactor = Redactor::new(&BTreeSet::from(["a".into(), "abc".into()])).unwrap();
        let mut value = serde_json::json!("abc-a");
        redactor.scrub(&mut value);
        assert_eq!(value, "[REDACTED]-[REDACTED]");
        let mut value = serde_json::json!("a".repeat(moleapi_core::MAX_BODY));
        redactor.scrub(&mut value);
        assert_eq!(value, "[REDACTED: history size limit]");
    }
}
