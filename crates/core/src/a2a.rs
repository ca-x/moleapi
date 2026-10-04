use crate::{Protocol, RequestSpec};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct A2aConfig {
    pub dialect: String,
    pub transport: String,
    pub operation: String,
    pub params_source: String,
    pub card_source: Option<String>,
    pub interface_url: Option<String>,
}
impl Default for A2aConfig {
    fn default() -> Self {
        Self {
            dialect: "0.3".into(),
            transport: "jsonrpc".into(),
            operation: "message/send".into(),
            params_source: "{}".into(),
            card_source: None,
            interface_url: None,
        }
    }
}
pub fn validate_a2a(request: &RequestSpec, _draft: bool) -> Result<()> {
    let Protocol::A2a { config } = &request.protocol else {
        return Ok(());
    };
    ensure!(
        matches!(
            (config.dialect.as_str(), config.transport.as_str()),
            ("0.3", "jsonrpc") | ("1.0", "jsonrpc" | "http-json")
        ),
        "Supported A2A interfaces: 0.3 JSONRPC, 1.0 JSONRPC or HTTP+JSON"
    );
    ensure!(
        config.params_source.len() <= 1024 * 1024
            && config
                .card_source
                .as_ref()
                .is_none_or(|s| s.len() <= 1024 * 1024)
            && config.operation.len() <= 128
            && config
                .interface_url
                .as_ref()
                .is_none_or(|s| s.len() <= 8192),
        "A2A draft exceeds limits"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn preparation_preserves_raw_a2a_json_sources_and_only_resolves_transport_fields() {
        let params = r#"{"message":{"text":"{{message_only}}","data":{"nested":true}}}"#;
        let card = r#"{"name":"Literal }} card","extension":{"unresolved":"{{card_only}}"}}"#;
        let request: RequestSpec = serde_json::from_value(json!({"protocol":{"kind":"a2a","params_source":params,"card_source":card},"id":"r","name":"A2A","method":"GET","url":"{{base}}/card.json","description":"","query":[],"headers":[{"id":"h","key":"X-Test","value":"{{header}}","enabled":true}],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":false,"verify_tls":true,"assertions":[],"examples":[]})).unwrap();
        let environment: crate::Environment=serde_json::from_value(json!({"id":"e","name":"test","variables":[{"id":"base","key":"base","value":"https://example.test","enabled":true},{"id":"header","key":"header","value":"transport-value","enabled":true}]})).unwrap();
        let resolved = crate::resolve_request(&request, Some(&environment)).unwrap();
        assert_eq!(resolved.url, "https://example.test/card.json");
        assert_eq!(resolved.headers[0].value, "transport-value");
        let Protocol::A2a { config } = resolved.protocol else {
            panic!("A2A")
        };
        assert_eq!(config.params_source, params);
        assert_eq!(config.card_source.as_deref(), Some(card));
    }
}
