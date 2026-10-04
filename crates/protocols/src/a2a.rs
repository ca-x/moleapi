//! Typed legacy/current SDK runtime. Only explicitly selected interfaces are passed to SDKs.
use crate::*;
use anyhow::{bail, ensure};
use futures_util::StreamExt;
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use tokio_util::sync::CancellationToken;

pub(crate) fn send(session: &Arc<Session>, message: SendMessage) -> Result<()> {
    ensure!(
        session.record.lock().unwrap().summary.protocol == "a2a",
        "Session is not A2A"
    );
    match &message {
        SendMessage::A2aRequest {
            request_id,
            method,
            params_source,
        } => {
            ensure!(
                !request_id.is_empty()
                    && request_id.len() <= 128
                    && request_id
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c)),
                "Invalid A2A invocation id"
            );
            ensure!(
                method.len() <= 128 && params_source.len() <= MAX_MESSAGE,
                "A2A invocation exceeds limits"
            );
        }
        SendMessage::A2aStop { request_id } => {
            ensure!(request_id.len() <= 128, "Invalid A2A invocation id")
        }
        _ => bail!("Unsupported A2A command"),
    }
    {
        let mut record = session.record.lock().unwrap();
        ensure!(
            record.summary.state == SessionState::Open,
            "A2A session is not open"
        );
        let size = serde_json::to_vec(&message)?.len();
        ensure!(
            record.input.saturating_add(size) <= MAX_INPUT,
            "A2A input budget exceeded"
        );
        record.input += size;
        record.summary.sent_bytes += size as u64;
    }
    session
        .commands
        .try_send(Command::A2a(message))
        .map_err(|_| anyhow::anyhow!("A2A command queue is full or closed"))
}
fn scrub(value: &mut Value, mask: &dyn Fn(&str) -> String) {
    match value {
        Value::Object(items) => {
            let original = std::mem::take(items);
            for (key, mut item) in original {
                scrub(&mut item, mask);
                items.insert(mask(&key), item);
            }
        }
        Value::Array(items) => {
            for item in items {
                scrub(item, mask);
            }
        }
        Value::String(text) => *text = mask(text),
        Value::Number(number) => {
            if mask(&number.to_string()) != number.to_string() {
                *value = Value::String("[REDACTED]".into());
            }
        }
        Value::Bool(_) | Value::Null => {
            let text = value.to_string();
            if mask(&text) != text {
                *value = Value::String("[REDACTED]".into());
            }
        }
    }
}
fn result<T: Serialize>(
    session: &Session,
    id: &str,
    method: &str,
    value: T,
    stream: bool,
    mask: &(dyn Fn(&str) -> String + Sync),
) -> Result<()> {
    let mut value = serde_json::to_value(value)?;
    moleapi_core::validate_mcp_json(&value)?;
    scrub(&mut value, mask);
    session.event(
        "received",
        if stream {
            EventMessage::A2aStream {
                request_id: mask(id),
                method: mask(method),
                result: value,
            }
        } else {
            EventMessage::A2aResult {
                request_id: mask(id),
                method: mask(method),
                result: value,
            }
        },
    )
}
async fn stream<T: Serialize, E: std::error::Error + Send + Sync + 'static>(
    session: &Session,
    id: &str,
    method: &str,
    mut events: std::pin::Pin<
        Box<dyn futures_util::Stream<Item = std::result::Result<T, E>> + Send>,
    >,
    mask: &(dyn Fn(&str) -> String + Sync),
) -> Result<()> {
    let mut count = 0;
    while let Some(event) = events.next().await {
        count += 1;
        ensure!(count <= 2048, "A2A stream event limit exceeded");
        let value = serde_json::to_value(event.map_err(anyhow::Error::new)?)?;
        let settled = value.get("final") == Some(&Value::Bool(true))
            || value
                .pointer("/statusUpdate/status/state")
                .or_else(|| value.pointer("/task/status/state"))
                .and_then(Value::as_str)
                .is_some_and(|state| {
                    matches!(
                        state,
                        "TASK_STATE_COMPLETED"
                            | "TASK_STATE_FAILED"
                            | "TASK_STATE_CANCELED"
                            | "TASK_STATE_REJECTED"
                            | "TASK_STATE_INPUT_REQUIRED"
                            | "TASK_STATE_AUTH_REQUIRED"
                    )
                });
        result(session, id, method, value, true, mask)?;
        if settled {
            break;
        }
    }
    Ok(())
}
struct Calls(HashMap<String, (CancellationToken, tokio::task::JoinHandle<()>)>);
impl Drop for Calls {
    fn drop(&mut self) {
        for (cancel, handle) in self.0.values() {
            cancel.cancel();
            handle.abort();
        }
    }
}
fn error_value(error: &anyhow::Error, mask: &(dyn Fn(&str) -> String + Sync)) -> Value {
    let message = error.to_string();
    let mut end = message.len().min(64 * 1024);
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    let mut value = serde_json::json!({"message":&message[..end]});
    if end < message.len() {
        value["truncated"] = true.into();
    }
    if let Some(a2a_client::A2AError::RemoteAgentError { code, .. }) = error.downcast_ref() {
        value["code"] = serde_json::json!(code);
    }
    if let Some(a2a_client_legacy::A2AError::RemoteAgentError { code, .. }) = error.downcast_ref() {
        value["code"] = serde_json::json!(code);
    }
    scrub(&mut value, mask);
    value
}
pub(crate) async fn run(
    session: Arc<Session>,
    request: RequestSpec,
    policy: NetworkPolicy,
    mut commands: mpsc::Receiver<Command>,
    mask: Arc<dyn Fn(&str) -> String + Send + Sync>,
) -> Result<String> {
    let Protocol::A2a { config } = &request.protocol else {
        bail!("Not A2A")
    };
    moleapi_core::validate_a2a(&request, false)?;
    let environment = session
        .a2a_environment
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| anyhow::anyhow!("A2A environment was not configured"))?;
    if session.cancel.is_cancelled() {
        return Ok("A2A session stopped while opening".into());
    }
    session.record.lock().unwrap().summary.state = SessionState::Open;
    session.event(
        "system",
        EventMessage::State {
            state: SessionState::Open,
            reason: None,
        },
    )?;
    session.event(
        "received",
        EventMessage::A2aReady {
            dialect: config.dialect.clone(),
            transport: config.transport.clone(),
        },
    )?;
    let mut calls = Calls(HashMap::new());
    let mut ids = HashSet::new();
    loop {
        tokio::select! { biased;
            _=session.cancel.cancelled()=> {for (_, (cancel,handle)) in calls.0.drain(){cancel.cancel();handle.abort();} return Ok("A2A session stopped locally".into());},
            command=commands.recv()=> {
                let Some(Command::A2a(command))=command else {return Ok("A2A command channel closed".into())};
                calls.0.retain(|_,(_,handle)|!handle.is_finished());
                match command {
                    SendMessage::A2aStop{request_id}=> {if let Some((token,_))=calls.0.get(&request_id){token.cancel();}},
                    SendMessage::A2aRequest{request_id,method,params_source}=> {
                        let rejected=if ids.len()>=512 {Some("A2A invocation budget exceeded")}else if calls.0.len()>=8 {Some("A2A active invocation capacity exceeded")}else if !ids.insert(request_id.clone()){Some("A2A invocation id already used")}else{None};
                        if let Some(error)=rejected{session.event("received",EventMessage::A2aError{request_id:mask(&request_id),method:mask(&method),error:serde_json::json!({"message":error})})?;session.event("received",EventMessage::A2aFinished{request_id:mask(&request_id),method:mask(&method)})?;continue;}

                        let token=CancellationToken::new();
                        let cancel=token.clone();let s=session.clone();let req=request.clone();let env=environment.clone();let m=mask.clone();let id=request_id.clone();
                        let handle=tokio::spawn(async move {
                            let work=invoke(&s,&req,policy,&env,&id,&method,&params_source,&*m);
                            let outcome=tokio::select!{biased;_=s.cancel.cancelled()=>return, _=cancel.cancelled()=>Err(anyhow::anyhow!("Invocation stopped locally; remote task was not canceled")), outcome=tokio::time::timeout(Duration::from_millis(req.timeout_ms.clamp(100,300_000)),work)=>outcome.map_err(|_|anyhow::anyhow!("A2A invocation timed out")).and_then(|v|v)};
                            if let Err(error)=outcome {let _=s.event("received",EventMessage::A2aError{request_id:m(&id),method:m(&method),error:error_value(&error,&*m)});}
                            let _=s.event("received",EventMessage::A2aFinished{request_id:m(&id),method:m(&method)});
                        });
                        calls.0.insert(request_id,(token,handle));
                    }
                    _=>bail!("Invalid A2A command")
                }
            }
        }
    }
}
#[allow(clippy::too_many_arguments)]
async fn invoke(
    session: &Session,
    request: &RequestSpec,
    policy: NetworkPolicy,
    environment: &moleapi_core::Environment,
    id: &str,
    method: &str,
    source: &str,
    mask: &(dyn Fn(&str) -> String + Sync),
) -> Result<()> {
    let Protocol::A2a { config } = &request.protocol else {
        bail!("Not A2A")
    };
    let value: Value = serde_json::from_str(source)?;
    moleapi_core::validate_mcp_json(&value)?;
    let resolved: Value =
        serde_json::from_str(&moleapi_core::resolve_grpc_source(source, environment)?)?;
    moleapi_core::validate_mcp_json(&resolved)?;
    let mut endpoint = moleapi_core::protocol_url(&request.url, false)?;
    if let Some(selected) = &config.interface_url {
        ensure!(
            selected == endpoint.as_str() || selected == &request.url,
            "Adopt the selected interface URL explicitly before connecting"
        );
    }
    for pair in request.query.iter().filter(|pair| pair.enabled) {
        endpoint
            .query_pairs_mut()
            .append_pair(&pair.key, &pair.value);
    }
    validate_params(method, &resolved, &config.dialect)?;
    // Build an injected, no-proxy, no-redirect client freshly for each invocation. SDKs never negotiate URLs.
    let addresses = moleapi_core::checked_destination(&endpoint, policy).await?;
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .danger_accept_invalid_certs(!request.verify_tls)
        .resolve_to_addrs(
            endpoint.host_str().unwrap().trim_matches(['[', ']']),
            &addresses,
        )
        .timeout(Duration::from_millis(
            request.timeout_ms.clamp(100, 300_000),
        ))
        .default_headers(moleapi_core::request_headers(request)?)
        .build()?;
    // SDK card inputs are synthetic only when no source was selected; a supplied card keeps capabilities.
    let mut card: Value = if let Some(source) = &config.card_source {
        serde_json::from_str(source)?
    } else if config.dialect == "0.3" {
        serde_json::json!({"name":"Explicit A2A endpoint","description":"","version":"1","protocolVersion":"0.3.0","url":request.url,"capabilities":{"streaming":true},"defaultInputModes":["text/plain"],"defaultOutputModes":["text/plain"],"skills":[]})
    } else {
        serde_json::json!({"name":"Explicit A2A endpoint","description":"","version":"1","supportedInterfaces":[],"capabilities":{"streaming":true},"defaultInputModes":["text/plain"],"defaultOutputModes":["text/plain"],"skills":[]})
    };
    moleapi_core::validate_mcp_json(&card)?;
    ensure!(
        !card
            .pointer("/capabilities/extensions")
            .and_then(Value::as_array)
            .is_some_and(|e| e.iter().any(|v| v["required"] == true)),
        "Agent requires unsupported extensions"
    );
    if let Some(source) = &config.card_source {
        let description = describe_card(source, &config.dialect)?;
        let transport = if config.transport == "http-json" {
            "HTTP+JSON"
        } else {
            "JSONRPC"
        };
        ensure!(
            description
                .interfaces
                .iter()
                .any(|interface| interface.supported && interface.transport == transport),
            "Agent Card does not advertise the selected version and transport"
        );
    }
    // Push registration must be an explicit method; message configuration cannot register callbacks implicitly.
    ensure!(
        (resolved
            .pointer("/configuration/pushNotificationConfig")
            .is_none()
            && resolved
                .pointer("/configuration/taskPushNotificationConfig")
                .is_none()
            && resolved
                .pointer("/configuration/task_push_notification_config")
                .is_none()),
        "Use an explicit push configuration operation after checking the callback URL"
    );
    if (method.contains("pushNotification") || method.contains("PushNotification"))
        && let Some(url) = resolved
            .pointer("/pushNotificationConfig/url")
            .or_else(|| resolved.get("url"))
            .and_then(Value::as_str)
    {
        let url = moleapi_core::protocol_url(url, false)?;
        moleapi_core::checked_destination(&url, policy).await?;
    }
    if config.dialect == "0.3" {
        let version = card["protocolVersion"].as_str().unwrap_or("0.3.0");
        ensure!(
            matches!(version, "0.3" | "0.3.0"),
            "Card protocol version does not match A2A 0.3"
        );
        card["url"] = endpoint.to_string().into();
        card["preferredTransport"] = "JSONRPC".into();
        card["additionalInterfaces"] = serde_json::json!([]);
        let sdk = a2a_client_legacy::A2AClient::from_card_with_client(
            serde_json::from_value(card)?,
            client,
        )?;
        macro_rules! call {
            ($name:ident) => {
                result(
                    session,
                    id,
                    method,
                    sdk.$name(serde_json::from_value(resolved)?).await?,
                    false,
                    mask,
                )
            };
        }
        match method {
            "message/send" => call!(send_message),
            "message/stream" => {
                stream(
                    session,
                    id,
                    method,
                    sdk.send_streaming_message(serde_json::from_value(resolved)?)
                        .await?,
                    mask,
                )
                .await
            }
            "tasks/get" => call!(get_task),
            "tasks/cancel" => call!(cancel_task),
            "tasks/resubscribe" => {
                stream(
                    session,
                    id,
                    method,
                    sdk.resubscribe_task(serde_json::from_value(resolved)?)
                        .await?,
                    mask,
                )
                .await
            }
            "tasks/pushNotificationConfig/set" => call!(set_task_push_notification_config),
            "tasks/pushNotificationConfig/get" => call!(get_task_push_notification_config),
            "tasks/pushNotificationConfig/list" => call!(list_task_push_notification_config),
            "tasks/pushNotificationConfig/delete" => call!(delete_task_push_notification_config),
            _ => bail!("Unsupported A2A 0.3 operation"),
        }
    } else {
        let binding = if config.transport == "http-json" {
            "HTTP+JSON"
        } else {
            "JSONRPC"
        };
        if config.card_source.is_some() {
            ensure!(
                card["supportedInterfaces"]
                    .as_array()
                    .is_some_and(|interfaces| interfaces
                        .iter()
                        .any(|i| matches!(i["protocolVersion"].as_str(), Some("1.0" | "1.0.0")))),
                "Card does not advertise A2A 1.0"
            );
        }
        card["supportedInterfaces"] = serde_json::json!([{"url":endpoint.as_str(),"protocolBinding":binding,"protocolVersion":"1.0"}]);
        let sdk =
            a2a_client::A2AClient::from_card_with_client(serde_json::from_value(card)?, client)?;
        macro_rules! call {
            ($name:ident) => {
                result(
                    session,
                    id,
                    method,
                    sdk.$name(serde_json::from_value(resolved)?).await?,
                    false,
                    mask,
                )
            };
        }
        match method {
            "message/send" => call!(send_message),
            "message/stream" => {
                stream(
                    session,
                    id,
                    method,
                    sdk.send_streaming_message(serde_json::from_value(resolved)?)
                        .await?,
                    mask,
                )
                .await
            }
            "tasks/get" => call!(get_task),
            "tasks/cancel" => call!(cancel_task),
            "tasks/list" => call!(list_tasks),
            "tasks/resubscribe" => {
                stream(
                    session,
                    id,
                    method,
                    sdk.subscribe_to_task(serde_json::from_value(resolved)?)
                        .await?,
                    mask,
                )
                .await
            }
            "tasks/pushNotificationConfig/set" => call!(create_task_push_notification_config),
            "tasks/pushNotificationConfig/get" => call!(get_task_push_notification_config),
            "tasks/pushNotificationConfig/list" => call!(list_task_push_notification_configs),
            "tasks/pushNotificationConfig/delete" => call!(delete_task_push_notification_config),
            _ => bail!("Unsupported A2A 1.0 operation"),
        }
    }
}

