//! Official MCP SDK sessions with bounded, manual client callbacks.
#![allow(deprecated)] // rmcp 3.5 retains negotiated legacy sampling/roots for interoperable servers.
use crate::*;
use anyhow::{Context, bail};
use rmcp::{
    ClientHandler, RoleClient, ServiceExt,
    model::*,
    service::{NotificationContext, Peer, PeerRequestOptions, RequestContext, ServiceError},
    transport::{
        StreamableHttpClientTransport, TokioChildProcess,
        streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde_json::{Value, json};
use std::collections::HashSet;
use tokio::sync::oneshot;
#[path = "mcp_http.rs"]
mod http;

type CallbackResult = std::result::Result<Value, ErrorData>;
type Callbacks = Arc<Mutex<HashMap<String, oneshot::Sender<CallbackResult>>>>;
#[derive(Clone)]
struct Handler {
    session: Arc<Session>,
    callbacks: Callbacks,
    refresh: mpsc::Sender<()>,
    mask: PrivacyMask,
    failure: Arc<Mutex<Option<String>>>,
}
fn safe_value(value: Value, mask: &PrivacyMask) -> Result<Value> {
    moleapi_core::validate_mcp_json(&value)?;
    let source = serde_json::to_string(&value)?;
    ensure!(source.len() <= MAX_MESSAGE, "MCP payload exceeds 1 MiB");
    fn scrub(value: &mut Value, mask: &PrivacyMask) {
        match value {
            Value::String(text) => *text = mask(text),
            Value::Array(items) => {
                for item in items {
                    scrub(item, mask);
                }
            }
            Value::Object(items) => {
                let mut cleaned = serde_json::Map::new();
                for (key, mut item) in std::mem::take(items) {
                    let key = mask(&key);
                    scrub(&mut item, mask);
                    if cleaned.insert(key, item).is_some() {
                        // A masked key must never overwrite another field or expose
                        // an original key to disambiguate it. Withhold this object.
                        *value = "[REDACTED: object key collision]".into();
                        return;
                    }
                }
                *items = cleaned;
            }
            Value::Number(_) | Value::Bool(_) | Value::Null => {
                let representation = value.to_string();
                if mask(&representation) != representation {
                    *value = "[REDACTED]".into();
                }
            }
        }
    }
    let mut value = value;
    scrub(&mut value, mask);
    ensure!(
        serde_json::to_vec(&value)?.len() <= MAX_MESSAGE,
        "MCP redacted payload exceeds 1 MiB"
    );
    Ok(value)
}
impl Handler {
    fn notify(&self, method: &str, params: Value) {
        match safe_value(params, &self.mask).and_then(|params| {
            self.session.event(
                "received",
                EventMessage::McpNotification {
                    method: (self.mask)(method),
                    params,
                },
            )
        }) {
            Ok(()) => {}
            Err(error) => {
                *self.failure.lock().unwrap() = Some(error.to_string());
                self.session.cancel.cancel();
            }
        }
    }
    async fn callback<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: Value,
        context: RequestContext<RoleClient>,
    ) -> std::result::Result<T, ErrorData> {
        let id = uuid::Uuid::new_v4().to_string();
        let (sender, receiver) = oneshot::channel();
        {
            let mut pending = self.callbacks.lock().unwrap();
            if pending.len() >= 8 {
                return Err(ErrorData::internal_error(
                    "Client callback capacity reached",
                    None,
                ));
            }
            pending.insert(id.clone(), sender);
        }
        let emitted = safe_value(params, &self.mask).and_then(|params| {
            self.session.event(
                "received",
                EventMessage::McpCallback {
                    callback_id: id.clone(),
                    method: method.into(),
                    params,
                },
            )
        });
        let result = if emitted.is_err() {
            Err(ErrorData::internal_error(
                "Callback payload exceeds limit",
                None,
            ))
        } else {
            tokio::select! {
                biased;
                _ = self.session.cancel.cancelled() => Err(ErrorData::internal_error("Client disconnected", None)),
                _ = context.ct.cancelled() => Err(ErrorData::internal_error("Server cancelled callback", None)),
                _ = tokio::time::sleep(Duration::from_secs(120)) => Err(ErrorData::internal_error("Client callback timed out", None)),
                response = receiver => response.unwrap_or_else(|_| Err(ErrorData::internal_error("Client callback ended", None))),
            }
        };
        self.callbacks.lock().unwrap().remove(&id);
        self.notify(
            "moleapi/callback_completed",
            json!({"callback_id":id,"status":if result.is_ok() {"replied"} else {"expired"}}),
        );
        let value = result?;
        serde_json::from_value(value)
            .map_err(|_| ErrorData::invalid_params("Invalid callback response", None))
    }
}
impl ClientHandler for Handler {
    fn get_info(&self) -> ClientConfig {
        // Capability advertisement matches the manual methods below; no filesystem or LLM discovery.
        ClientConfig::new(
            serde_json::from_value(json!({"roots":{},"sampling":{},"elicitation":{"form":{}}}))
                .unwrap(),
            Implementation::new("MoleAPI", env!("CARGO_PKG_VERSION")),
        )
        .with_protocol_version(ProtocolVersion::V_2025_11_25)
    }
    async fn create_message(
        &self,
        params: CreateMessageRequestParams,
        context: RequestContext<RoleClient>,
    ) -> std::result::Result<CreateMessageResult, ErrorData> {
        self.callback(
            "sampling/createMessage",
            serde_json::to_value(params).unwrap(),
            context,
        )
        .await
    }
    async fn create_elicitation(
        &self,
        params: ElicitRequestParams,
        context: RequestContext<RoleClient>,
    ) -> std::result::Result<ElicitResult, ErrorData> {
        let params = serde_json::to_value(params).unwrap();
        if params.get("requestedSchema").is_none() {
            return Err(ErrorData::invalid_params(
                "Only manual form elicitation is supported",
                None,
            ));
        }
        let result: ElicitResult = self
            .callback("elicitation/create", params.clone(), context)
            .await?;
        if result.action == ElicitationAction::Accept
            && let Some(schema) = params.get("requestedSchema")
        {
            let valid = result.content.as_ref().is_some_and(|content| {
                jsonschema::options()
                    .with_pattern_options(
                        jsonschema::PatternOptions::fancy_regex()
                            .backtrack_limit(10000)
                            .size_limit(65536),
                    )
                    .build(schema)
                    .is_ok_and(|validator| validator.is_valid(content))
            });
            if !valid {
                return Err(ErrorData::invalid_params(
                    "Accepted elicitation content does not match requested schema",
                    None,
                ));
            }
        }
        Ok(result)
    }
    async fn list_roots(
        &self,
        context: RequestContext<RoleClient>,
    ) -> std::result::Result<ListRootsResult, ErrorData> {
        let result: ListRootsResult = self.callback("roots/list", json!({}), context).await?;
        if result.roots.len() > 64
            || result.roots.iter().any(|root| {
                root.uri.len() > 8192
                    || !url::Url::parse(&root.uri).is_ok_and(|uri| uri.scheme() == "file")
            })
        {
            return Err(ErrorData::invalid_params(
                "Roots must contain at most 64 explicit file URIs",
                None,
            ));
        }
        Ok(result)
    }
    async fn on_progress(
        &self,
        params: ProgressNotificationParam,
        _: NotificationContext<RoleClient>,
    ) {
        self.notify(
            "notifications/progress",
            serde_json::to_value(params).unwrap(),
        );
    }
    async fn on_logging_message(
        &self,
        params: LoggingMessageNotificationParam,
        _: NotificationContext<RoleClient>,
    ) {
        self.notify(
            "notifications/message",
            serde_json::to_value(params).unwrap(),
        );
    }
    async fn on_resource_updated(
        &self,
        params: ResourceUpdatedNotificationParam,
        _: NotificationContext<RoleClient>,
    ) {
        self.notify(
            "notifications/resources/updated",
            serde_json::to_value(params).unwrap(),
        );
    }
    async fn on_cancelled(
        &self,
        params: CancelledNotificationParam,
        _: NotificationContext<RoleClient>,
    ) {
        self.notify(
            "notifications/cancelled",
            serde_json::to_value(params).unwrap(),
        );
    }
    async fn on_tool_list_changed(&self, _: NotificationContext<RoleClient>) {
        self.notify("notifications/tools/list_changed", json!({}));
        let _ = self.refresh.try_send(());
    }
    async fn on_resource_list_changed(&self, _: NotificationContext<RoleClient>) {
        self.notify("notifications/resources/list_changed", json!({}));
        let _ = self.refresh.try_send(());
    }
    async fn on_prompt_list_changed(&self, _: NotificationContext<RoleClient>) {
        self.notify("notifications/prompts/list_changed", json!({}));
        let _ = self.refresh.try_send(());
    }
    async fn on_custom_notification(
        &self,
        notification: CustomNotification,
        _: NotificationContext<RoleClient>,
    ) {
        self.notify(
            &notification.method,
            serde_json::to_value(notification.params).unwrap_or(Value::Null),
        );
    }
}
pub(super) fn send(session: &Arc<Session>, mut message: SendMessage) -> Result<()> {
    if let SendMessage::McpRequest {
        name,
        uri,
        arguments_source,
        ..
    } = &mut message
        && let Some(environment) = session.mcp_environment.lock().unwrap().as_ref()
    {
        *arguments_source = moleapi_core::resolve_grpc_source(arguments_source, environment)
            .map_err(|_| anyhow::anyhow!("Invalid or unresolved MCP arguments"))?;
        *name = moleapi_core::resolve_value(name, environment)
            .map_err(|_| anyhow::anyhow!("Unresolved MCP operation"))?;
        *uri = moleapi_core::resolve_value(uri, environment)
            .map_err(|_| anyhow::anyhow!("Unresolved MCP URI"))?;
    }
    let size = serde_json::to_vec(&message)?.len();
    ensure!(size <= MAX_MESSAGE, "MCP command exceeds 1 MiB");
    if let SendMessage::McpRequest {
        request_id,
        method,
        name,
        uri,
        arguments_source,
    } = &message
    {
        ensure!(
            !request_id.is_empty()
                && request_id.len() <= 128
                && name.len() <= 4096
                && uri.len() <= 8192,
            "Invalid MCP command identifier"
        );
        ensure!(
            matches!(
                method.as_str(),
                "refresh"
                    | "tools/call"
                    | "resources/read"
                    | "prompts/get"
                    | "resources/subscribe"
                    | "resources/unsubscribe"
            ),
            "Unsupported MCP operation"
        );
        let value: Value =
            serde_json::from_str(arguments_source).context("Invalid MCP argument JSON")?;
        ensure!(value.is_object(), "MCP arguments must be a JSON object");
        moleapi_core::validate_mcp_json(&value)?;
    }
    if let SendMessage::McpCancel { request_id } = &message {
        ensure!(
            !request_id.is_empty() && request_id.len() <= 128,
            "Invalid MCP request identifier"
        );
    }
    if let SendMessage::McpCallback {
        callback_id,
        result,
        error,
    } = &message
    {
        ensure!(
            !callback_id.is_empty() && callback_id.len() <= 128,
            "Invalid MCP callback identifier"
        );
        ensure!(
            result.is_some() != error.is_some(),
            "Supply callback result or error"
        );
        if let Some(value) = result {
            moleapi_core::validate_mcp_json(value)?;
        }
        if let Some(value) = error {
            serde_json::from_value::<ErrorData>(value.clone())
                .context("Invalid MCP callback error")?;
        }
    }
    let mut record = session.record.lock().unwrap();
    ensure!(
        record.summary.protocol == "mcp" && record.summary.state == SessionState::Open,
        "Expected open MCP session"
    );
    ensure!(
        record.input + size <= MAX_INPUT,
        "MCP input budget exceeded"
    );
    ensure!(
        record.grpc_input_messages < 512,
        "MCP command budget exceeded (512)"
    );
    session
        .commands
        .try_send(Command::Mcp(message))
        .map_err(|_| anyhow::anyhow!("MCP command queue is full or closed"))?;
    record.input += size;
    record.grpc_input_messages += 1;
    record.summary.sent_bytes += size as u64;
    Ok(())
}
async fn request(
    peer: &Peer<RoleClient>,
    method: &str,
    params: Value,
    timeout: Duration,
    cancel: &CancellationToken,
) -> Result<Value> {
    let request: ClientRequest = serde_json::from_value(json!({"method":method,"params":params}))?;
    let mut handle = peer
        .send_request_with_option(request, PeerRequestOptions::with_timeout(timeout))
        .await?;
    let result = tokio::select! {
        biased;
        _ = cancel.cancelled() => { let _ = handle.cancel(Some("User stopped MCP operation".into())).await; bail!("MCP operation cancelled"); },
        _ = tokio::time::sleep(timeout) => { let _ = handle.cancel(Some("MCP operation timeout".into())).await; bail!("MCP operation timed out"); },
        response = &mut handle.rx => response.context("MCP response channel closed")??,
    };
    let value = serde_json::to_value(result)?;
    moleapi_core::validate_mcp_json(&value)?;
    ensure!(
        serde_json::to_vec(&value)?.len() <= MAX_MESSAGE,
        "MCP result exceeds 1 MiB"
    );
    Ok(value)
}
#[derive(Default)]
struct Catalog {
    tools: Vec<Value>,
    resources: Vec<Value>,
    templates: Vec<Value>,
    prompts: Vec<Value>,
}
async fn pages(
    peer: &Peer<RoleClient>,
    method: &str,
    key: &str,
    timeout: Duration,
    cancel: &CancellationToken,
) -> Result<Vec<Value>> {
    let mut result = vec![];
    let mut cursor = None;
    let mut seen = HashSet::new();
    let mut bytes = 0;
    for _ in 0..32 {
        let page = request(
            peer,
            method,
            cursor.as_ref().map_or(json!({}), |c| json!({"cursor":c})),
            timeout,
            cancel,
        )
        .await?;
        bytes += serde_json::to_vec(&page)?.len();
        ensure!(bytes <= MAX_MESSAGE, "MCP capability bytes exceed 1 MiB");
        let items = page
            .get(key)
            .and_then(Value::as_array)
            .context("Invalid MCP capability response")?;
        result.extend(items.iter().cloned());
        ensure!(result.len() <= 512, "MCP capability count exceeds 512");
        cursor = page
            .get("nextCursor")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let Some(next) = &cursor else {
            return Ok(result);
        };
        ensure!(
            next.len() <= 4096 && seen.insert(next.clone()),
            "MCP pagination cursor repeats or exceeds limit"
        );
    }
    bail!("MCP capability pagination exceeds 32 pages")
}
async fn refresh(
    peer: &Peer<RoleClient>,
    handler: &Handler,
    catalog: &Arc<Mutex<Catalog>>,
    timeout: Duration,
    cancel: &CancellationToken,
) -> Result<()> {
    let caps = peer
        .peer_info()
        .context("MCP initialization missing")?
        .capabilities
        .clone();
    let mut next = Catalog::default();
    if caps.tools.is_some() {
        next.tools = pages(peer, "tools/list", "tools", timeout, cancel).await?;
    }
    if caps.resources.is_some() {
        next.resources = pages(peer, "resources/list", "resources", timeout, cancel).await?;
        next.templates = pages(
            peer,
            "resources/templates/list",
            "resourceTemplates",
            timeout,
            cancel,
        )
        .await?;
    }
    if caps.prompts.is_some() {
        next.prompts = pages(peer, "prompts/list", "prompts", timeout, cancel).await?;
    }
    let value = safe_value(
        json!({"tools":next.tools,"resources":next.resources,"resource_templates":next.templates,"prompts":next.prompts}),
        &handler.mask,
    )?;
    handler.session.event(
        "received",
        EventMessage::McpCapabilities {
            tools: value
                .get("tools")
                .and_then(Value::as_array)
                .context("MCP capability fields withheld by privacy policy")?
                .clone(),
            resources: value
                .get("resources")
                .and_then(Value::as_array)
                .context("MCP capability fields withheld by privacy policy")?
                .clone(),
            resource_templates: value
                .get("resource_templates")
                .and_then(Value::as_array)
                .context("MCP capability fields withheld by privacy policy")?
                .clone(),
            prompts: value
                .get("prompts")
                .and_then(Value::as_array)
                .context("MCP capability fields withheld by privacy policy")?
                .clone(),
        },
    )?;
    *catalog.lock().unwrap() = next;
    Ok(())
}
fn validate_operation(
    peer: &Peer<RoleClient>,
    catalog: &Arc<Mutex<Catalog>>,
    method: &str,
    name: &str,
    uri: &str,
    arguments: &Value,
) -> Result<Value> {
    let catalog = catalog.lock().unwrap();
    let caps = peer
        .peer_info()
        .context("MCP initialization missing")?
        .capabilities
        .clone();
    match method {
        "tools/call" => {
            ensure!(caps.tools.is_some(), "Server does not support tools");
            let tool = catalog
                .tools
                .iter()
                .find(|t| t["name"].as_str() == Some(name))
                .context("Select a discovered MCP tool")?;
            let schema = tool
                .get("inputSchema")
                .context("Tool input schema missing")?;
            let validator = jsonschema::options()
                .with_pattern_options(
                    jsonschema::PatternOptions::fancy_regex()
                        .backtrack_limit(10000)
                        .size_limit(65536),
                )
                .build(schema)
                .map_err(|_| anyhow::anyhow!("Unsupported or invalid tool input schema"))?;
            ensure!(
                validator.is_valid(arguments),
                "MCP tool arguments do not match input schema"
            );
            Ok(json!({"name":name,"arguments":arguments}))
        }
        "prompts/get" => {
            ensure!(caps.prompts.is_some(), "Server does not support prompts");
            let prompt = catalog
                .prompts
                .iter()
                .find(|p| p["name"].as_str() == Some(name))
                .context("Select a discovered MCP prompt")?;
            ensure!(
                arguments
                    .as_object()
                    .unwrap()
                    .values()
                    .all(Value::is_string),
                "Prompt arguments must be strings"
            );
            if let Some(required) = prompt["arguments"].as_array() {
                for field in required {
                    if field["required"] == true {
                        ensure!(
                            field["name"]
                                .as_str()
                                .is_some_and(|name| arguments.get(name).is_some()),
                            "Required MCP prompt argument missing"
                        );
                    }
                }
            }
            Ok(json!({"name":name,"arguments":arguments}))
        }
        "resources/read" => {
            ensure!(
                caps.resources.is_some() && !uri.is_empty(),
                "Server does not support resources or URI missing"
            );
            Ok(json!({"uri":uri}))
        }
        "resources/subscribe" | "resources/unsubscribe" => {
            ensure!(
                caps.resources
                    .as_ref()
                    .is_some_and(|c| c.subscribe == Some(true))
                    && !uri.is_empty(),
                "Server does not support resource subscriptions or URI missing"
            );
            Ok(json!({"uri":uri}))
        }
        _ => bail!("Unsupported MCP operation"),
    }
}
fn report(handler: &Handler, id: &str, method: &str, result: Result<Value>) -> Result<()> {
    match result {
        Ok(value) => handler.session.event(
            "received",
            EventMessage::McpResult {
                request_id: id.into(),
                method: method.into(),
                result: safe_value(value, &handler.mask)?,
            },
        ),
        Err(error) => {
            let value =
                if let Some(ServiceError::McpError(data)) = error.downcast_ref::<ServiceError>() {
                    serde_json::to_value(data)?
                } else {
                    json!({"code":-32000,"message":(handler.mask)(&error.to_string())})
                };
            handler.session.event(
                "received",
                EventMessage::McpError {
                    request_id: id.into(),
                    method: method.into(),
                    error: safe_value(value, &handler.mask)?,
                },
            )
        }
    }
}
pub(super) async fn run(
    session: Arc<Session>,
    request_spec: RequestSpec,
    policy: NetworkPolicy,
    mut commands: mpsc::Receiver<Command>,
    mask: PrivacyMask,
) -> Result<String> {
    let _lifecycle_guard = session.cancel.clone().drop_guard();
    let Protocol::Mcp { config } = &request_spec.protocol else {
        unreachable!()
    };
    moleapi_core::validate_mcp(&request_spec, false)?;
    let timeout = Duration::from_millis(request_spec.timeout_ms.clamp(1, 120000));
    let (refresh_tx, mut refresh_rx) = mpsc::channel(1);
    let handler = Handler {
        session: session.clone(),
        callbacks: Arc::default(),
        refresh: refresh_tx,
        mask,
        failure: Arc::default(),
    };
    let initialize = async {
        if config.transport == "stdio" {
            let mut command = tokio::process::Command::new(&config.command);
            command.args(&config.args).env_clear();
            for pair in config.env.iter().filter(|p| p.enabled) {
                command.env(&pair.key, &pair.value);
            }
            let mut wrapped = process_wrap::tokio::CommandWrap::from(command);
            wrapped.wrap(process_wrap::tokio::KillOnDrop);
            #[cfg(unix)]
            wrapped.wrap(process_wrap::tokio::ProcessGroup::leader());
            #[cfg(windows)]
            wrapped.wrap(process_wrap::tokio::JobObject);
            let transport = TokioChildProcess::builder(wrapped)
                .stderr(std::process::Stdio::null())
                .spawn()?
                .0;
            handler
                .clone()
                .serve_with_ct(transport, session.cancel.child_token())
                .await
                .map_err(anyhow::Error::from)
        } else {
            let mut endpoint = moleapi_core::protocol_url(&request_spec.url, false)?;
            for pair in request_spec.query.iter().filter(|p| p.enabled) {
                endpoint
                    .query_pairs_mut()
                    .append_pair(&pair.key, &pair.value);
            }
            let mut transport_config =
                StreamableHttpClientTransportConfig::with_uri(endpoint.as_str());
            let mut retry = rmcp::transport::common::client_side_sse::ExponentialBackoff::default();
            retry.max_times = Some(3);
            transport_config.retry_config = Arc::new(retry);
            transport_config.max_sse_event_size = MAX_MESSAGE;
            transport_config.channel_buffer_capacity = 16;
            transport_config.max_concurrent_requests = 4;
            transport_config.reinit_on_expired_session = false;
            let client = http::CheckedHttp {
                endpoint: endpoint.to_string(),
                policy,
                verify_tls: request_spec.verify_tls,
                network: request_spec.network.clone(),
                timeout,
                headers: moleapi_core::request_headers(&request_spec)?,
                received: Arc::default(),
            };
            handler
                .clone()
                .serve_with_ct(
                    StreamableHttpClientTransport::with_client(client, transport_config),
                    session.cancel.child_token(),
                )
                .await
                .map_err(anyhow::Error::from)
        }
    };
    let service = tokio::select! {
        biased;
        _ = session.cancel.cancelled() => return Ok("MCP connect cancelled".into()),
        result = tokio::time::timeout(timeout, initialize) => result.context("MCP initialization timed out")??,
    };
    let peer = service.peer().clone();
    let catalog = Arc::new(Mutex::new(Catalog::default()));
    session.open(Handshake {
        status: 200,
        headers: vec![],
    })?;
    // MCP negotiation is reported in mcp_initialized; STDIO has no HTTP handshake.
    session.record.lock().unwrap().summary.handshake = None;
    session.event(
        "received",
        EventMessage::McpInitialized {
            info: safe_value(
                serde_json::to_value(
                    peer.peer_info()
                        .context("MCP initialize response missing")?,
                )?,
                &handler.mask,
            )?,
        },
    )?;
    let mut jobs = tokio::task::JoinSet::new();
    let mut running: HashMap<String, CancellationToken> = HashMap::new();
    let initial = session.cancel.child_token();
    running.insert("capabilities".into(), initial.clone());
    {
        let peer = peer.clone();
        let handler = handler.clone();
        let catalog = catalog.clone();
        jobs.spawn(async move {
            let result = refresh(&peer, &handler, &catalog, timeout, &initial).await;
            let _ = report(
                &handler,
                "capabilities",
                "refresh",
                result.map(|_| json!({"ok":true})),
            );
            "capabilities".to_string()
        });
    }
    let service_cancel = service.cancellation_token();
    let mut service_task = tokio::spawn(service.waiting());
    let mut service_finished = false;
    let end = loop {
        tokio::select! {
            biased;
            _ = session.cancel.cancelled() => break "MCP disconnected",
            _ = &mut service_task => { service_finished = true; break "MCP server disconnected"; },
            result = jobs.join_next(), if !jobs.is_empty() => { if let Some(Ok(id))=result { running.remove(&id); } },
            Some(()) = refresh_rx.recv(), if running.is_empty() => {
                let cancel=session.cancel.child_token(); running.insert("capabilities".into(),cancel.clone());
                let peer=peer.clone(); let handler=handler.clone(); let catalog=catalog.clone();
                jobs.spawn(async move { let result=refresh(&peer,&handler,&catalog,timeout,&cancel).await; let _=report(&handler,"capabilities","refresh",result.map(|_|json!({"ok":true}))); "capabilities".to_string() });
            }
            command = commands.recv() => {
                let Some(Command::Mcp(command))=command else { break "MCP command channel closed" };
                match command {
                    SendMessage::McpCancel {request_id} => { if let Some(cancel)=running.get(&request_id) {cancel.cancel();} },
                    SendMessage::McpCallback {callback_id,result,error} => {
                        let sender=handler.callbacks.lock().unwrap().remove(&callback_id);
                        if let Some(sender)=sender { let result=if let Some(error)=error {Err(serde_json::from_value(error)?)} else {Ok(result.unwrap_or(Value::Null))}; let _=sender.send(result); }
                        else { let _=report(&handler,&callback_id,"callback",Err(anyhow::anyhow!("Callback expired or not found"))); }
                    },
                    SendMessage::McpRequest {request_id,method,name,uri,arguments_source} => {
                        if running.len()>=4 || running.contains_key(&request_id) { let _=report(&handler,&request_id,&method,Err(anyhow::anyhow!("MCP operation capacity reached or duplicate request ID"))); continue; }
                        let arguments:Value=serde_json::from_str(&arguments_source)?;
                        let params=if method=="refresh" {Ok(json!({}))} else {validate_operation(&peer,&catalog,&method,&name,&uri,&arguments)};
                        let params=match params {Ok(params)=>params,Err(error)=>{let _=report(&handler,&request_id,&method,Err(error));continue;}};
                        let cancel=session.cancel.child_token(); running.insert(request_id.clone(),cancel.clone());
                        let peer=peer.clone();let handler=handler.clone();let catalog=catalog.clone();
                        jobs.spawn(async move { let result=if method=="refresh" {refresh(&peer,&handler,&catalog,timeout,&cancel).await.map(|_|json!({"ok":true}))} else {request(&peer,&method,params,timeout,&cancel).await}; if report(&handler,&request_id,&method,result).is_err() { handler.session.cancel.cancel(); } request_id });
                    },
                    _=>unreachable!(),
                }
            }
        }
    };
    for cancel in running.values() {
        cancel.cancel();
    }
    handler.callbacks.lock().unwrap().clear();
    let _ = tokio::time::timeout(Duration::from_secs(5), async {
        while jobs.join_next().await.is_some() {}
    })
    .await;
    jobs.abort_all();
    service_cancel.cancel();
    if !service_finished {
        let _ = tokio::time::timeout(Duration::from_secs(5), &mut service_task).await;
    }
    if let Some(error) = handler.failure.lock().unwrap().take() {
        bail!(error);
    }
    Ok(end.into())
}

#[cfg(test)]
mod review_tests {
    use super::*;
    fn session() -> (SessionManager, Arc<Session>, mpsc::Receiver<Command>) {
        let manager = SessionManager::default();
        let request:RequestSpec=serde_json::from_value(json!({"protocol":{"kind":"mcp"},"id":"r","name":"Quota","method":"GET","url":"https://example.com/","description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":false,"verify_tls":true,"assertions":[],"examples":[]})).unwrap();
        let summary = manager
            .register(
                "owner",
                "w",
                &request,
                "MCP".into(),
                PreparedFeedback::default(),
            )
            .unwrap();
        let session = manager.owned("owner", &summary.id).unwrap();
        session.record.lock().unwrap().summary.state = SessionState::Open;
        let receiver = session.receiver.lock().unwrap().take().unwrap();
        (manager, session, receiver)
    }
    fn cancel() -> SendMessage {
        SendMessage::McpCancel {
            request_id: "small".into(),
        }
    }
    #[test]
    fn exact_command_quota_rejects_513th_and_queue_failure_preserves_admission() {
        let (_manager, session, mut receiver) = session();
        for _ in 0..32 {
            send(&session, cancel()).unwrap();
        }
        assert!(send(&session, cancel()).is_err());
        assert_eq!(session.record.lock().unwrap().grpc_input_messages, 32);
        for _ in 0..32 {
            receiver.try_recv().unwrap();
        }
        for _ in 32..512 {
            send(&session, cancel()).unwrap();
            receiver.try_recv().unwrap();
        }
        assert_eq!(session.record.lock().unwrap().grpc_input_messages, 512);
        assert!(send(&session, cancel()).is_err());
        assert!(receiver.try_recv().is_err());
    }
    #[test]
    fn masked_object_key_collision_withholds_the_entire_object() {
        let mask: PrivacyMask = Arc::new(|text| text.replace("secret", "[REDACTED]"));
        let value = safe_value(json!({"secret":"one","[REDACTED]":"two"}), &mask).unwrap();
        assert_eq!(value, "[REDACTED: object key collision]");
    }
}
