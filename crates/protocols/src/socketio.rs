//! Socket.IO adapters. Engine.IO/Socket.IO framing, heartbeat and ACK routing belong to the SDK.
use crate::*;
use anyhow::{Context, bail};
use bytes::Bytes;
use futures_util::StreamExt;
use moleapi_core::{checked_destination, request_headers, validate_socketio_event};
use rust_socketio::{
    asynchronous::ClientBuilder,
    packet::{Packet, PacketId},
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use tokio_tungstenite::{
    Connector, client_async_tls_with_config,
    tungstenite::{client::IntoClientRequest, protocol::WebSocketConfig},
};

const MAX_ACKS: usize = 32;
const ACK_LIFETIME: Duration = Duration::from_secs(120);
#[derive(Default)]
pub(crate) struct Control {
    listeners: HashSet<String>,
    incoming: HashMap<String, (i32, Instant)>,
    outgoing: HashSet<String>,
    emitted: usize,
}
pub(crate) enum SocketioCommand {
    Emit {
        event: String,
        arguments: Vec<Value>,
        attachments: Vec<Bytes>,
        ack_id: Option<String>,
        timeout_ms: u64,
    },
    Listen {
        event: String,
        enabled: bool,
    },
    Ack {
        token: String,
        id: i32,
        arguments: Vec<Value>,
        attachments: Vec<Bytes>,
    },
}
fn arguments(
    source: &str,
    encoded: &[String],
    environment: Option<&moleapi_core::Environment>,
) -> Result<(Vec<Value>, Vec<Bytes>, usize)> {
    ensure!(
        source.len() <= MAX_MESSAGE && encoded.len() <= MAX_ACKS,
        "Socket.IO arguments exceed limit"
    );
    ensure!(
        encoded.iter().map(String::len).sum::<usize>() <= MAX_MESSAGE.div_ceil(3) * 4,
        "Socket.IO binary arguments exceed limit"
    );
    let source = environment
        .map(|env| moleapi_core::resolve_grpc_source(source, env))
        .transpose()
        .map_err(|_| anyhow::anyhow!("Invalid or unresolved Socket.IO JSON arguments"))?
        .unwrap_or_else(|| source.into());
    let arguments: Vec<Value> =
        serde_json::from_str(&source).context("Socket.IO arguments must be a JSON array")?;
    ensure!(
        arguments.len() <= 256,
        "Socket.IO argument count exceeds 256"
    );
    let attachments = encoded
        .iter()
        .map(|encoded| {
            let encoded = environment
                .map(|env| moleapi_core::resolve_value(encoded, env))
                .transpose()
                .map_err(|_| anyhow::anyhow!("Unresolved Socket.IO binary argument"))?
                .unwrap_or_else(|| encoded.clone());
            ensure!(
                encoded.len() <= MAX_MESSAGE.div_ceil(3) * 4,
                "Socket.IO binary argument exceeds limit"
            );
            Ok(Bytes::from(
                STANDARD
                    .decode(encoded)
                    .context("Invalid Socket.IO attachment base64")?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let size = source.len() + attachments.iter().map(Bytes::len).sum::<usize>();
    ensure!(size <= MAX_MESSAGE, "Socket.IO arguments exceed 1 MiB");
    // Validate SDK placeholder references, including nested arguments; do not parse wire syntax.
    fn references(value: &Value, count: usize, used: &mut HashSet<usize>) -> Result<()> {
        match value {
            Value::Object(map) if map.contains_key("_placeholder") => {
                ensure!(
                    map.get("_placeholder") == Some(&Value::Bool(true)) && map.len() == 2,
                    "Invalid Socket.IO binary placeholder"
                );
                let index = map
                    .get("num")
                    .and_then(Value::as_u64)
                    .and_then(|n| usize::try_from(n).ok())
                    .context("Invalid Socket.IO attachment index")?;
                ensure!(index < count, "Socket.IO attachment index out of range");
                used.insert(index);
            }
            Value::Object(map) => {
                for value in map.values() {
                    references(value, count, used)?;
                }
            }
            Value::Array(values) => {
                for value in values {
                    references(value, count, used)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    let mut used = HashSet::new();
    for value in &arguments {
        references(value, attachments.len(), &mut used)?;
    }
    ensure!(
        used.len() == attachments.len(),
        "Every Socket.IO attachment requires a placeholder"
    );
    Ok((arguments, attachments, size))
}
pub(crate) fn send(session: &Session, message: SendMessage) -> Result<()> {
    let environment = session.grpc_environment.lock().unwrap().clone();
    let mask = session.grpc_mask.lock().unwrap().clone();
    let resolve = |text: &str| -> Result<String> {
        environment
            .as_ref()
            .map(|env| moleapi_core::resolve_value(text, env))
            .transpose()
            .map_err(|_| anyhow::anyhow!("Unresolved Socket.IO event name"))
            .map(|value| value.unwrap_or_else(|| text.into()))
    };
    let mut record = session.record.lock().unwrap();
    ensure!(
        record.summary.protocol == "socketio"
            && record.summary.state == SessionState::Open
            && !session.cancel.is_cancelled(),
        "Expected open Socket.IO session"
    );
    let mut control = session.socketio.lock().unwrap();
    control
        .incoming
        .retain(|_, (_, deadline)| *deadline > Instant::now());
    let (command, size, outgoing, incoming) = match message {
        SendMessage::SocketioEmit {
            event,
            arguments_source,
            attachments_base64,
            ack_id,
            ack_timeout_ms,
        } => {
            let event = resolve(&event)?;
            validate_socketio_event(&event)?;
            ensure!(
                (1..=120_000).contains(&ack_timeout_ms),
                "Socket.IO ACK timeout must be 1–120000 ms"
            );
            ensure!(
                control.emitted < 10_000,
                "Socket.IO sent-event limit reached (10000)"
            );
            if let Some(id) = &ack_id {
                ensure!(
                    !id.is_empty() && id.len() <= 128 && !id.chars().any(char::is_control),
                    "Invalid Socket.IO ACK correlation ID"
                );
                ensure!(
                    mask.as_ref().is_none_or(|mask| mask(id) == *id),
                    "Socket.IO ACK correlation ID contains a private value"
                );
                ensure!(
                    control.outgoing.len() < MAX_ACKS,
                    "Socket.IO pending ACK capacity reached (32)"
                );
                ensure!(
                    !control.outgoing.contains(id),
                    "Socket.IO ACK correlation ID is already pending"
                );
            }
            let (arguments, attachments, size) =
                arguments(&arguments_source, &attachments_base64, environment.as_ref())?;
            (
                SocketioCommand::Emit {
                    event,
                    arguments,
                    attachments,
                    ack_id: ack_id.clone(),
                    timeout_ms: ack_timeout_ms,
                },
                size,
                ack_id,
                None,
            )
        }
        SendMessage::SocketioListen { event, enabled } => {
            let event = resolve(&event)?;
            validate_socketio_event(&event)?;
            ensure!(
                !enabled || control.listeners.contains(&event) || control.listeners.len() < 64,
                "Socket.IO listener capacity reached (64)"
            );
            (SocketioCommand::Listen { event, enabled }, 0, None, None)
        }
        SendMessage::SocketioAck {
            ack_id,
            arguments_source,
            attachments_base64,
        } => {
            let (id, _) = control
                .incoming
                .get(&ack_id)
                .context("Socket.IO server ACK token is expired, unknown or already answered")?;
            let (arguments, attachments, size) =
                arguments(&arguments_source, &attachments_base64, environment.as_ref())?;
            (
                SocketioCommand::Ack {
                    token: ack_id.clone(),
                    id: *id,
                    arguments,
                    attachments,
                },
                size,
                None,
                Some(ack_id),
            )
        }
        _ => bail!("Expected Socket.IO command"),
    };
    ensure!(
        record.input + size <= MAX_INPUT,
        "Session input limit reached (20 MiB)"
    );
    // State changes happen only after queue admission; callers cannot reserve unbounded callbacks.
    let listener = if let SocketioCommand::Listen { event, enabled } = &command {
        Some((event.clone(), *enabled))
    } else {
        None
    };
    let emitting = matches!(command, SocketioCommand::Emit { .. });
    session
        .commands
        .try_send(Command::Socketio(command))
        .map_err(|_| anyhow::anyhow!("Session command queue is full or closed"))?;
    if let Some((event, enabled)) = listener {
        if enabled {
            control.listeners.insert(event);
        } else {
            control.listeners.remove(&event);
        }
    }
    record.input += size;
    control.emitted += usize::from(emitting);
    if let Some(id) = outgoing {
        control.outgoing.insert(id);
    }
    if let Some(id) = incoming {
        control.incoming.remove(&id);
    }
    Ok(())
}
fn scrub(value: &mut Value, mask: &dyn Fn(&str) -> String) {
    match value {
        Value::String(text) => *text = mask(text),
        Value::Number(number) if mask(&number.to_string()) != number.to_string() => {
            *value = Value::String("[REDACTED]".into())
        }
        Value::Array(values) => {
            for value in values {
                scrub(value, mask);
            }
        }
        Value::Object(map) => {
            let original = std::mem::take(map);
            for (key, mut value) in original {
                scrub(&mut value, mask);
                map.insert(mask(&key), value);
            }
        }
        _ => {}
    }
}
fn display_payload(
    mut arguments: Vec<Value>,
    attachments: &[Bytes],
    mask: &dyn Fn(&str) -> String,
) -> (Vec<Value>, Vec<String>) {
    for value in &mut arguments {
        scrub(value, mask);
    }
    let attachments = attachments
        .iter()
        .map(|bytes| {
            let text = String::from_utf8_lossy(bytes);
            if mask(&text) != text || mask(&STANDARD.encode(bytes)) != STANDARD.encode(bytes) {
                STANDARD.encode(b"[REDACTED]")
            } else {
                STANDARD.encode(bytes)
            }
        })
        .collect();
    (arguments, attachments)
}
fn packet_arguments(packet: &Packet) -> Result<(Vec<Value>, Vec<Bytes>)> {
    let arguments: Vec<Value> = serde_json::from_str(packet.data.as_deref().unwrap_or("[]"))
        .context("Invalid Socket.IO SDK argument array")?;
    let attachments = packet.attachments.clone().unwrap_or_default();
    ensure!(
        arguments.len() <= 257
            && packet.data.as_ref().map_or(0, String::len)
                + attachments.iter().map(Bytes::len).sum::<usize>()
                <= MAX_MESSAGE,
        "Socket.IO incoming arguments exceed limit"
    );
    Ok((arguments, attachments))
}
pub(crate) async fn run(
    session: Arc<Session>,
    request: RequestSpec,
    policy: NetworkPolicy,
    mut commands: mpsc::Receiver<Command>,
    mask: PrivacyMask,
) -> Result<String> {
    moleapi_core::validate_request(&request, false)?;
    let Protocol::Socketio {
        namespace,
        path,
        auth_source,
        listeners,
        ..
    } = &request.protocol
    else {
        unreachable!()
    };
    session.socketio.lock().unwrap().listeners = listeners.iter().cloned().collect();
    let mut url = moleapi_core::protocol_url(&request.url, true)?;
    url.set_path(path);
    for pair in request.query.iter().filter(|pair| pair.enabled) {
        url.query_pairs_mut().append_pair(&pair.key, &pair.value);
    }
    let mut wire_url = url.clone();
    wire_url
        .query_pairs_mut()
        .append_pair("EIO", "4")
        .append_pair("transport", "websocket");
    let deadline = tokio::time::Instant::now() + Duration::from_millis(request.timeout_ms);
    let connect = async {
        let addresses = checked_destination(&url, policy).await?;
        let tcp = tokio::net::TcpStream::connect(addresses.as_slice())
            .await
            .context("Socket.IO pinned connection failed")?;
        let mut handshake_request = wire_url.as_str().into_client_request()?;
        handshake_request
            .headers_mut()
            .extend(request_headers(&request)?);
        let config = WebSocketConfig {
            max_message_size: Some(MAX_MESSAGE),
            max_frame_size: Some(MAX_MESSAGE),
            max_write_buffer_size: MAX_MESSAGE * 2,
            write_buffer_size: 0,
            ..Default::default()
        };
        let tls = native_tls::TlsConnector::builder()
            .danger_accept_invalid_certs(!request.verify_tls)
            .danger_accept_invalid_hostnames(!request.verify_tls)
            .build()?;
        // Original URL controls Host and SNI; the stream uses only policy-checked pinned addresses.
        let (stream, response) = client_async_tls_with_config(
            handshake_request,
            tcp,
            Some(config),
            Some(Connector::NativeTls(tls)),
        )
        .await?;
        let handshake = Handshake {
            status: response.status().as_u16(),
            headers: response
                .headers()
                .iter()
                .map(|(name, value)| moleapi_core::Pair {
                    id: uuid::Uuid::new_v4().to_string(),
                    key: mask(name.as_str()),
                    value: if matches!(
                        name.as_str(),
                        "set-cookie" | "authorization" | "proxy-authenticate"
                    ) {
                        "[REDACTED]".into()
                    } else {
                        mask(value.to_str().unwrap_or("[binary]"))
                    },
                    enabled: true,
                    secret: None,
                    local_value: None,
                })
                .collect(),
        };
        session.record.lock().unwrap().summary.handshake = Some(handshake.clone());
        let engine = rust_engineio::asynchronous::ClientBuilder::new(url.clone())
            .build_websocket_on(stream)
            .await?;
        let client = ClientBuilder::new(url.to_string())
            .namespace(namespace)
            .auth(serde_json::from_str::<Value>(auth_source)?)
            .reconnect(false)
            .connect_with_engineio(engine)
            .await?;
        Ok::<_, anyhow::Error>((client, handshake))
    };
    let (client, handshake) = tokio::select! {
        biased;
        _ = session.cancel.cancelled() => return Ok("Session closed while connecting".into()),
        result = tokio::time::timeout_at(deadline, connect) => result.context("Socket.IO connect timed out")??,
    };
    let mut stream = client.as_stream().await;
    let connected = tokio::select! {
        biased;
        _ = session.cancel.cancelled() => return Ok("Session closed while connecting".into()),
        result = tokio::time::timeout_at(deadline, stream.next()) => result.context("Socket.IO namespace connect timed out")?.context("Socket.IO closed before namespace connected")??,
    };
    if connected.packet_type == PacketId::ConnectError {
        bail!(
            "Socket.IO namespace rejected: {}",
            mask(connected.data.as_deref().unwrap_or("Unknown error"))
        );
    }
    ensure!(
        connected.packet_type == PacketId::Connect && connected.nsp == *namespace,
        "Socket.IO namespace did not connect"
    );
    session.open(handshake)?;
    let mut acks = tokio::task::JoinSet::new();
    let mut expiry = tokio::time::interval(Duration::from_secs(1));
    let result = async { loop {
        tokio::select! {
            biased;
            _ = session.cancel.cancelled() => break Ok("Session closed by client".into()),
            Some(result) = acks.join_next(), if !acks.is_empty() => {
                let (ack_id, result) = result.context("Socket.IO ACK task stopped")?;
                session.socketio.lock().unwrap().outgoing.remove(&ack_id);
                let event = match result {
                    Ok(packet) => {
                        let (arguments, attachments) = packet_arguments(&packet)?;
                        let (arguments, attachments_base64) = display_payload(arguments, &attachments, &*mask);
                        EventMessage::SocketioAck { ack_id, status: "ok".into(), arguments, attachments_base64, error: None }
                    }
                    Err(error) => EventMessage::SocketioAck { ack_id, status: if matches!(error, rust_socketio::Error::AckTimeout) { "timeout" } else { "error" }.into(), arguments: vec![], attachments_base64: vec![], error: Some(mask(&error.to_string())) },
                };
                session.event("incoming", event)?;
            }
            _ = expiry.tick() => {
                let expired: Vec<_> = { let mut control = session.socketio.lock().unwrap(); let ids = control.incoming.iter().filter(|(_, (_, deadline))| *deadline <= Instant::now()).map(|(id, _)| id.clone()).collect::<Vec<_>>(); for id in &ids { control.incoming.remove(id); } ids };
                for ack_id in expired { session.event("system", EventMessage::SocketioAck { ack_id, status: "timeout".into(), arguments: vec![], attachments_base64: vec![], error: Some("Server ACK reply deadline expired".into()) })?; }
            }
            command = commands.recv() => {
                let Some(Command::Socketio(command)) = command else { break Ok("Session command channel closed".into()) };
                match command {
                    SocketioCommand::Listen { event, enabled } => {
                        session.event("system", EventMessage::SocketioEvent { event: mask(&event), arguments: vec![serde_json::json!({"listening": enabled})], attachments_base64: vec![], ack_id: None })?;
                    }
                    SocketioCommand::Emit { event, arguments, attachments, ack_id, timeout_ms } => {
                        let (display_arguments, attachments_base64) = display_payload(arguments.clone(), &attachments, &*mask);
                        let size = serde_json::to_vec(&arguments)?.len() + attachments.iter().map(Bytes::len).sum::<usize>();
                        session.record.lock().unwrap().summary.sent_bytes += size as u64;
                        session.event("outgoing", EventMessage::SocketioEvent { event: mask(&event), arguments: display_arguments, attachments_base64, ack_id: ack_id.clone() })?;
                        if let Some(ack_id) = ack_id {
                            let pending = tokio::select! { biased;
                                _ = session.cancel.cancelled() => break Ok("Session closed while emitting".into()),
                                result = client.start_arguments_ack(event, arguments, attachments, Duration::from_millis(timeout_ms)) => result,
                            };
                            match pending {
                                Ok(pending) => { acks.spawn(async move { (ack_id, pending.wait().await) }); },
                                Err(error) => {
                                    session.socketio.lock().unwrap().outgoing.remove(&ack_id);
                                    session.event("system", EventMessage::SocketioAck { ack_id, status: if matches!(error, rust_socketio::Error::AckTimeout) { "timeout" } else { "error" }.into(), arguments: vec![], attachments_base64: vec![], error: Some(mask(&error.to_string())) })?;
                                    return Err(anyhow::anyhow!(error));
                                }
                            }
                        } else {
                            tokio::select! { biased;
                                _ = session.cancel.cancelled() => break Ok("Session closed while emitting".into()),
                                result = tokio::time::timeout(Duration::from_secs(5), client.emit_arguments(event, arguments, attachments)) => result.context("Socket.IO emit timed out")??,
                            }
                        }
                    }
                    SocketioCommand::Ack { token, id, arguments, attachments } => {
                        let size = serde_json::to_vec(&arguments)?.len() + attachments.iter().map(Bytes::len).sum::<usize>();
                        let (display_arguments, attachments_base64) = display_payload(arguments.clone(), &attachments, &*mask);
                        tokio::select! { biased;
                            _ = session.cancel.cancelled() => break Ok("Session closed while replying to ACK".into()),
                            result = tokio::time::timeout(Duration::from_secs(5), client.reply_ack(id, arguments, attachments)) => result.context("Socket.IO ACK reply timed out")??,
                        }
                        session.record.lock().unwrap().summary.sent_bytes += size as u64;
                        session.event("outgoing", EventMessage::SocketioAck { ack_id: token, status: "ok".into(), arguments: display_arguments, attachments_base64, error: None })?;
                    }
                }
            }
            packet = stream.next() => {
                let Some(packet) = packet else { break Ok("Socket.IO peer ended stream".into()) };
                let packet = packet.context("Socket.IO SDK read failed")?;
                session.received(packet.data.as_ref().map_or(0, String::len) + packet.attachments.as_ref().map_or(0, |attachments| attachments.iter().map(Bytes::len).sum::<usize>()))?;
                // ACK payloads are delivered once by SDK routing above.
                if matches!(packet.packet_type, PacketId::Ack | PacketId::BinaryAck) { continue; }
                if packet.nsp != *namespace { continue; }
                match packet.packet_type {
                    PacketId::Disconnect => break Ok("Socket.IO namespace disconnected by server".into()),
                    PacketId::ConnectError => bail!("Socket.IO error: {}", mask(packet.data.as_deref().unwrap_or("Unknown error"))),
                    PacketId::Event | PacketId::BinaryEvent => {
                        let (mut arguments, attachments) = packet_arguments(&packet)?;
                        ensure!(!arguments.is_empty(), "Socket.IO event has no name");
                        let event = arguments.remove(0).as_str().context("Socket.IO event name must be a string")?.to_owned();
                        ensure!(event.len() <= 256, "Socket.IO event name exceeds limit");
                        let ack_id = {
                            let mut control = session.socketio.lock().unwrap();
                            if !control.listeners.contains(&event) { continue; }
                            if let Some(id) = packet.id {
                                ensure!(control.incoming.len() < MAX_ACKS, "Socket.IO server ACK capacity reached (32)");
                                ensure!(!control.incoming.values().any(|(pending, _)| *pending == id), "Duplicate Socket.IO server ACK identifier");
                                let token = uuid::Uuid::new_v4().to_string();
                                control.incoming.insert(token.clone(), (id, Instant::now() + ACK_LIFETIME));
                                Some(token)
                            } else { None }
                        };
                        let (arguments, attachments_base64) = display_payload(arguments, &attachments, &*mask);
                        session.event("incoming", EventMessage::SocketioEvent { event: mask(&event), arguments, attachments_base64, ack_id })?;
                    }
                    _ => {},
                }
            }
        }
    } }.await;
    acks.abort_all();
    while acks.join_next().await.is_some() {}
    {
        let mut control = session.socketio.lock().unwrap();
        let pending: Vec<_> = control.outgoing.drain().collect();
        control.incoming.clear();
        drop(control);
        for ack_id in pending {
            let _ = session.event(
                "system",
                EventMessage::SocketioAck {
                    ack_id,
                    status: "error".into(),
                    arguments: vec![],
                    attachments_base64: vec![],
                    error: Some("Socket.IO session ended before acknowledgement completed".into()),
                },
            );
        }
    }
    let _ = tokio::time::timeout(Duration::from_secs(1), client.disconnect()).await;
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    fn session() -> (Arc<SessionManager>, Arc<Session>) {
        let manager = SessionManager::new();
        let mut request: RequestSpec = serde_json::from_value(serde_json::json!({"id":"r","name":"request","method":"GET","url":"ws://example.com","description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":true,"verify_tls":true,"assertions":[],"examples":[]})).unwrap();
        request.protocol = serde_json::from_value(serde_json::json!({"kind":"socketio"})).unwrap();
        let summary = manager
            .register(
                "owner",
                "w",
                &request,
                "safe".into(),
                PreparedFeedback::default(),
            )
            .unwrap();
        let session = manager.owned("owner", &summary.id).unwrap();
        session.record.lock().unwrap().summary.state = SessionState::Open;
        (manager, session)
    }
    #[test]
    fn json_and_attachment_bounds_reject_inconsistent_inputs() {
        assert!(arguments("{}", &[], None).is_err());
        assert!(arguments("[broken", &[], None).is_err());
        assert!(arguments(r#"[{"_placeholder":true,"num":0}]"#, &[], None).is_err());
        assert!(arguments("[]", &["AA==".into()], None).is_err());
        assert!(arguments("[]", &["!invalid".into()], None).is_err());
        assert!(arguments(&format!("[\"{}\"]", "x".repeat(MAX_MESSAGE)), &[], None).is_err());
        assert!(arguments("[]", &vec!["AA==".into(); 33], None).is_err());
        assert!(
            arguments(
                r#"[{"_placeholder":true,"num":0,"extra":1}]"#,
                &["AA==".into()],
                None
            )
            .is_err()
        );
    }
    #[tokio::test]
    async fn bounded_ack_capacity_expiration_and_listener_queue_admission() {
        let (_manager, session) = session();
        for n in 0..32 {
            send(
                &session,
                SendMessage::SocketioEmit {
                    event: "echo".into(),
                    arguments_source: "[]".into(),
                    attachments_base64: vec![],
                    ack_id: Some(n.to_string()),
                    ack_timeout_ms: 1000,
                },
            )
            .unwrap();
        }
        assert!(
            send(
                &session,
                SendMessage::SocketioEmit {
                    event: "echo".into(),
                    arguments_source: "[]".into(),
                    attachments_base64: vec![],
                    ack_id: Some("over".into()),
                    ack_timeout_ms: 1000
                }
            )
            .unwrap_err()
            .to_string()
            .contains("capacity")
        );
        assert!(
            send(
                &session,
                SendMessage::SocketioListen {
                    event: "unadmitted".into(),
                    enabled: true
                }
            )
            .is_err()
        );
        assert!(
            !session
                .socketio
                .lock()
                .unwrap()
                .listeners
                .contains("unadmitted")
        );
        session.socketio.lock().unwrap().incoming.insert(
            "expired".into(),
            (7, Instant::now() - Duration::from_secs(1)),
        );
        assert!(
            send(
                &session,
                SendMessage::SocketioAck {
                    ack_id: "expired".into(),
                    arguments_source: "[]".into(),
                    attachments_base64: vec![]
                }
            )
            .is_err()
        );
        assert!(
            !session
                .socketio
                .lock()
                .unwrap()
                .incoming
                .contains_key("expired")
        );
    }
}
