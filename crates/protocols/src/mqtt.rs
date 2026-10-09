//! MQTT adaptation. The mature SDK owns framing, packet identifiers and QoS handshakes.
use crate::{
    Command, EventMessage, Handshake, MAX_INPUT, MAX_MESSAGE, PrivacyMask, SendMessage, Session,
    SessionState,
};
use anyhow::{Context, Result, bail, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use moleapi_core::{
    MqttConfig, MqttMessage, MqttSubscription, NetworkPolicy, Protocol, RequestSpec, mqtt_payload,
    mqtt_url, resolve_mqtt_message, validate_mqtt_message, validate_mqtt_subscription,
};
use rumqttc::v5::mqttbytes::v5;
use rumqttc::{NetworkOptions, Outgoing, TlsConfiguration, Transport};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, VecDeque},
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};
use tokio::sync::mpsc;
#[derive(Default)]
pub(crate) struct Control {
    config: MqttConfig,
    subscriptions: HashMap<String, MqttSubscription>,
    inputs: usize,
    secrets: Vec<String>,
    secret_bytes: Vec<Vec<u8>>,
    withhold: bool,
}
impl Control {
    pub fn new(config: MqttConfig) -> Self {
        Self {
            subscriptions: config
                .subscriptions
                .iter()
                .filter(|s| s.enabled)
                .map(|s| (s.filter.clone(), s.clone()))
                .collect(),
            config,
            inputs: 0,
            secrets: vec![],
            secret_bytes: vec![],
            withhold: false,
        }
    }
    fn capture(&mut self, text: &str) {
        if text.is_empty() || self.secrets.iter().any(|s| s == text) {
            return;
        }
        if text.len() > 4096
            || self.secrets.iter().map(String::len).sum::<usize>() + text.len() > 16384
        {
            self.withhold = true;
            return;
        }
        self.secrets.push(text.into());
    }
    fn capture_message(&mut self, m: &MqttMessage) -> Result<()> {
        if m.topic_secret {
            self.capture(&m.topic);
        }
        if m.payload_secret {
            let bytes = mqtt_payload(m)?;
            if bytes.len() > 4096
                || self.secret_bytes.iter().map(Vec::len).sum::<usize>() + bytes.len() > 16384
            {
                self.withhold = true;
            } else if !bytes.is_empty() {
                if let Ok(text) = std::str::from_utf8(&bytes) {
                    self.capture(text);
                }
                self.capture(&STANDARD.encode(&bytes));
                self.secret_bytes.push(bytes);
            }
        }
        for p in &m.properties.user_properties {
            if p.secret {
                self.capture(&p.value);
            }
        }
        Ok(())
    }
}
pub(crate) enum MqttCommand {
    Publish(MqttMessage),
    Subscribe(MqttSubscription),
    Unsubscribe(String),
    Abort,
}
pub(crate) fn send(s: &Arc<Session>, message: SendMessage) -> Result<()> {
    let mut r = s.record.lock().unwrap();
    ensure!(
        r.summary.protocol == "mqtt"
            && r.summary.state == SessionState::Open
            && !s.cancel.is_cancelled(),
        "Expected open MQTT session"
    );
    let mut control = s.mqtt.lock().unwrap();
    ensure!(
        control.inputs < 10_000,
        "MQTT input-message limit reached (10000)"
    );
    let environment =
        s.grpc_environment
            .lock()
            .unwrap()
            .clone()
            .unwrap_or(moleapi_core::Environment {
                id: String::new(),
                name: String::new(),
                variables: vec![],
            });
    let mask = s
        .grpc_mask
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_else(|| Arc::new(str::to_owned));
    let command = match message {
        SendMessage::MqttPublish { message } => {
            let message = resolve_mqtt_message(&message, &environment)
                .context("Invalid or unresolved MQTT message")?;
            validate_mqtt_message(&message, &control.config.version, true)
                .map_err(|e| anyhow::anyhow!(mask(&e.to_string())))?;
            control.capture_message(&message)?;
            MqttCommand::Publish(message)
        }
        SendMessage::MqttSubscribe { subscription } => {
            let source = serde_json::to_string(&subscription)?;
            let subscription: MqttSubscription =
                serde_json::from_str(&moleapi_core::resolve_grpc_source(&source, &environment)?)?;
            ensure!(
                control.subscriptions.len() < 64
                    || control.subscriptions.contains_key(&subscription.filter),
                "MQTT active subscription limit reached (64)"
            );
            validate_mqtt_subscription(&subscription, &control.config.version, true)?;
            if subscription.filter_secret {
                control.capture(&subscription.filter);
            }
            for p in &subscription.user_properties {
                if p.secret {
                    control.capture(&p.value);
                }
            }
            MqttCommand::Subscribe(subscription)
        }
        SendMessage::MqttUnsubscribe { filter } => {
            let filter = moleapi_core::resolve_value(&filter, &environment)?;
            moleapi_core::validate_mqtt_filter(&filter)?;
            MqttCommand::Unsubscribe(filter)
        }
        SendMessage::MqttAbort => MqttCommand::Abort,
        _ => unreachable!(),
    };
    let size = match &command {
        MqttCommand::Publish(m) => serde_json::to_vec(m)?.len(),
        MqttCommand::Subscribe(v) => serde_json::to_vec(v)?.len(),
        MqttCommand::Unsubscribe(v) => v.len(),
        MqttCommand::Abort => 0,
    };
    ensure!(
        r.input + size <= MAX_INPUT,
        "Session input limit reached (20 MiB)"
    );
    let subscription_edit = match &command {
        MqttCommand::Subscribe(subscription) => {
            Some((subscription.filter.clone(), Some(subscription.clone())))
        }
        MqttCommand::Unsubscribe(filter) => Some((filter.clone(), None)),
        _ => None,
    };
    s.commands
        .try_send(Command::Mqtt(command))
        .map_err(|_| anyhow::anyhow!("Session command queue is full or closed"))?;
    if let Some((filter, subscription)) = subscription_edit {
        if let Some(subscription) = subscription {
            control.subscriptions.insert(filter, subscription);
        } else {
            control.subscriptions.remove(&filter);
        }
    }
    control.inputs += 1;
    r.input += size;
    Ok(())
}
fn pairs(properties: &[moleapi_core::MqttProperty]) -> Vec<(String, String)> {
    properties
        .iter()
        .map(|p| (p.key.clone(), p.value.clone()))
        .collect()
}
fn publish_properties(m: &MqttMessage) -> Result<v5::PublishProperties> {
    let p = &m.properties;
    Ok(v5::PublishProperties {
        payload_format_indicator: p.payload_format_indicator,
        message_expiry_interval: p.message_expiry_interval,
        response_topic: p.response_topic.clone(),
        correlation_data: p
            .correlation_data_base64
            .as_ref()
            .map(|s| STANDARD.decode(s).map(Into::into))
            .transpose()?,
        content_type: p.content_type.clone(),
        user_properties: pairs(&p.user_properties),
        topic_alias: None,
        subscription_identifiers: vec![],
    })
}
fn transport(
    url: &url::Url,
    verify: bool,
    network: Option<&moleapi_core::RequestNetwork>,
) -> Result<Transport> {
    let tls = || -> Result<TlsConfiguration> {
        if let Some(network) = network {
            return Ok(TlsConfiguration::Rustls(Arc::new(
                moleapi_core::request_tls_config(network, verify)?,
            )));
        }
        let mut roots = rustls::RootCertStore::empty();
        for cert in rustls_native_certs::load_native_certs().certs {
            let _ = roots.add(cert);
        }
        let mut config = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()?
        .with_root_certificates(roots)
        .with_no_client_auth();
        if !verify {
            config
                .dangerous()
                .set_certificate_verifier(Arc::new(crate::grpc::UnverifiedCertificate));
        }
        Ok(TlsConfiguration::Rustls(Arc::new(config)))
    };
    Ok(match url.scheme() {
        "mqtt" => Transport::Tcp,
        "mqtts" => Transport::Tls(tls()?),
        "ws" => Transport::Ws,
        "wss" => Transport::Wss(tls()?),
        _ => unreachable!(),
    })
}
enum Sdk {
    V3(rumqttc::AsyncClient, Box<rumqttc::EventLoop>),
    V5(rumqttc::v5::AsyncClient, Box<rumqttc::v5::EventLoop>),
}
enum SdkEvent {
    Outgoing(Outgoing),
    V3(rumqttc::Packet),
    V5(Box<v5::Packet>),
}
impl Sdk {
    fn new(r: &RequestSpec, c: &MqttConfig) -> Result<Self> {
        let mut url = mqtt_url(&r.url)?;
        let websocket = matches!(url.scheme(), "ws" | "wss");
        if websocket {
            for p in r.query.iter().filter(|p| p.enabled) {
                url.query_pairs_mut().append_pair(&p.key, &p.value);
            }
        }
        let mut handshake_request = r.clone();
        handshake_request.auth.kind = "none".into();
        let headers = moleapi_core::request_headers(&handshake_request)?;
        let host = if matches!(url.scheme(), "ws" | "wss") {
            url.to_string()
        } else {
            url.host_str().unwrap().trim_matches(['[', ']']).to_owned()
        };
        let id = if c.client_id.is_empty() {
            format!("moleapi-{}", uuid::Uuid::new_v4())
        } else {
            c.client_id.clone()
        };
        let port = url.port_or_known_default().unwrap();
        let transport = transport(&url, r.verify_tls, r.network.as_deref())?;
        if c.version == "5" {
            let mut o = rumqttc::v5::MqttOptions::new(id, host, port);
            o.set_clean_start(c.clean_start)
                .set_keep_alive(Duration::from_secs(u64::from(c.keep_alive_secs)))
                .set_session_expiry_interval(Some(c.session_expiry_interval))
                .set_user_properties(pairs(&c.user_properties))
                .set_max_packet_size(Some((MAX_MESSAGE + 32768) as u32))
                .set_receive_maximum(Some(16))
                .set_outgoing_inflight_upper_limit(16)
                .set_transport(transport)
                .set_connection_timeout(r.timeout_ms.div_ceil(1000));
            if websocket {
                let headers = headers.clone();
                o.set_request_modifier(move |mut request| {
                    for (name, value) in &headers {
                        request.headers_mut().append(name, value.clone());
                    }
                    std::future::ready(request)
                });
            }
            if r.auth.kind == "basic" {
                o.set_credentials(&r.auth.username, &r.auth.password);
            }
            if let Some(w) = &c.will {
                let p = publish_properties(&w.message)?;
                o.set_last_will(v5::LastWill::new(
                    &w.message.topic,
                    mqtt_payload(&w.message)?,
                    rumqttc::v5::mqttbytes::qos(w.message.qos).unwrap(),
                    w.message.retain,
                    Some(v5::LastWillProperties {
                        delay_interval: Some(w.delay_interval),
                        payload_format_indicator: p.payload_format_indicator,
                        message_expiry_interval: p.message_expiry_interval,
                        content_type: p.content_type,
                        response_topic: p.response_topic,
                        correlation_data: p.correlation_data,
                        user_properties: p.user_properties,
                    }),
                ));
            }
            let (client, eventloop) = rumqttc::v5::AsyncClient::new(o, 32);
            Ok(Self::V5(client, Box::new(eventloop)))
        } else {
            let mut o = rumqttc::MqttOptions::new(id, host, port);
            o.set_clean_session(c.clean_start)
                .set_keep_alive(Duration::from_secs(u64::from(c.keep_alive_secs)))
                .set_max_packet_size(MAX_MESSAGE + 32768, MAX_MESSAGE + 32768)
                .set_inflight(16)
                .set_transport(transport);
            if websocket {
                let headers = headers.clone();
                o.set_request_modifier(move |mut request| {
                    for (name, value) in &headers {
                        request.headers_mut().append(name, value.clone());
                    }
                    std::future::ready(request)
                });
            }
            if r.auth.kind == "basic" {
                o.set_credentials(&r.auth.username, &r.auth.password);
            }
            if let Some(w) = &c.will {
                o.set_last_will(rumqttc::LastWill::new(
                    &w.message.topic,
                    mqtt_payload(&w.message)?,
                    rumqttc::mqttbytes::qos(w.message.qos).unwrap(),
                    w.message.retain,
                ));
            }
            let (client, eventloop) = rumqttc::AsyncClient::new(o, 32);
            Ok(Self::V3(client, Box::new(eventloop)))
        }
    }
    fn pin(
        &mut self,
        addresses: Vec<SocketAddr>,
        timeout: u64,
        budget: Arc<rumqttc::TrafficBudget>,
        connector: Option<rumqttc::SocketConnector>,
    ) {
        let mut n = NetworkOptions::new();
        n.set_traffic_budget(budget)
            .set_pinned_addresses(addresses)
            .set_connection_timeout(timeout.div_ceil(1000));
        if let Some(connector) = connector {
            n.set_socket_connector(connector);
        }
        match self {
            Self::V3(_, e) => {
                e.set_network_options(n);
            }
            Self::V5(_, e) => {
                e.options.set_network_options(n);
            }
        }
    }
    fn buffered(&mut self) -> Vec<SdkEvent> {
        match self {
            Self::V3(_, e) => e
                .state
                .events
                .drain(..)
                .map(|event| match event {
                    rumqttc::Event::Incoming(p) => SdkEvent::V3(p),
                    rumqttc::Event::Outgoing(o) => SdkEvent::Outgoing(o),
                })
                .collect(),
            Self::V5(_, e) => e
                .state
                .events
                .drain(..)
                .map(|event| match event {
                    rumqttc::v5::Event::Incoming(p) => SdkEvent::V5(Box::new(p)),
                    rumqttc::v5::Event::Outgoing(o) => SdkEvent::Outgoing(o),
                })
                .collect(),
        }
    }
    fn resumed(&mut self, c: &MqttConfig) {
        if c.session_expiry_interval > 0
            && let Self::V5(_, e) = self
        {
            e.options.set_clean_start(false);
        }
    }
    async fn poll(&mut self) -> Result<SdkEvent> {
        match self {
            Self::V3(_, e) => Ok(match e.poll().await? {
                rumqttc::Event::Outgoing(o) => SdkEvent::Outgoing(o),
                rumqttc::Event::Incoming(p) => SdkEvent::V3(p),
            }),
            Self::V5(_, e) => Ok(match e.poll().await? {
                rumqttc::v5::Event::Outgoing(o) => SdkEvent::Outgoing(o),
                rumqttc::v5::Event::Incoming(p) => SdkEvent::V5(Box::new(p)),
            }),
        }
    }
    fn publish(&self, m: &MqttMessage) -> Result<()> {
        let payload = mqtt_payload(m)?;
        match self {
            Self::V3(c, _) => c.try_publish(
                &m.topic,
                rumqttc::mqttbytes::qos(m.qos).unwrap(),
                m.retain,
                payload,
            )?,
            Self::V5(c, _) => c.try_publish_with_properties(
                &m.topic,
                rumqttc::v5::mqttbytes::qos(m.qos).unwrap(),
                m.retain,
                payload,
                publish_properties(m)?,
            )?,
        };
        Ok(())
    }
    fn subscribe(&self, s: &MqttSubscription) -> Result<()> {
        match self {
            Self::V3(c, _) => {
                c.try_subscribe(&s.filter, rumqttc::mqttbytes::qos(s.qos).unwrap())?
            }
            Self::V5(c, _) => {
                let mut f = v5::Filter::new(&s.filter, rumqttc::v5::mqttbytes::qos(s.qos).unwrap());
                f.nolocal = s.no_local;
                f.preserve_retain = s.retain_as_published;
                f.retain_forward_rule = match s.retain_handling {
                    0 => v5::RetainForwardRule::OnEverySubscribe,
                    1 => v5::RetainForwardRule::OnNewSubscribe,
                    _ => v5::RetainForwardRule::Never,
                };
                c.try_subscribe_many_with_properties(
                    [f],
                    v5::SubscribeProperties {
                        id: s.subscription_identifier,
                        user_properties: pairs(&s.user_properties),
                    },
                )?;
            }
        }
        Ok(())
    }
    fn unsubscribe(&self, filter: &str) -> Result<()> {
        match self {
            Self::V3(c, _) => c.try_unsubscribe(filter)?,
            Self::V5(c, _) => c.try_unsubscribe(filter)?,
        };
        Ok(())
    }
    fn disconnect(&self) -> Result<()> {
        match self {
            Self::V3(c, _) => c.try_disconnect()?,
            Self::V5(c, _) => c.try_disconnect()?,
        };
        Ok(())
    }
}
fn scrub(v: &mut Value, mask: &PrivacyMask) -> bool {
    match v {
        Value::String(s) => {
            let m = mask(s);
            let redacted = m != *s;
            *s = m;
            redacted
        }
        Value::Array(a) => {
            let mut changed = false;
            for v in a {
                changed |= scrub(v, mask);
            }
            changed
        }
        Value::Object(o) => {
            let mut changed = false;
            for v in o.values_mut() {
                changed |= scrub(v, mask);
            }
            changed
        }
        _ => false,
    }
}
fn status(
    s: &Session,
    mask: &PrivacyMask,
    operation: &str,
    state: &str,
    pkid: Option<u16>,
    codes: Vec<String>,
    mut details: Value,
) -> Result<()> {
    scrub(&mut details, mask);
    s.event(
        "system",
        EventMessage::MqttStatus {
            operation: operation.into(),
            status: state.into(),
            packet_id: pkid,
            reason_codes: codes.into_iter().map(|c| mask(&c)).collect(),
            details,
        },
    )
}
// MQTT binary fields must be checked after decoding: base64 padding/alignment can
// hide a known private substring from a string-only redactor.
fn private_bytes(s: &Session, mask: &PrivacyMask, bytes: &[u8]) -> bool {
    let control = s.mqtt.lock().unwrap();
    let sensitive = control.withhold
        || control
            .secret_bytes
            .iter()
            .any(|secret| bytes.windows(secret.len()).any(|part| part == secret));
    drop(control);
    let text = String::from_utf8_lossy(bytes);
    let encoded = STANDARD.encode(bytes);
    sensitive || mask(&text) != text || mask(&encoded) != encoded
}
fn scrub_binary_properties(s: &Session, mask: &PrivacyMask, properties: &mut Value) -> bool {
    let Some(encoded) = properties
        .get("correlation_data_base64")
        .and_then(Value::as_str)
    else {
        return false;
    };
    if STANDARD
        .decode(encoded)
        .map_or(true, |bytes| private_bytes(s, mask, &bytes))
    {
        properties["correlation_data_base64"] = Value::Null;
        return true;
    }
    false
}
fn error_text(error: &anyhow::Error, mask: &PrivacyMask) -> String {
    // NotConnAck's SDK Display uses raw Packet Debug (byte arrays can evade text
    // redaction). Keep the protocol error without exposing unexpected wire data.
    if matches!(
        error.downcast_ref::<rumqttc::ConnectionError>(),
        Some(rumqttc::ConnectionError::NotConnAck(_))
    ) || matches!(
        error.downcast_ref::<rumqttc::v5::ConnectionError>(),
        Some(rumqttc::v5::ConnectionError::NotConnAck(_))
    ) {
        return "MQTT broker sent a packet other than CONNACK; packet data withheld".into();
    }
    mask(&format!("{error:#}"))
}
struct MessageView<'a> {
    topic: &'a str,
    payload: &'a [u8],
    qos: u8,
    retain: bool,
    duplicate: bool,
    pkid: u16,
    properties: Value,
    topic_secret: bool,
    payload_secret: bool,
}
fn message(s: &Session, mask: &PrivacyMask, direction: &str, view: MessageView<'_>) -> Result<()> {
    let MessageView {
        topic,
        payload,
        qos,
        retain,
        duplicate,
        pkid,
        mut properties,
        topic_secret,
        payload_secret,
    } = view;
    ensure!(
        !topic.is_empty() && topic.len() <= 1024,
        "Invalid or oversized MQTT incoming topic"
    );
    ensure!(
        serde_json::to_vec(&properties)?.len() <= 32768,
        "MQTT incoming properties exceed 32 KiB"
    );
    if let Some(values) = properties.get("user_properties").and_then(Value::as_array) {
        ensure!(
            values.len() <= 32,
            "MQTT incoming property limit reached (32)"
        );
    }
    let control = s.mqtt.lock().unwrap();
    let secret_filter = control.subscriptions.values().any(|v| v.filter_secret);
    drop(control);
    let masked_topic = mask(topic);
    let topic_redacted = topic_secret || secret_filter || masked_topic != topic;
    let payload_redacted = payload_secret || private_bytes(s, mask, payload);
    let binary_properties_redacted = scrub_binary_properties(s, mask, &mut properties);
    let properties_redacted = scrub(&mut properties, mask)
        || binary_properties_redacted
        || properties.to_string().contains("[REDACTED]");
    s.event(
        direction,
        EventMessage::MqttMessage {
            topic: if topic_redacted {
                "[REDACTED]".into()
            } else {
                masked_topic
            },
            payload_base64: if payload_redacted {
                String::new()
            } else {
                STANDARD.encode(payload)
            },
            payload_text: if payload_redacted {
                None
            } else {
                std::str::from_utf8(payload).ok().map(str::to_owned)
            },
            qos,
            retain,
            duplicate,
            packet_id: pkid,
            properties,
            topic_redacted,
            payload_redacted,
            properties_redacted,
        },
    )
}
fn properties_json(p: Option<&v5::PublishProperties>) -> Value {
    p.map_or(json!({}),|p|json!({"payload_format_indicator":p.payload_format_indicator,"message_expiry_interval":p.message_expiry_interval,"topic_alias":p.topic_alias,"response_topic":p.response_topic,"correlation_data_base64":p.correlation_data.as_ref().map(|b|STANDARD.encode(b)),"user_properties":p.user_properties.iter().map(|(key,value)|json!({"key":key,"value":value})).collect::<Vec<_>>(),"subscription_identifiers":p.subscription_identifiers,"content_type":p.content_type}))
}
// ACK reason strings/user properties are included without requiring a patched serde implementation in the SDK.
macro_rules! ack_details {($p:expr)=>{$p.as_ref().map_or(json!({}),|p|json!({"reason_string":p.reason_string,"user_properties":p.user_properties.iter().map(|(key,value)|json!({"key":key,"value":value})).collect::<Vec<_>>()}))}}
struct Runtime {
    packets: usize,
    connects: usize,
    pending: VecDeque<MqttMessage>,
    inflight: HashMap<u16, MqttMessage>,
    subscriptions: VecDeque<MqttSubscription>,
}
impl Runtime {
    fn process(
        &mut self,
        s: &Session,
        mask: &PrivacyMask,
        event: SdkEvent,
    ) -> Result<Option<bool>> {
        self.packets += 1;
        ensure!(self.packets <= 10_000, "MQTT packet limit reached (10000)");
        match event {
            SdkEvent::Outgoing(o) => {
                let (op, id) = match o {
                    Outgoing::Publish(id) => ("publish", Some(id)),
                    Outgoing::Subscribe(id) => ("subscribe", Some(id)),
                    Outgoing::Unsubscribe(id) => ("unsubscribe", Some(id)),
                    Outgoing::PubAck(id) => ("puback", Some(id)),
                    Outgoing::PubRec(id) => ("pubrec", Some(id)),
                    Outgoing::PubRel(id) => ("pubrel", Some(id)),
                    Outgoing::PubComp(id) => ("pubcomp", Some(id)),
                    Outgoing::AwaitAck(id) => ("await_ack", Some(id)),
                    Outgoing::PingReq | Outgoing::PingResp => ("ping", None),
                    Outgoing::Disconnect => ("disconnect", None),
                };
                if op == "publish" {
                    let id = id.unwrap();
                    let m = self
                        .inflight
                        .get(&id)
                        .cloned()
                        .or_else(|| self.pending.pop_front());
                    if let Some(m) = m {
                        let bytes = mqtt_payload(&m)?;
                        let mut properties = serde_json::to_value(&m.properties)?;
                        for p in properties["user_properties"]
                            .as_array_mut()
                            .into_iter()
                            .flatten()
                        {
                            if p["secret"] == true {
                                p["value"] = "[REDACTED]".into();
                            }
                        }
                        message(
                            s,
                            mask,
                            "outgoing",
                            MessageView {
                                topic: &m.topic,
                                payload: &bytes,
                                qos: m.qos,
                                retain: m.retain,
                                duplicate: false,
                                pkid: id,
                                properties,
                                topic_secret: m.topic_secret,
                                payload_secret: m.payload_secret,
                            },
                        )?;
                        if m.qos > 0 {
                            self.inflight.insert(id, m);
                        }
                    }
                }
                status(s, mask, op, "sent", id, vec![], json!({}))?;
            }
            SdkEvent::V3(p) => match p {
                rumqttc::Packet::ConnAck(p) => {
                    status(
                        s,
                        mask,
                        "connect",
                        "acknowledged",
                        None,
                        vec![format!("{:?}", p.code)],
                        json!({"session_present":p.session_present}),
                    )?;
                    return Ok(Some(p.session_present));
                }
                rumqttc::Packet::Publish(p) => message(
                    s,
                    mask,
                    "incoming",
                    MessageView {
                        topic: &p.topic,
                        payload: &p.payload,
                        qos: p.qos as u8,
                        retain: p.retain,
                        duplicate: p.dup,
                        pkid: p.pkid,
                        properties: json!({}),
                        topic_secret: false,
                        payload_secret: false,
                    },
                )?,
                rumqttc::Packet::PubAck(p) => {
                    self.inflight.remove(&p.pkid);
                    status(
                        s,
                        mask,
                        "publish",
                        "acknowledged",
                        Some(p.pkid),
                        vec!["Success".into()],
                        json!({"qos":1}),
                    )?;
                }
                rumqttc::Packet::PubComp(p) => {
                    self.inflight.remove(&p.pkid);
                    status(
                        s,
                        mask,
                        "publish",
                        "acknowledged",
                        Some(p.pkid),
                        vec!["Success".into()],
                        json!({"qos":2}),
                    )?;
                }
                rumqttc::Packet::PubRec(p) => status(
                    s,
                    mask,
                    "pubrec",
                    "handshake",
                    Some(p.pkid),
                    vec![],
                    json!({}),
                )?,
                rumqttc::Packet::PubRel(p) => status(
                    s,
                    mask,
                    "pubrel",
                    "handshake",
                    Some(p.pkid),
                    vec![],
                    json!({}),
                )?,
                rumqttc::Packet::SubAck(p) => status(
                    s,
                    mask,
                    "subscribe",
                    if p.return_codes
                        .iter()
                        .any(|v| matches!(v, rumqttc::SubscribeReasonCode::Failure))
                    {
                        "rejected"
                    } else {
                        "acknowledged"
                    },
                    Some(p.pkid),
                    p.return_codes.iter().map(|c| format!("{c:?}")).collect(),
                    json!({}),
                )?,
                rumqttc::Packet::UnsubAck(p) => status(
                    s,
                    mask,
                    "unsubscribe",
                    "acknowledged",
                    Some(p.pkid),
                    vec![],
                    json!({}),
                )?,
                rumqttc::Packet::Disconnect => bail!("MQTT broker disconnected"),
                _ => status(
                    s,
                    mask,
                    "packet",
                    "received",
                    None,
                    vec![],
                    json!({"type":match p {rumqttc::Packet::PingReq=>"ping_request",rumqttc::Packet::PingResp=>"ping_response",_=>"unexpected_packet"},"packet_data_withheld":true}),
                )?,
            },
            SdkEvent::V5(p) => match *p {
                v5::Packet::ConnAck(p) => {
                    let details=p.properties.as_ref().map_or(json!({}),|v|json!({"session_expiry_interval":v.session_expiry_interval,"receive_maximum":v.receive_max,"max_qos":v.max_qos,"retain_available":v.retain_available,"max_packet_size":v.max_packet_size,"assigned_client_identifier":v.assigned_client_identifier,"topic_alias_maximum":v.topic_alias_max,"reason_string":v.reason_string,"user_properties":v.user_properties,"wildcard_subscription_available":v.wildcard_subscription_available,"subscription_identifiers_available":v.subscription_identifiers_available,"shared_subscription_available":v.shared_subscription_available,"server_keep_alive":v.server_keep_alive,"response_information":v.response_information,"server_reference":v.server_reference}));
                    status(
                        s,
                        mask,
                        "connect",
                        "acknowledged",
                        None,
                        vec![format!("{:?}", p.code)],
                        json!({"session_present":p.session_present,"properties":details}),
                    )?;
                    return Ok(Some(p.session_present));
                }
                v5::Packet::Publish(p) => message(
                    s,
                    mask,
                    "incoming",
                    MessageView {
                        topic: std::str::from_utf8(&p.topic)?,
                        payload: &p.payload,
                        qos: p.qos as u8,
                        retain: p.retain,
                        duplicate: p.dup,
                        pkid: p.pkid,
                        properties: properties_json(p.properties.as_ref()),
                        topic_secret: false,
                        payload_secret: false,
                    },
                )?,
                v5::Packet::PubAck(p) => {
                    self.inflight.remove(&p.pkid);
                    status(
                        s,
                        mask,
                        "publish",
                        if matches!(
                            p.reason,
                            v5::PubAckReason::Success | v5::PubAckReason::NoMatchingSubscribers
                        ) {
                            "acknowledged"
                        } else {
                            "rejected"
                        },
                        Some(p.pkid),
                        vec![format!("{:?}", p.reason)],
                        ack_details!(p.properties),
                    )?;
                }
                v5::Packet::PubComp(p) => {
                    self.inflight.remove(&p.pkid);
                    status(
                        s,
                        mask,
                        "publish",
                        if matches!(p.reason, v5::PubCompReason::Success) {
                            "acknowledged"
                        } else {
                            "rejected"
                        },
                        Some(p.pkid),
                        vec![format!("{:?}", p.reason)],
                        ack_details!(p.properties),
                    )?;
                }
                v5::Packet::PubRec(p) => {
                    if !matches!(
                        p.reason,
                        v5::PubRecReason::Success | v5::PubRecReason::NoMatchingSubscribers
                    ) {
                        self.inflight.remove(&p.pkid);
                    }
                    status(
                        s,
                        mask,
                        "pubrec",
                        if matches!(
                            p.reason,
                            v5::PubRecReason::Success | v5::PubRecReason::NoMatchingSubscribers
                        ) {
                            "handshake"
                        } else {
                            "rejected"
                        },
                        Some(p.pkid),
                        vec![format!("{:?}", p.reason)],
                        ack_details!(p.properties),
                    )?;
                }
                v5::Packet::PubRel(p) => status(
                    s,
                    mask,
                    "pubrel",
                    "handshake",
                    Some(p.pkid),
                    vec![format!("{:?}", p.reason)],
                    ack_details!(p.properties),
                )?,
                v5::Packet::SubAck(p) => status(
                    s,
                    mask,
                    "subscribe",
                    if p.return_codes
                        .iter()
                        .any(|v| !matches!(v, v5::SubscribeReasonCode::Success(_)))
                    {
                        "rejected"
                    } else {
                        "acknowledged"
                    },
                    Some(p.pkid),
                    p.return_codes.iter().map(|c| format!("{c:?}")).collect(),
                    ack_details!(p.properties),
                )?,
                v5::Packet::UnsubAck(p) => status(
                    s,
                    mask,
                    "unsubscribe",
                    if p.reasons.iter().any(|v| {
                        !matches!(
                            v,
                            v5::UnsubAckReason::Success | v5::UnsubAckReason::NoSubscriptionExisted
                        )
                    }) {
                        "rejected"
                    } else {
                        "acknowledged"
                    },
                    Some(p.pkid),
                    p.reasons.iter().map(|c| format!("{c:?}")).collect(),
                    ack_details!(p.properties),
                )?,
                v5::Packet::Disconnect(p) => {
                    status(s,mask,"disconnect","received",None,vec![format!("{:?}",p.reason_code)],p.properties.map_or(json!({}),|p|json!({"reason_string":p.reason_string,"session_expiry_interval":p.session_expiry_interval,"server_reference":p.server_reference,"user_properties":p.user_properties})))?;
                    bail!("MQTT broker disconnected");
                }
                _ => status(
                    s,
                    mask,
                    "packet",
                    "received",
                    None,
                    vec![],
                    json!({"type":match *p {v5::Packet::PingReq(_)=>"ping_request",v5::Packet::PingResp(_)=>"ping_response",v5::Packet::Auth(_)=>"auth",_=>"unexpected_packet"},"packet_data_withheld":true}),
                )?,
            },
        }
        Ok(None)
    }
}
pub(crate) async fn run(
    session: Arc<Session>,
    request: RequestSpec,
    policy: NetworkPolicy,
    mut commands: mpsc::Receiver<Command>,
    mask: PrivacyMask,
) -> Result<String> {
    let base_mask = mask;
    let mask_session = session.clone();
    let mask: PrivacyMask = Arc::new(move |text| {
        let mut masked = base_mask(text);
        let c = mask_session.mqtt.lock().unwrap();
        if c.withhold {
            return "[REDACTED: MQTT privacy limit]".into();
        }
        for secret in &c.secrets {
            masked = masked.replace(secret, "[REDACTED]");
            if masked.len() > MAX_MESSAGE {
                return "[REDACTED: MQTT privacy limit]".into();
            }
        }
        masked
    });
    moleapi_core::validate_request(&request, false)?;
    let Protocol::Mqtt { config: c } = &request.protocol else {
        unreachable!()
    };
    let url = mqtt_url(&request.url)?;
    {
        let mut control = session.mqtt.lock().unwrap();
        for p in &c.user_properties {
            if p.secret {
                control.capture(&p.value);
            }
        }
        if let Some(w) = &c.will {
            control.capture_message(&w.message)?;
        }
        for sub in &c.subscriptions {
            if sub.filter_secret {
                control.capture(&sub.filter);
            }
            for p in &sub.user_properties {
                if p.secret {
                    control.capture(&p.value);
                }
            }
        }
    }
    let budget = Arc::new(rumqttc::TrafficBudget::new(MAX_INPUT as u64, 10_000));
    let mut sdk = Sdk::new(&request, c)?;
    let mut runtime = Runtime {
        packets: 0,
        connects: 0,
        pending: VecDeque::new(),
        inflight: HashMap::new(),
        subscriptions: VecDeque::new(),
    };
    let mut attempts = 0u8;
    let connect_timeout = request.network.as_deref().map_or(request.timeout_ms, |c| {
        c.connect_timeout_ms.min(request.timeout_ms)
    });
    let connector: Option<rumqttc::SocketConnector> = request.network.clone().map(|network| {
        let endpoint = url.clone();
        let verify = request.verify_tls;
        Arc::new(move || {
            let endpoint = endpoint.clone();
            let network = network.clone();
            Box::pin(async move {
                let socket = tokio::time::timeout(
                    Duration::from_millis(connect_timeout),
                    moleapi_core::connect_request_socket(&endpoint, policy, verify, &network, true),
                )
                .await
                .map_err(|_| {
                    std::io::Error::new(std::io::ErrorKind::TimedOut, "MQTT socket timed out")
                })?
                .map_err(std::io::Error::other)?;
                Ok(Box::new(socket) as Box<dyn rumqttc::AsyncReadWrite>)
            }) as rumqttc::SocketFuture
        }) as rumqttc::SocketConnector
    });
    let mut connecting = true;
    loop {
        if connecting {
            let destinations = if connector.is_some() {
                Vec::new()
            } else {
                tokio::select! {biased;_ = session.cancel.cancelled()=>return Ok("MQTT closed before connecting".into()), result=tokio::time::timeout(Duration::from_millis(request.timeout_ms),moleapi_core::checked_destination(&url,policy))=>result.context("MQTT DNS timed out")??}
            };
            sdk.pin(
                destinations,
                connect_timeout,
                budget.clone(),
                connector.clone(),
            );
        }
        while !connecting && !runtime.subscriptions.is_empty() {
            if sdk
                .subscribe(runtime.subscriptions.front().unwrap())
                .is_err()
            {
                break;
            }
            runtime.subscriptions.pop_front();
        }
        let poll = async {
            if connecting {
                tokio::time::timeout(Duration::from_millis(connect_timeout), sdk.poll())
                    .await
                    .context("MQTT connection timed out")?
            } else {
                sdk.poll().await
            }
        };
        enum Action {
            Cancel,
            Command(Option<Command>),
            Event(Result<SdkEvent>),
        }
        let action = tokio::select! {_=session.cancel.cancelled()=>Action::Cancel,command=commands.recv(), if !connecting =>Action::Command(command),event=poll=>Action::Event(event)};
        {
            let mut record = session.record.lock().unwrap();
            record.summary.received_bytes = budget.received();
            record.summary.sent_bytes = budget.sent();
        }
        match action {
            Action::Cancel | Action::Command(None) => {
                let graceful = async {
                    sdk.disconnect()?;
                    loop {
                        if matches!(sdk.poll().await?, SdkEvent::Outgoing(Outgoing::Disconnect)) {
                            break;
                        }
                    }
                    Ok::<(), anyhow::Error>(())
                };
                let done = tokio::time::timeout(Duration::from_millis(500), graceful)
                    .await
                    .is_ok_and(|r| r.is_ok());
                status(
                    &session,
                    &mask,
                    "disconnect",
                    if done { "sent" } else { "transport_closed" },
                    None,
                    vec![],
                    json!({"graceful":done}),
                )?;
                return Ok(if done {
                    "MQTT gracefully disconnected"
                } else {
                    "MQTT transport closed before graceful disconnect completed"
                }
                .into());
            }
            Action::Command(Some(Command::Mqtt(command))) => match command {
                MqttCommand::Abort => {
                    return Ok("MQTT transport aborted (ungraceful; broker Will applies)".into());
                }
                MqttCommand::Publish(m) => {
                    if runtime.pending.len() + runtime.inflight.len() >= 48 {
                        status(
                            &session,
                            &mask,
                            "publish",
                            "rejected",
                            None,
                            vec![],
                            json!({"reason":"MQTT outstanding publish limit reached (48)"}),
                        )?;
                        continue;
                    }
                    match sdk.publish(&m) {
                        Ok(()) => {
                            runtime.pending.push_back(m);
                            status(
                                &session,
                                &mask,
                                "publish",
                                "queued",
                                None,
                                vec![],
                                json!({}),
                            )?;
                        }
                        Err(e) => status(
                            &session,
                            &mask,
                            "publish",
                            "rejected",
                            None,
                            vec![],
                            json!({"reason":e.to_string()}),
                        )?,
                    }
                }
                MqttCommand::Subscribe(s) => match sdk.subscribe(&s) {
                    Ok(()) => {
                        status(
                            &session,
                            &mask,
                            "subscribe",
                            "queued",
                            None,
                            vec![],
                            json!({}),
                        )?;
                    }
                    Err(e) => status(
                        &session,
                        &mask,
                        "subscribe",
                        "rejected",
                        None,
                        vec![],
                        json!({"reason":e.to_string()}),
                    )?,
                },
                MqttCommand::Unsubscribe(filter) => match sdk.unsubscribe(&filter) {
                    Ok(()) => {
                        status(
                            &session,
                            &mask,
                            "unsubscribe",
                            "queued",
                            None,
                            vec![],
                            json!({}),
                        )?;
                    }
                    Err(e) => status(
                        &session,
                        &mask,
                        "unsubscribe",
                        "rejected",
                        None,
                        vec![],
                        json!({"reason":e.to_string()}),
                    )?,
                },
            },
            Action::Command(Some(_)) => bail!("Invalid MQTT command"),
            Action::Event(Ok(event)) => {
                if let Some(present) = runtime.process(&session, &mask, event)? {
                    runtime.connects += 1;
                    connecting = false;
                    sdk.resumed(c);
                    session.open(Handshake {
                        status: 200,
                        headers: vec![],
                    })?;
                    if !present {
                        if runtime.connects > 1
                            && (!runtime.inflight.is_empty() || !runtime.pending.is_empty())
                        {
                            status(
                                &session,
                                &mask,
                                "publish",
                                "discarded",
                                None,
                                vec![],
                                json!({"count":runtime.inflight.len()+runtime.pending.len(),"reason":"Broker returned no persistent session; SDK discarded pending packets"}),
                            )?;
                            runtime.pending.clear();
                        }
                        runtime.inflight.clear();
                        let subscriptions: Vec<_> = session
                            .mqtt
                            .lock()
                            .unwrap()
                            .subscriptions
                            .values()
                            .cloned()
                            .collect();
                        runtime.subscriptions.extend(subscriptions);
                    }
                }
            }
            Action::Event(Err(error)) => {
                // Preserve SDK notifications buffered immediately before a transport/state error,
                // including broker DISCONNECT properties and actual negative acknowledgements.
                for event in sdk.buffered() {
                    let _ = runtime.process(&session, &mask, event)?;
                }
                let text = error_text(&error, &mask);
                status(
                    &session,
                    &mask,
                    "connect",
                    "error",
                    None,
                    vec![],
                    json!({"reason":text}),
                )?;
                // Auth/protocol refusals are terminal, regardless of automatic reconnect.
                let refused = matches!(
                    error.downcast_ref::<rumqttc::ConnectionError>(),
                    Some(rumqttc::ConnectionError::ConnectionRefused(_))
                ) || matches!(
                    error.downcast_ref::<rumqttc::v5::ConnectionError>(),
                    Some(rumqttc::v5::ConnectionError::ConnectionRefused(_))
                );
                ensure!(
                    c.reconnect.enabled
                        && attempts < c.reconnect.max_attempts
                        && !refused
                        && !text.contains("MQTT traffic budget exceeded"),
                    "{text}"
                );
                attempts += 1;
                connecting = true;
                {
                    let mut r = session.record.lock().unwrap();
                    r.summary.state = SessionState::Connecting;
                }
                status(
                    &session,
                    &mask,
                    "reconnect",
                    "waiting",
                    None,
                    vec![],
                    json!({"attempt":attempts,"maximum":c.reconnect.max_attempts}),
                )?;
                tokio::select! {biased;_=session.cancel.cancelled()=>return Ok("MQTT reconnect cancelled".into()),_=tokio::time::sleep(Duration::from_millis(c.reconnect.delay_ms))=>{}}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn websocket_header_modifier_separates_mqtt_connect_credentials_from_http() {
        let request:RequestSpec=serde_json::from_value(json!({"protocol":{"kind":"mqtt"},"id":"r","name":"r","method":"GET","url":"ws://localhost:18894/mqtt?original=yes","description":"","query":[{"id":"q","key":"custom","value":"value with space","enabled":true}],"headers":[{"id":"h","key":"X-Custom","value":"header-value","enabled":true}],"body_kind":"none","body":"","auth":{"kind":"basic","token":"","username":"connect-only","password":"mqtt-secret"},"timeout_ms":1000,"follow_redirects":false,"verify_tls":true,"assertions":[],"examples":[]})).unwrap();
        for version in ["3.1.1", "5"] {
            let c = MqttConfig {
                version: version.into(),
                ..Default::default()
            };
            let sdk = Sdk::new(&request, &c).unwrap();
            let (host, modifier) = match sdk {
                Sdk::V3(_, e) => (
                    e.mqtt_options.broker_address().0,
                    e.mqtt_options.request_modifier().unwrap(),
                ),
                Sdk::V5(_, e) => (
                    e.options.broker_address().0,
                    e.options.request_modifier().unwrap(),
                ),
            };
            assert!(host.contains("original=yes&custom=value+with+space"));
            let handshake = tokio_tungstenite::tungstenite::http::Request::builder()
                .uri(host)
                .body(())
                .unwrap();
            let handshake = modifier(handshake).await;
            assert_eq!(handshake.headers()["x-custom"], "header-value");
            assert!(!handshake.headers().contains_key("authorization"));
        }
    }
    #[tokio::test]
    async fn subscription_admission_reserves_queued_intents_and_never_exceeds_64() {
        let r:RequestSpec=serde_json::from_value(json!({"protocol":{"kind":"mqtt"},"id":"r","name":"r","method":"GET","url":"mqtt://localhost:18891","description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":false,"verify_tls":true,"assertions":[],"examples":[]})).unwrap();
        let manager = crate::SessionManager::new();
        let summary = manager
            .register(
                "owner",
                "w",
                &r,
                "safe".into(),
                crate::PreparedFeedback::default(),
            )
            .unwrap();
        let c = MqttConfig {
            subscriptions: (0..63)
                .map(|i| MqttSubscription {
                    filter: format!("initial/{i}"),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };
        manager
            .configure_mqtt(
                "owner",
                &summary.id,
                c,
                moleapi_core::Environment {
                    id: String::new(),
                    name: String::new(),
                    variables: vec![],
                },
                Arc::new(str::to_owned),
            )
            .unwrap();
        let session = manager.owned("owner", &summary.id).unwrap();
        session.record.lock().unwrap().summary.state = SessionState::Open;
        manager
            .send(
                "owner",
                &summary.id,
                SendMessage::MqttSubscribe {
                    subscription: MqttSubscription {
                        filter: "queued/64".into(),
                        ..Default::default()
                    },
                },
            )
            .unwrap();
        assert!(
            manager
                .send(
                    "owner",
                    &summary.id,
                    SendMessage::MqttSubscribe {
                        subscription: MqttSubscription {
                            filter: "queued/65".into(),
                            ..Default::default()
                        }
                    }
                )
                .unwrap_err()
                .to_string()
                .contains("64")
        );
        manager
            .send(
                "owner",
                &summary.id,
                SendMessage::MqttUnsubscribe {
                    filter: "queued/64".into(),
                },
            )
            .unwrap();
        manager
            .send(
                "owner",
                &summary.id,
                SendMessage::MqttSubscribe {
                    subscription: MqttSubscription {
                        filter: "queued/replacement".into(),
                        ..Default::default()
                    },
                },
            )
            .unwrap();
        assert_eq!(session.mqtt.lock().unwrap().subscriptions.len(), 64);
        manager.close_owner("owner").await;
    }
    #[tokio::test]
    async fn unexpected_sdk_packet_diagnostics_withhold_binary_authentication_data() {
        let bytes = b"\0private-secret\0".to_vec();
        let mask: PrivacyMask = Arc::new(|s| s.replace("private-secret", "[REDACTED]"));
        let error = anyhow::Error::new(rumqttc::ConnectionError::NotConnAck(
            rumqttc::Packet::Publish(rumqttc::Publish::new(
                "topic",
                rumqttc::QoS::AtMostOnce,
                bytes.clone(),
            )),
        ));
        assert_eq!(
            error_text(&error, &mask),
            "MQTT broker sent a packet other than CONNACK; packet data withheld"
        );
        let error = anyhow::Error::new(rumqttc::v5::ConnectionError::NotConnAck(Box::new(
            v5::Packet::Publish(v5::Publish::new(
                "topic",
                rumqttc::v5::mqttbytes::QoS::AtMostOnce,
                bytes.clone(),
                None,
            )),
        )));
        assert_eq!(
            error_text(&error, &mask),
            "MQTT broker sent a packet other than CONNACK; packet data withheld"
        );
        let r:RequestSpec=serde_json::from_value(json!({"protocol":{"kind":"mqtt"},"id":"r","name":"r","method":"GET","url":"mqtt://localhost:18891","description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":false,"verify_tls":true,"assertions":[],"examples":[]})).unwrap();
        let manager = crate::SessionManager::new();
        let summary = manager
            .register(
                "owner",
                "w",
                &r,
                "safe".into(),
                crate::PreparedFeedback::default(),
            )
            .unwrap();
        let session = manager.owned("owner", &summary.id).unwrap();
        let mut properties = v5::ConnectProperties::new();
        properties.authentication_data = Some(bytes.into());
        let packet = v5::Packet::Connect(
            v5::Connect {
                keep_alive: 60,
                client_id: "unexpected".into(),
                clean_start: true,
                properties: Some(properties),
            },
            None,
            None,
        );
        let mut runtime = Runtime {
            packets: 0,
            connects: 0,
            pending: VecDeque::new(),
            inflight: HashMap::new(),
            subscriptions: VecDeque::new(),
        };
        runtime
            .process(&session, &mask, SdkEvent::V5(Box::new(packet)))
            .unwrap();
        let batch = manager.events("owner", &summary.id, 0).unwrap();
        let status = batch.events.last().unwrap();
        let EventMessage::MqttStatus { details, .. } = &status.message else {
            panic!("expected status")
        };
        assert_eq!(
            details,
            &json!({"type":"unexpected_packet","packet_data_withheld":true})
        );
        manager.close_owner("owner").await;
    }
}
