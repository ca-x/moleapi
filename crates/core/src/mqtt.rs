//! Canonical editable MQTT drafts. rumqttc owns topic semantics and all wire packets.
use crate::{Environment, Protocol, RequestSpec, resolve_grpc_source};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use url::Url;
pub const MQTT_MAX_MESSAGE: usize = 1024 * 1024;
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct MqttProperty {
    pub key: String,
    pub value: String,
    pub secret: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct MqttPublishProperties {
    pub payload_format_indicator: Option<u8>,
    pub message_expiry_interval: Option<u32>,
    pub response_topic: Option<String>,
    pub correlation_data_base64: Option<String>,
    pub content_type: Option<String>,
    pub user_properties: Vec<MqttProperty>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct MqttMessage {
    pub topic: String,
    pub payload_source: String,
    pub encoding: String,
    pub qos: u8,
    pub retain: bool,
    pub topic_secret: bool,
    pub payload_secret: bool,
    pub properties: MqttPublishProperties,
}
impl Default for MqttMessage {
    fn default() -> Self {
        Self {
            topic: String::new(),
            payload_source: String::new(),
            encoding: "text".into(),
            qos: 0,
            retain: false,
            topic_secret: false,
            payload_secret: false,
            properties: MqttPublishProperties::default(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct MqttSubscription {
    pub filter: String,
    pub qos: u8,
    pub enabled: bool,
    pub filter_secret: bool,
    pub no_local: bool,
    pub retain_as_published: bool,
    pub retain_handling: u8,
    pub subscription_identifier: Option<usize>,
    pub user_properties: Vec<MqttProperty>,
}
impl Default for MqttSubscription {
    fn default() -> Self {
        Self {
            filter: String::new(),
            qos: 0,
            enabled: true,
            filter_secret: false,
            no_local: false,
            retain_as_published: false,
            retain_handling: 0,
            subscription_identifier: None,
            user_properties: vec![],
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct MqttWill {
    pub message: MqttMessage,
    pub delay_interval: u32,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct MqttSavedMessage {
    pub id: String,
    pub name: String,
    pub message: MqttMessage,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct MqttReconnect {
    pub enabled: bool,
    pub max_attempts: u8,
    pub delay_ms: u64,
}
impl Default for MqttReconnect {
    fn default() -> Self {
        Self {
            enabled: false,
            max_attempts: 3,
            delay_ms: 1000,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct MqttConfig {
    pub version: String,
    pub client_id: String,
    pub clean_start: bool,
    pub keep_alive_secs: u16,
    pub session_expiry_interval: u32,
    pub reconnect: MqttReconnect,
    pub subscriptions: Vec<MqttSubscription>,
    pub user_properties: Vec<MqttProperty>,
    pub will: Option<MqttWill>,
    pub message: MqttMessage,
    pub saved_messages: Vec<MqttSavedMessage>,
}
impl Default for MqttConfig {
    fn default() -> Self {
        Self {
            version: "5".into(),
            client_id: String::new(),
            clean_start: true,
            keep_alive_secs: 60,
            session_expiry_interval: 0,
            reconnect: MqttReconnect::default(),
            subscriptions: vec![],
            user_properties: vec![],
            will: None,
            message: MqttMessage::default(),
            saved_messages: vec![],
        }
    }
}
fn string(value: &str, max: usize) -> Result<()> {
    ensure!(
        value.len() <= max && !value.contains('\0'),
        "MQTT string exceeds limit or contains NUL"
    );
    Ok(())
}
fn properties(items: &[MqttProperty], execute: bool) -> Result<()> {
    ensure!(items.len() <= 32, "MQTT property limit reached (32)");
    ensure!(
        items
            .iter()
            .map(|p| p.key.len() + p.value.len())
            .sum::<usize>()
            <= 16 * 1024,
        "MQTT properties exceed 16 KiB"
    );
    for p in items {
        string(&p.key, 4096)?;
        string(&p.value, 4096)?;
        if execute {
            ensure!(!p.key.is_empty(), "MQTT property key is required");
        }
    }
    Ok(())
}
pub fn mqtt_url(raw: &str) -> Result<Url> {
    let mut url = Url::parse(raw).context("Invalid MQTT broker URL")?;
    let port = match url.scheme() {
        "mqtt" => 1883,
        "mqtts" => 8883,
        "ws" => 80,
        "wss" => 443,
        _ => anyhow::bail!("MQTT requires mqtt, mqtts, ws or wss broker URL"),
    };
    ensure!(
        url.host_str().is_some() && url.username().is_empty() && url.password().is_none(),
        "MQTT broker requires a host; use Basic authentication for credentials"
    );
    ensure!(
        url.fragment().is_none(),
        "MQTT broker fragments are unsupported"
    );
    if matches!(url.scheme(), "mqtt" | "mqtts") {
        ensure!(
            matches!(url.path(), "" | "/") && url.query().is_none(),
            "MQTT TCP broker cannot have a path or query"
        );
    }
    if url.port().is_none() {
        let _ = url.set_port(Some(port));
    }
    Ok(url)
}
pub fn validate_mqtt_filter(filter: &str) -> Result<()> {
    string(filter, 1024)?;
    ensure!(
        rumqttc::mqttbytes::valid_filter(filter),
        "Invalid MQTT subscription filter"
    );
    if let Some(shared) = filter.strip_prefix("$share/") {
        let (group, filter) = shared
            .split_once('/')
            .context("Invalid MQTT shared subscription")?;
        ensure!(
            !group.is_empty()
                && !group.contains(['+', '#'])
                && rumqttc::mqttbytes::valid_filter(filter),
            "Invalid MQTT shared subscription"
        );
    }
    Ok(())
}
pub fn validate_mqtt_subscription(
    s: &MqttSubscription,
    version: &str,
    execute: bool,
) -> Result<()> {
    string(&s.filter, 1024)?;
    properties(&s.user_properties, execute)?;
    ensure!(
        s.qos <= 2
            && s.retain_handling <= 2
            && s.subscription_identifier
                .is_none_or(|id| (1..=268_435_455).contains(&id)),
        "Invalid MQTT subscription options"
    );
    if execute {
        validate_mqtt_filter(&s.filter)?;
        ensure!(
            version == "5"
                || (!s.no_local
                    && !s.retain_as_published
                    && s.retain_handling == 0
                    && s.subscription_identifier.is_none()
                    && s.user_properties.is_empty()),
            "MQTT 5 subscription options require version 5"
        );
        ensure!(
            !s.no_local || !s.filter.starts_with("$share/"),
            "Shared subscriptions cannot use no_local"
        );
    }
    Ok(())
}
pub fn validate_mqtt_message(m: &MqttMessage, version: &str, execute: bool) -> Result<()> {
    string(&m.topic, 1024)?;
    ensure!(
        m.payload_source.len() <= MQTT_MAX_MESSAGE.div_ceil(3) * 4,
        "MQTT payload draft exceeds limit"
    );
    ensure!(
        m.qos <= 2 && matches!(m.encoding.as_str(), "text" | "json" | "base64"),
        "Invalid MQTT QoS or payload encoding"
    );
    let p = &m.properties;
    properties(&p.user_properties, execute)?;
    for s in [
        &p.response_topic,
        &p.content_type,
        &p.correlation_data_base64,
    ]
    .into_iter()
    .flatten()
    {
        string(s, 4096)?;
    }
    ensure!(
        p.payload_format_indicator.is_none_or(|v| v <= 1),
        "Invalid MQTT payload format indicator"
    );
    if execute {
        ensure!(
            !m.topic.is_empty() && rumqttc::mqttbytes::valid_topic(&m.topic),
            "MQTT publish topic must be nonempty without wildcards"
        );
        ensure!(
            version == "5" || p == &MqttPublishProperties::default(),
            "MQTT publish properties require version 5"
        );
        if let Some(t) = &p.response_topic {
            ensure!(
                !t.is_empty() && rumqttc::mqttbytes::valid_topic(t),
                "Invalid MQTT response topic"
            );
        }
        if let Some(c) = &p.correlation_data_base64 {
            STANDARD
                .decode(c)
                .context("Invalid correlation data base64")?;
        }
        let bytes = mqtt_payload(m)?;
        if p.payload_format_indicator == Some(1) {
            std::str::from_utf8(&bytes).context("MQTT payload indicator 1 requires UTF-8")?;
        }
    }
    Ok(())
}
pub fn mqtt_payload(m: &MqttMessage) -> Result<Vec<u8>> {
    let bytes = match m.encoding.as_str() {
        "text" => m.payload_source.as_bytes().to_vec(),
        "json" => {
            serde_json::from_str::<serde_json::Value>(&m.payload_source)
                .context("Invalid MQTT JSON payload")?;
            m.payload_source.as_bytes().to_vec()
        }
        "base64" => STANDARD
            .decode(&m.payload_source)
            .context("Invalid MQTT payload base64")?,
        _ => anyhow::bail!("Invalid MQTT payload encoding"),
    };
    ensure!(
        bytes.len() <= MQTT_MAX_MESSAGE,
        "MQTT payload exceeds 1 MiB"
    );
    Ok(bytes)
}
pub fn validate_mqtt(r: &RequestSpec, templates: bool) -> Result<()> {
    let Protocol::Mqtt { config: c } = &r.protocol else {
        return Ok(());
    };
    string(&c.client_id, 256)?;
    properties(&c.user_properties, !templates)?;
    ensure!(
        c.subscriptions.len() <= 64 && c.saved_messages.len() <= 64,
        "MQTT subscription or saved-message limit reached (64)"
    );
    ensure!(
        serde_json::to_vec(c)?.len() <= 5 * 1024 * 1024,
        "MQTT draft exceeds 5 MiB"
    );
    validate_mqtt_message(&c.message, &c.version, false)?;
    for m in &c.saved_messages {
        string(&m.id, 256)?;
        string(&m.name, 256)?;
        validate_mqtt_message(&m.message, &c.version, false)?;
    }
    for s in &c.subscriptions {
        validate_mqtt_subscription(s, &c.version, !templates && s.enabled)?;
    }
    if let Some(w) = &c.will {
        validate_mqtt_message(&w.message, &c.version, !templates)?;
    }
    if !templates {
        mqtt_url(&r.url)?;
        string(&r.auth.username, 4096)?;
        string(&r.auth.password, 4096)?;
        ensure!(
            matches!(c.version.as_str(), "3.1.1" | "5"),
            "MQTT version must be 3.1.1 or 5"
        );
        ensure!(
            (5..=3600).contains(&c.keep_alive_secs),
            "MQTT keep-alive must be 5–3600 seconds"
        );
        ensure!(
            c.reconnect.max_attempts <= 10 && (100..=30_000).contains(&c.reconnect.delay_ms),
            "MQTT reconnect is limited to 10 attempts with 100–30000 ms delay"
        );
        ensure!(
            c.clean_start || !c.client_id.is_empty(),
            "Persistent MQTT sessions require a client ID"
        );
        ensure!(
            c.version == "5"
                || (c.session_expiry_interval == 0
                    && c.user_properties.is_empty()
                    && c.will.as_ref().is_none_or(|w| w.delay_interval == 0)),
            "MQTT session expiry, connection properties and Will delay require version 5"
        );
        ensure!(
            matches!(r.auth.kind.as_str(), "none" | "basic"),
            "MQTT supports anonymous or Basic username/password authentication; Bearer is unsupported"
        );
        let websocket = matches!(mqtt_url(&r.url)?.scheme(), "ws" | "wss");
        ensure!(
            websocket || r.query.iter().chain(&r.headers).all(|p| !p.enabled),
            "MQTT TCP has no HTTP headers/query; use WebSocket transport for handshake fields"
        );
        ensure!(
            r.body_kind == "none" && r.method == "GET",
            "MQTT has no HTTP body/method; use the MQTT composer"
        );
        if websocket {
            for h in r.headers.iter().filter(|h| h.enabled) {
                ensure!(
                    !matches!(
                        h.key.to_ascii_lowercase().as_str(),
                        "host"
                            | "connection"
                            | "upgrade"
                            | "sec-websocket-key"
                            | "sec-websocket-version"
                            | "sec-websocket-protocol"
                            | "sec-websocket-extensions"
                            | "content-length"
                            | "transfer-encoding"
                    ),
                    "MQTT WebSocket transport headers are SDK-owned"
                );
            }
        }
        ensure!(
            r.post_response_script.trim().is_empty(),
            "MQTT event/post scripts are not supported"
        );
    }
    Ok(())
}
/// JSON validation is structural, but original source bytes are preserved (including large numbers).
pub fn resolve_mqtt_message(m: &MqttMessage, environment: &Environment) -> Result<MqttMessage> {
    let mut value = serde_json::to_value(m)?;
    value["payload_source"] = "".into();
    let source = serde_json::to_string(&value)?;
    let resolved = resolve_grpc_source(&source, environment)?;
    let mut output: MqttMessage = serde_json::from_str(&resolved)?;
    output.payload_source = crate::resolve_mqtt_source(&m.payload_source, environment)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn request() -> RequestSpec {
        serde_json::from_value(json!({"protocol":{"kind":"mqtt"},"id":"r","name":"MQTT","method":"GET","url":"mqtt://localhost:18891","description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":false,"verify_tls":true,"assertions":[],"examples":[]})).unwrap()
    }
    #[test]
    fn incomplete_composer_saved_messages_and_disabled_subscriptions_are_editable() {
        let mut r = request();
        let Protocol::Mqtt { config } = &mut r.protocol else {
            unreachable!()
        };
        config.message.encoding = "json".into();
        config.message.payload_source = "{unfinished{{not_defined}}".into();
        config.saved_messages.push(MqttSavedMessage {
            id: "one".into(),
            name: "incomplete".into(),
            message: config.message.clone(),
        });
        config.subscriptions.push(MqttSubscription {
            filter: "unfinished+{{missing}}".into(),
            enabled: false,
            ..Default::default()
        });
        crate::validate_request(&r, true).unwrap();
        let resolved = crate::resolve_request(&r, None).unwrap();
        let Protocol::Mqtt { config } = &resolved.protocol else {
            unreachable!()
        };
        assert_eq!(config.message.payload_source, "{unfinished{{not_defined}}");
        assert_eq!(config.saved_messages.len(), 1);
        assert!(config.subscriptions.is_empty());
        assert!(validate_mqtt_message(&config.message, "5", true).is_err());
    }
    #[test]
    fn mqtt_json_source_and_binary_bytes_are_never_coerced() {
        let e = Environment {
            id: "e".into(),
            name: "e".into(),
            variables: vec![crate::Pair {
                id: "v".into(),
                key: "selected".into(),
                value: "original".into(),
                enabled: true,
                secret: None,
                local_value: None,
            }],
        };
        let message = MqttMessage {
            topic: "{{selected}}/topic".into(),
            encoding: "json".into(),
            payload_source: r#"{"large":18446744073709551616000,"nested":{"s":"{{selected}}"}}"#
                .into(),
            ..Default::default()
        };
        let resolved = resolve_mqtt_message(&message, &e).unwrap();
        assert_eq!(resolved.topic, "original/topic");
        assert_eq!(
            String::from_utf8(mqtt_payload(&resolved).unwrap()).unwrap(),
            r#"{"large":18446744073709551616000,"nested":{"s":"original"}}"#
        );
        let binary = MqttMessage {
            topic: "t".into(),
            encoding: "base64".into(),
            payload_source: STANDARD.encode([0, 255, 128, 0]),
            ..Default::default()
        };
        assert_eq!(mqtt_payload(&binary).unwrap(), vec![0, 255, 128, 0]);
    }
    #[test]
    fn sdk_wildcard_semantics_and_version_guards_are_explicit() {
        for filter in ["#", "a/+", "a/#", "中文/+", "$share/group/a/+"] {
            validate_mqtt_filter(filter).unwrap();
        }
        for filter in ["", "a+", "a/#/b", "$share//a", "$share/+/a"] {
            assert!(validate_mqtt_filter(filter).is_err(), "{filter}");
        }
        assert!(
            validate_mqtt_message(
                &MqttMessage {
                    topic: "a/+".into(),
                    ..Default::default()
                },
                "5",
                true
            )
            .is_err()
        );
        let mut r = request();
        let Protocol::Mqtt { config } = &mut r.protocol else {
            unreachable!()
        };
        config.version = "3.1.1".into();
        config.session_expiry_interval = 1;
        assert!(
            crate::validate_request(&r, false)
                .unwrap_err()
                .to_string()
                .contains("version 5")
        );
        assert!(
            validate_mqtt_subscription(
                &MqttSubscription {
                    filter: "a/#".into(),
                    no_local: true,
                    ..Default::default()
                },
                "3.1.1",
                true
            )
            .is_err()
        );
    }
    #[test]
    fn anonymous_basic_and_persistent_client_id_are_explicit() {
        let mut r = request();
        r.auth.kind = "bearer".into();
        assert!(
            crate::validate_request(&r, false)
                .unwrap_err()
                .to_string()
                .contains("Bearer")
        );
        r.auth.kind = "basic".into();
        crate::validate_request(&r, false).unwrap();
        let Protocol::Mqtt { config } = &mut r.protocol else {
            unreachable!()
        };
        config.clean_start = false;
        assert!(
            crate::validate_request(&r, false)
                .unwrap_err()
                .to_string()
                .contains("client ID")
        );
    }
    #[test]
    fn urls_and_memory_budgets_fail_closed() {
        for url in [
            "mqtt://u:p@localhost",
            "mqtt://localhost/path",
            "mqtt://localhost?token=secret",
            "http://localhost",
            "mqtts://localhost#fragment",
        ] {
            assert!(mqtt_url(url).is_err());
        }
        for url in [
            "mqtt://localhost",
            "mqtts://localhost",
            "ws://localhost/mqtt",
            "wss://localhost/mqtt",
        ] {
            assert!(mqtt_url(url).unwrap().port_or_known_default().is_some());
        }
        let mut r = request();
        let Protocol::Mqtt { config } = &mut r.protocol else {
            unreachable!()
        };
        config.subscriptions = vec![MqttSubscription::default(); 65];
        assert!(crate::validate_request(&r, true).is_err());
        assert!(
            mqtt_payload(&MqttMessage {
                payload_source: "x".repeat(MQTT_MAX_MESSAGE + 1),
                ..Default::default()
            })
            .is_err()
        );
        assert!(
            validate_mqtt_message(
                &MqttMessage {
                    topic: "a\0b".into(),
                    ..Default::default()
                },
                "5",
                true
            )
            .is_err()
        );
    }
    #[test]
    fn will_source_is_resolved_and_roundtrips_without_an_environment() {
        let mut r = request();
        let Protocol::Mqtt { config } = &mut r.protocol else {
            unreachable!()
        };
        config.will = Some(MqttWill {
            message: MqttMessage {
                topic: "will".into(),
                encoding: "json".into(),
                payload_source: r#"{"nested":{"n":18446744073709551616000}}"#.into(),
                ..Default::default()
            },
            delay_interval: 1,
        });
        let resolved = crate::resolve_request(&r, None).unwrap();
        assert_eq!(resolved.protocol, r.protocol);
    }
}
