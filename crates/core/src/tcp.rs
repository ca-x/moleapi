use crate::{Environment, RequestSpec};
use anyhow::{Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use url::Url;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct TcpMessage {
    pub encoding: String,
    pub payload_source: String,
    pub secret: bool,
}
impl Default for TcpMessage {
    fn default() -> Self {
        Self {
            encoding: "text".into(),
            payload_source: String::new(),
            secret: false,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct TcpConfig {
    pub framing: String,
    pub max_frame_bytes: usize,
    pub no_delay: bool,
    pub idle_timeout_ms: u64,
    pub message: TcpMessage,
}
impl Default for TcpConfig {
    fn default() -> Self {
        Self {
            framing: "raw".into(),
            max_frame_bytes: 1024 * 1024,
            no_delay: true,
            idle_timeout_ms: 0,
            message: TcpMessage::default(),
        }
    }
}
pub fn tcp_url(raw: &str) -> Result<Url> {
    let url = Url::parse(raw)?;
    ensure!(
        matches!(url.scheme(), "tcp" | "tcps")
            && url.host_str().is_some()
            && url.port().is_some_and(|p| p != 0),
        "TCP requires tcp:// or tcps:// and an explicit nonzero port"
    );
    ensure!(
        url.username().is_empty()
            && url.password().is_none()
            && matches!(url.path(), "" | "/")
            && url.query().is_none()
            && url.fragment().is_none(),
        "TCP endpoints cannot contain credentials, path, query or fragment"
    );
    Ok(url)
}
pub fn tcp_payload(message: &TcpMessage) -> Result<Vec<u8>> {
    ensure!(
        message.payload_source.len() <= 2 * 1024 * 1024,
        "TCP payload source exceeds limit"
    );
    let bytes = match message.encoding.as_str() {
        "text" => message.payload_source.as_bytes().to_vec(),
        "base64" => STANDARD
            .decode(&message.payload_source)
            .map_err(|_| anyhow::anyhow!("Invalid TCP Base64 payload"))?,
        "hex" => hex::decode(
            message
                .payload_source
                .split_whitespace()
                .collect::<String>(),
        )
        .map_err(|_| anyhow::anyhow!("Invalid TCP Hex payload"))?,
        _ => anyhow::bail!("Unsupported TCP payload encoding"),
    };
    ensure!(bytes.len() <= 1024 * 1024, "TCP payload exceeds 1 MiB");
    Ok(bytes)
}
pub fn resolve_tcp_message(message: &TcpMessage, environment: &Environment) -> Result<TcpMessage> {
    let mut resolved = message.clone();
    resolved.payload_source = crate::resolve_mqtt_source(&message.payload_source, environment)?;
    Ok(resolved)
}
pub fn validate_tcp(request: &RequestSpec, draft: bool) -> Result<()> {
    let crate::Protocol::Tcp { config } = &request.protocol else {
        return Ok(());
    };
    ensure!(
        matches!(
            config.framing.as_str(),
            "raw" | "lines" | "length_be" | "length_le"
        ),
        "Unsupported TCP framing"
    );
    ensure!(
        (1..=1024 * 1024).contains(&config.max_frame_bytes),
        "TCP frame limit must be 1..1048576 bytes"
    );
    ensure!(
        config.idle_timeout_ms == 0 || (100..=120000).contains(&config.idle_timeout_ms),
        "TCP idle timeout must be 0 or 100..120000 ms"
    );
    ensure!(
        matches!(config.message.encoding.as_str(), "text" | "hex" | "base64")
            && config.message.payload_source.len() <= 2 * 1024 * 1024,
        "Invalid TCP payload draft"
    );
    if !draft {
        tcp_url(&request.url)?;
        ensure!(
            request.method == "GET" && request.body_kind == "none",
            "TCP connection envelope requires GET/None"
        );
        ensure!(
            request.auth.kind == "none"
                && request.headers.iter().all(|p| !p.enabled)
                && request.query.iter().all(|p| !p.enabled),
            "TCP does not use HTTP auth, headers or query rows"
        );
        ensure!(
            request.post_response_script.trim().is_empty(),
            "TCP post/event scripts are not supported"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tcp_drafts_keep_literal_braces_and_invoke_only_payload_variables() {
        let source = "{\"literal\":\"}}\",\"value\":\"{{secret}}\"}";
        let request:RequestSpec=serde_json::from_value(serde_json::json!({"protocol":{"kind":"tcp","message":{"payload_source":source}},"id":"r","name":"TCP","method":"GET","url":"tcp://{{host}}:9876","description":"","query":[],"headers":[],"body_kind":"none","body":"{\"unused\":{\"nested\":true}}","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":false,"verify_tls":true,"assertions":[],"examples":[]})).unwrap();
        let environment:Environment=serde_json::from_value(serde_json::json!({"id":"e","name":"env","variables":[{"id":"h","key":"host","value":"127.0.0.1","enabled":true},{"id":"s","key":"secret","value":"invoked","enabled":true,"secret":true}]})).unwrap();
        let resolved = crate::resolve_request(&request, Some(&environment)).unwrap();
        assert_eq!(resolved.url, "tcp://127.0.0.1:9876");
        assert_eq!(resolved.body, request.body);
        let crate::Protocol::Tcp { config } = resolved.protocol else {
            panic!()
        };
        assert_eq!(config.message.payload_source, source);
        assert_eq!(
            resolve_tcp_message(&config.message, &environment)
                .unwrap()
                .payload_source,
            "{\"literal\":\"}}\",\"value\":\"invoked\"}"
        );
    }
    #[test]
    fn tcp_url_encoding_and_config_boundaries_are_explicit() {
        for url in [
            "http://localhost:1",
            "tcp://localhost",
            "tcp://u:p@localhost:1",
            "tcp://localhost:1/path",
            "tcp://localhost:1?q=x",
            "tcp://localhost:0",
        ] {
            assert!(tcp_url(url).is_err());
        }
        assert!(tcp_url("tcps://[::1]:1234").is_ok());
        assert_eq!(
            tcp_payload(&TcpMessage {
                encoding: "hex".into(),
                payload_source: "00 01 ff 41".into(),
                secret: false
            })
            .unwrap(),
            [0, 1, 255, 65]
        );
        assert_eq!(
            tcp_payload(&TcpMessage {
                encoding: "base64".into(),
                payload_source: "AAH/QQ==".into(),
                secret: false
            })
            .unwrap(),
            [0, 1, 255, 65]
        );
        assert!(
            tcp_payload(&TcpMessage {
                encoding: "hex".into(),
                payload_source: "f".into(),
                secret: false
            })
            .unwrap_err()
            .to_string()
            .contains("Invalid TCP Hex")
        );
    }
}