#[derive(serde::Serialize)]
pub struct CardInterface {
    pub url: String,
    pub transport: String,
    pub version: String,
    pub supported: bool,
}
#[derive(serde::Serialize)]
pub struct CardDescription {
    pub card: Value,
    pub dialect: String,
    pub interfaces: Vec<CardInterface>,
    pub warnings: Vec<String>,
}
pub fn describe_card(source: &str, dialect: &str) -> Result<CardDescription> {
    ensure!(source.len() <= MAX_MESSAGE, "Agent Card exceeds 1 MiB");
    let card: Value = serde_json::from_str(source)?;
    moleapi_core::validate_mcp_json(&card)?;
    ensure!(
        card["name"]
            .as_str()
            .is_some_and(|name| !name.is_empty() && name.len() <= 4096),
        "Agent Card name is required and bounded"
    );
    ensure!(
        card["skills"]
            .as_array()
            .is_none_or(|skills| skills.len() <= 128),
        "Agent Card skill budget exceeds 128"
    );
    let mut interfaces = vec![];
    let mut warnings = vec![];
    match dialect {
        "0.3" => {
            let parsed: a2a_types_legacy::AgentCard = serde_json::from_value(card.clone())?;
            let supported = matches!(parsed.protocol_version.as_str(), "0.3" | "0.3.0");
            interfaces.push(CardInterface {
                url: parsed.url,
                transport: serde_json::to_value(parsed.preferred_transport)?
                    .as_str()
                    .unwrap_or("")
                    .into(),
                version: parsed.protocol_version.clone(),
                supported,
            });
            for interface in parsed.additional_interfaces {
                let transport = serde_json::to_value(interface.transport)?
                    .as_str()
                    .unwrap_or("")
                    .to_owned();
                interfaces.push(CardInterface {
                    url: interface.url,
                    supported: supported && transport == "JSONRPC",
                    transport,
                    version: parsed.protocol_version.clone(),
                });
            }
        }
        "1.0" => {
            let parsed: a2a_types::AgentCard = serde_json::from_value(card.clone())?;
            for interface in parsed.supported_interfaces {
                interfaces.push(CardInterface {
                    url: interface.url,
                    supported: matches!(interface.protocol_version.as_str(), "1.0" | "1.0.0")
                        && matches!(interface.protocol_binding.as_str(), "JSONRPC" | "HTTP+JSON"),
                    transport: interface.protocol_binding,
                    version: interface.protocol_version,
                });
            }
        }
        _ => bail!("Choose A2A 0.3 or 1.0"),
    }
    for interface in &mut interfaces {
        interface.supported &= matches!(interface.transport.as_str(), "JSONRPC")
            || dialect == "1.0" && interface.transport == "HTTP+JSON";
        if moleapi_core::protocol_url(&interface.url, false).is_err() {
            interface.supported = false;
        }
    }
    if interfaces.iter().any(|i| !i.supported) {
        warnings.push("Some advertised versions/transports are unsupported; select a supported interface explicitly".into());
    }
    ensure!(
        interfaces.len() <= 32,
        "Agent Card interface budget exceeds 32"
    );
    ensure!(!interfaces.is_empty(), "Agent Card contains no interfaces");
    Ok(CardDescription {
        card,
        dialect: dialect.into(),
        interfaces,
        warnings,
    })
}
fn validate_params(method: &str, value: &Value, dialect: &str) -> Result<()> {
    ensure!(
        matches!(
            method,
            "message/send"
                | "message/stream"
                | "tasks/get"
                | "tasks/cancel"
                | "tasks/resubscribe"
                | "tasks/pushNotificationConfig/set"
                | "tasks/pushNotificationConfig/get"
                | "tasks/pushNotificationConfig/list"
                | "tasks/pushNotificationConfig/delete"
        ) || method == "tasks/list" && dialect == "1.0",
        "Unsupported A2A operation"
    );
    for pointer in [
        "/historyLength",
        "/history_length",
        "/configuration/historyLength",
        "/configuration/history_length",
    ] {
        if let Some(length) = value.pointer(pointer) {
            ensure!(
                length
                    .as_i64()
                    .is_some_and(|length| (0..=1000).contains(&length)),
                "A2A historyLength must be 0..1000"
            );
        }
    }
    if let Some(size) = value.get("pageSize").or_else(|| value.get("page_size")) {
        ensure!(
            size.as_i64().is_some_and(|size| (1..=100).contains(&size)),
            "A2A pageSize must be 1..100"
        );
    }
    if method.starts_with("message/") {
        let message = value
            .get("message")
            .ok_or_else(|| anyhow::anyhow!("A2A message is required"))?;
        ensure!(
            message
                .get("messageId")
                .or_else(|| message.get("message_id"))
                .and_then(Value::as_str)
                .is_some_and(|id| !id.is_empty() && id.len() <= 4096),
            "A2A messageId is required and bounded"
        );
        ensure!(
            matches!(
                (dialect, message["role"].as_str()),
                ("0.3", Some("user" | "agent")) | ("1.0", Some("ROLE_USER" | "ROLE_AGENT"))
            ) || dialect == "1.0" && matches!(message["role"].as_i64(), Some(1 | 2)),
            "Invalid A2A message role"
        );
        for key in ["contextId", "context_id", "taskId", "task_id"] {
            if let Some(value) = message.get(key) {
                ensure!(
                    value.as_str().is_some_and(|id| id.len() <= 4096),
                    "A2A context/task id exceeds limit"
                );
            }
        }
        ensure!(
            message["parts"]
                .as_array()
                .is_some_and(|parts| !parts.is_empty() && parts.len() <= 128),
            "A2A message requires 1..128 parts"
        );
    } else if matches!(method, "tasks/get" | "tasks/cancel" | "tasks/resubscribe") {
        ensure!(
            value["id"]
                .as_str()
                .is_some_and(|id| !id.is_empty() && id.len() <= 4096),
            "A2A task id is required and bounded"
        );
    }
    Ok(())
}
