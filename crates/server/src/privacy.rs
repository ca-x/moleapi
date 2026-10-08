//! Private execution values are retained only in the live response.
use crate::ApiError;
use aho_corasick::{AhoCorasick, MatchKind};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
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
            patterns.insert(moleapi_core::escape_xml_value(secret));
            patterns.insert(STANDARD.encode(secret));
            patterns.insert(URL_SAFE_NO_PAD.encode(secret));
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
/// Retain raw and currently resolvable credentials before any script can remove them.
pub(crate) fn request_values(
    request: &moleapi_core::RequestSpec,
    scopes: &mut moleapi_core::VariableScopes,
) -> Result<(), ApiError> {
    let environment = scopes.effective();
    let mut capture = |value: &str| {
        if value.is_empty() {
            return;
        }
        scopes.private_values.insert(value.into());
        if let Ok(resolved) = moleapi_core::resolve_value(value, &environment)
            && !resolved.is_empty()
        {
            scopes.private_values.insert(resolved);
        }
    };
    if request.protocol.is_soap()
        && let Ok(doc) = moleapi_core::parse_bounded_xml(&request.body)
    {
        for n in doc.descendants().filter(|n| n.is_element()) {
            if moleapi_core::sensitive_query_key(n.tag_name().name()) {
                for text in n.descendants().filter_map(|n| n.text()) {
                    capture(text)
                }
            }
            for attr in n.attributes() {
                if moleapi_core::sensitive_query_key(attr.name()) {
                    capture(attr.value())
                }
            }
        }
    }
    if let moleapi_core::Protocol::Graphql {
        connection_params, ..
    } = &request.protocol
    {
        fn strings(value: &serde_json::Value, capture: &mut impl FnMut(&str)) {
            match value {
                serde_json::Value::String(s) => capture(s),
                serde_json::Value::Array(items) => {
                    for item in items {
                        strings(item, capture);
                    }
                }
                serde_json::Value::Object(items) => {
                    for item in items.values() {
                        strings(item, capture);
                    }
                }
                _ => {}
            }
        }
        strings(connection_params, &mut capture);
    }
    if let moleapi_core::Protocol::Socketio { auth_source, .. } = &request.protocol {
        fn capture_auth(value: &serde_json::Value, capture: &mut impl FnMut(&str)) {
            match value {
                serde_json::Value::String(text) => capture(text),
                serde_json::Value::Array(values) => {
                    for value in values {
                        capture_auth(value, capture);
                    }
                }
                serde_json::Value::Object(values) => {
                    for value in values.values() {
                        capture_auth(value, capture);
                    }
                }
                serde_json::Value::Number(number) => capture(&number.to_string()),
                _ => {}
            }
        }
        if let Ok(auth) = serde_json::from_str(auth_source) {
            capture_auth(&auth, &mut capture);
        }
    }
    if let moleapi_core::Protocol::Mqtt { config } = &request.protocol {
        let properties = |items: &[moleapi_core::MqttProperty], capture: &mut dyn FnMut(&str)| {
            for p in items {
                if p.secret {
                    capture(&p.value);
                }
            }
        };
        let message = |m: &moleapi_core::MqttMessage, capture: &mut dyn FnMut(&str)| {
            if m.topic_secret {
                capture(&m.topic);
            }
            if m.payload_secret {
                capture(&m.payload_source);
                if m.encoding == "base64"
                    && let Ok(bytes) = STANDARD.decode(&m.payload_source)
                    && let Ok(text) = std::str::from_utf8(&bytes)
                {
                    capture(text);
                }
            }
            properties(&m.properties.user_properties, capture);
        };
        properties(&config.user_properties, &mut capture);
        if let Some(will) = &config.will {
            message(&will.message, &mut capture);
        }
        for subscription in &config.subscriptions {
            if subscription.filter_secret {
                capture(&subscription.filter);
            }
            properties(&subscription.user_properties, &mut capture);
        }
    }
    if let moleapi_core::Protocol::Mcp { config } = &request.protocol {
        for pair in config.env.iter().filter(|pair| pair.enabled) {
            if pair.secret == Some(true) || moleapi_core::sensitive_query_key(&pair.key) {
                capture(&pair.value);
            }
        }
    }
    if let Some(key) = &request.auth.api_key {
        capture(&key.value);
    }
    if let Some(jwt) = &request.auth.jwt {
        capture(&jwt.key);
        if jwt.key_base64 {
            for key in std::iter::once(jwt.key.clone())
                .chain(moleapi_core::resolve_value(&jwt.key, &environment).ok())
            {
                if let Ok(bytes) = STANDARD.decode(&key)
                    && let Ok(text) = std::str::from_utf8(&bytes)
                {
                    capture(text);
                }
            }
        }
    }
    if let Some(aws) = &request.auth.aws {
        capture(&aws.access_key);
        capture(&aws.secret_key);
        capture(&aws.session_token);
    }
    if let Some(oauth) = &request.auth.oauth2 {
        capture(&oauth.client_secret);
        capture(&oauth.password);
        for row in oauth
            .token_params
            .iter()
            .chain(&oauth.authorization_params)
            .chain(&oauth.token_headers)
        {
            if row.secret == Some(true) || moleapi_core::sensitive_query_key(&row.key) {
                capture(&row.value);
            }
        }
    }
    capture(&request.auth.token);
    capture(&request.auth.password);
    if !request.auth.password.is_empty() {
        capture(&format!(
            "{}:{}",
            request.auth.username, request.auth.password
        ));
    }
    let sensitive_header = |key: &str| {
        matches!(
            key.to_ascii_lowercase().as_str(),
            "authorization" | "cookie" | "proxy-authorization" | "x-api-key"
        )
    };
    let authorization_header = |key: &str| {
        matches!(
            key.to_ascii_lowercase().as_str(),
            "authorization" | "proxy-authorization"
        )
    };
    for header in request.headers.iter().filter(|h| h.enabled) {
        let resolved_key = moleapi_core::resolve_value(&header.key, &environment).ok();
        if header.secret == Some(true)
            || sensitive_header(&header.key)
            || resolved_key.as_deref().is_some_and(sensitive_header)
        {
            capture(&header.value);
            if (authorization_header(&header.key)
                || resolved_key.as_deref().is_some_and(authorization_header))
                && let Some((_, credential)) = header.value.split_once(' ')
            {
                capture(credential.trim());
            }
        }
    }
    for query in request.query.iter().filter(|p| p.enabled) {
        let resolved_key = moleapi_core::resolve_value(&query.key, &environment).ok();
        if query.secret == Some(true)
            || moleapi_core::sensitive_query_key(&query.key)
            || resolved_key
                .as_deref()
                .is_some_and(moleapi_core::sensitive_query_key)
        {
            capture(&query.value);
        }
    }
    let subscription_url = match &request.protocol {
        moleapi_core::Protocol::Graphql {
            subscription_url, ..
        } => subscription_url.as_deref(),
        _ => None,
    };
    for target in std::iter::once(request.url.as_str()).chain(subscription_url) {
        let resolved_url = moleapi_core::resolve_value(target, &environment).ok();
        for raw in std::iter::once(target).chain(resolved_url.as_deref()) {
            if let Ok(url) = url::Url::parse(raw) {
                capture(url.username());
                if let Some(password) = url.password() {
                    capture(password);
                }
                for (key, value) in url.query_pairs() {
                    let resolved_key = moleapi_core::resolve_value(&key, &environment).ok();
                    if moleapi_core::sensitive_query_key(&key)
                        || resolved_key
                            .as_deref()
                            .is_some_and(moleapi_core::sensitive_query_key)
                    {
                        capture(&value);
                    }
                }
            }
        }
    }
    scopes.validate().map_err(|e| ApiError::bad(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encoded_jwt_signing_secret_and_resolved_source_are_private() {
        let request: moleapi_core::RequestSpec = serde_json::from_value(serde_json::json!({
            "id":"auth", "name":"auth", "method":"GET", "url":"https://example.test",
            "auth":{"kind":"jwt", "username":"", "password":"", "token":"", "jwt":{"key":"{{signing_key}}", "key_base64":true}},
            "query":[], "headers":[], "body":"", "body_kind":"none", "description":"",
            "timeout_ms":3000,"verify_tls":true,"follow_redirects":true,"assertions":[],"examples":[]
        })).unwrap();
        let mut scopes = moleapi_core::VariableScopes::default();
        scopes.environment.insert(
            "signing_key".into(),
            STANDARD.encode("decoded-private-signing-key"),
        );
        request_values(&request, &mut scopes).unwrap();
        assert!(
            scopes
                .private_values
                .contains("decoded-private-signing-key")
        );
        let redactor = Redactor::new(&scopes.private_values).unwrap();
        let mut history = serde_json::json!({"body":"decoded-private-signing-key", "copy":STANDARD.encode("decoded-private-signing-key")});
        redactor.scrub(&mut history);
        assert!(!history.to_string().contains("decoded-private-signing-key"));
        assert!(
            !history
                .to_string()
                .contains(&STANDARD.encode("decoded-private-signing-key"))
        );
    }
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
