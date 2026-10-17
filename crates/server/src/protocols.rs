use crate::{ApiError, AppState, auth::Identity, execution, privacy::Redactor, workspaces::owned};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
};
use moleapi_core::{DataSource, Pair, Protocol, RequestSpec, Response, VariableUpdate};
use moleapi_protocols::{
    DataCompletion, DataOutcome, EventBatch, EventMessage, SendMessage, SessionSummary,
};
use serde::Deserialize;
use std::sync::Arc;
type SharedMask = Arc<std::sync::RwLock<Arc<Redactor>>>;
#[derive(Deserialize)]
pub struct Create {
    workspace_id: String,
    request: RequestSpec,
    environment_id: Option<String>,
    #[serde(default)]
    locals: Vec<VariableUpdate>,
}
#[derive(Deserialize)]
pub struct Cursor {
    #[serde(default)]
    after: u64,
}
fn error(e: anyhow::Error) -> ApiError {
    let text = e.to_string();
    if text == "Session not found" {
        ApiError::not_found()
    } else if text.contains("capacity") || text.contains("queue is full") {
        ApiError {
            status: axum::http::StatusCode::TOO_MANY_REQUESTS,
            message: text,
        }
    } else {
        ApiError::bad(text)
    }
}
pub(crate) struct DataCompletionContext {
    state: AppState,
    owner: String,
    session_id: String,
    workspace_id: String,
    workspace: moleapi_core::Workspace,
    request: RequestSpec,
    prepared_request: RequestSpec,
    admission: Arc<tokio::sync::Mutex<u64>>,
    generation: u64,
    scopes: tokio::sync::Mutex<moleapi_core::VariableScopes>,
    processing: tokio::sync::watch::Sender<Option<String>>,
    mask_state: SharedMask,
}
impl DataCompletionContext {
    fn refresh_mask(&self, values: &std::collections::BTreeSet<String>) -> Result<(), ApiError> {
        let redactor = Arc::new(Redactor::new(values)?);
        *self.mask_state.write().unwrap() = redactor;
        Ok(())
    }
}
async fn check(s: &AppState, owner: &str, id: &str) -> Result<SessionSummary, ApiError> {
    let summary = s.protocol_sessions.summary(owner, id).map_err(error)?;
    let workspace = owned(s, owner, &summary.workspace_id).await?;
    if !workspace
        .data
        .collections
        .iter()
        .any(|c| c.requests.iter().any(|r| r.id == summary.request_id))
    {
        return Err(ApiError::not_found());
    }
    Ok(summary)
}
pub async fn create(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    headers: axum::http::HeaderMap,
    Json(c): Json<Create>,
) -> Result<Json<SessionSummary>, ApiError> {
    let admission = s.protocol_admission.owner(&owner.0)?;
    let generation = {
        let generation = admission.lock().await;
        crate::auth::still_authenticated(&s, &owner.0, &headers).await?;
        *generation
    };
    let w = owned(&s, &owner.0, &c.workspace_id).await?;
    let collection = w
        .data
        .collections
        .iter()
        .find(|collection| collection.requests.iter().any(|r| r.id == c.request.id))
        .ok_or_else(ApiError::not_found)?;
    if c.request.protocol == Protocol::Http {
        return Err(ApiError::bad("HTTP requests use the execute API"));
    }
    if c.request.protocol.is_dubbo() {
        return Err(ApiError::bad("Classic Dubbo calls use the execute API"));
    }
    if let Some(id) = &c.request.specification_id
        && !w.data.specifications.iter().any(|s| &s.id == id)
    {
        return Err(ApiError::not_found());
    }
    let environment = execution::environment(&w, c.environment_id.as_deref())?;
    let mut scopes =
        execution::variables(&s, &w, Some(collection), environment, &[], &[], &c.locals)?;
    let (mut request, mut feedback, mut updates, mut request_updates) =
        execution::prepare_live(&s, &owner.0, &w, &c.request, collection, &mut scopes).await?;
    if let Protocol::A2a { config } = &mut request.protocol
        && let Some(spec_id) = &request.specification_id
    {
        let spec = w
            .data
            .specifications
            .iter()
            .find(|s| &s.id == spec_id)
            .ok_or_else(ApiError::not_found)?;
        if spec.kind != "a2a-agent-card" || spec.dialect != format!("a2a-{}", config.dialect) {
            return Err(ApiError::bad(
                "Select an Agent Card matching the A2A dialect",
            ));
        }
        config.card_source = Some(spec.source.clone());
    }
    if request.protocol.is_graphql() {
        if !moleapi_core::graphql_is_subscription(&request)
            .map_err(|e| ApiError::bad(e.to_string()))?
        {
            return Err(ApiError::bad(
                "GraphQL query/mutation requests use the execute API",
            ));
        }
    } else if !request.protocol.is_a2a()
        && !request.protocol.is_mcp()
        && !request.protocol.is_mqtt()
        && !request.protocol.is_grpc()
        && (request.method != "GET" || request.body_kind != "none")
    {
        return Err(ApiError::bad(
            "SSE and WebSocket connections require GET with body mode None",
        ));
    }
    if matches!((&request.protocol,&c.request.protocol),(Protocol::A2a{config},Protocol::A2a{config:original}) if config.dialect!=original.dialect || config.transport!=original.transport)
        || request.protocol.is_a2a() != c.request.protocol.is_a2a()
        || request.protocol.is_mcp() != c.request.protocol.is_mcp()
        || matches!((&request.protocol, &c.request.protocol), (Protocol::Mcp { config }, Protocol::Mcp { config: original }) if config.transport != original.transport)
        || request.protocol.is_mqtt() != c.request.protocol.is_mqtt()
        || request.protocol.is_tcp() != c.request.protocol.is_tcp()
        || request.protocol.is_socketio() != c.request.protocol.is_socketio()
        || request.protocol.is_graphql() != c.request.protocol.is_graphql()
        || request.protocol.is_grpc() != c.request.protocol.is_grpc()
        || (!request.protocol.is_a2a()
            && !request.protocol.is_mcp()
            && !request.protocol.is_mqtt()
            && !request.protocol.is_socketio()
            && !request.protocol.is_graphql()
            && !request.protocol.is_grpc()
            && request.protocol != c.request.protocol)
    {
        return Err(ApiError::bad("Pre scripts cannot change the live protocol"));
    }
    let mask_state: SharedMask = Arc::new(std::sync::RwLock::new(Arc::new(Redactor::new(
        &scopes.private_values,
    )?)));
    let redactor = mask_state.clone();
    let mask: Arc<dyn Fn(&str) -> String + Send + Sync> = Arc::new(move |text| {
        let mut value = serde_json::json!(text);
        redactor.read().unwrap().scrub(&mut value);
        value.as_str().unwrap_or("[REDACTED]").into()
    });
    let grpc_method = if request.protocol.is_grpc() {
        let id = request
            .specification_id
            .as_ref()
            .ok_or_else(|| ApiError::bad("Select a protobuf specification"))?;
        let spec = w
            .data
            .specifications
            .iter()
            .find(|spec| &spec.id == id)
            .ok_or_else(ApiError::not_found)?;
        let pool = moleapi_core::protobuf_pool(spec).map_err(|e| ApiError::bad(e.to_string()))?;
        let method =
            moleapi_core::grpc_method(&pool, &request).map_err(|e| ApiError::bad(e.to_string()))?;
        let Protocol::Grpc { message_source, .. } = &request.protocol else {
            unreachable!()
        };
        moleapi_core::grpc_message(method.input(), message_source)
            .map_err(|e| ApiError::bad(mask(&e.to_string())))?;
        Some(method)
    } else {
        None
    };
    for log in &mut feedback.logs {
        log.message = mask(&log.message);
    }
    for test in &mut feedback.tests {
        test.name = mask(&test.name);
        test.actual = mask(&test.actual);
        test.expected = mask(&test.expected);
    }
    // Returned updates are creation-only and carry no local/private values into saved drafts.
    updates.retain(|u| {
        !matches!(u.scope.as_str(), "local" | "vault")
            && u.value.as_ref().is_none_or(|v| mask(v) == *v)
    });
    request_updates.retain(|u| mask(&u.value) == u.value);
    let (target_url, ws_url) = match &request.protocol {
        Protocol::Graphql {
            subscription_url: Some(url),
            ..
        } => (url.as_str(), true),
        _ => (
            request.url.as_str(),
            request.protocol == Protocol::Websocket || request.protocol.is_socketio(),
        ),
    };
    if let Protocol::Mcp { config } = &request.protocol
        && config.transport == "stdio"
        && !s.local
    {
        let allowed = std::env::var_os("MOLEAPI_MCP_STDIO_ALLOWLIST")
            .map(|value| std::env::split_paths(&value).collect::<Vec<_>>())
            .unwrap_or_default();
        if !allowed
            .iter()
            .any(|path| path == std::path::Path::new(&config.command))
        {
            return Err(ApiError::bad(
                "MCP STDIO launch is disabled or executable is not administrator-allowed",
            ));
        }
    }
    let stdio =
        matches!(&request.protocol, Protocol::Mcp { config } if config.transport == "stdio");
    let mut resolved_url = if let Protocol::Data { config } = &request.protocol {
        if config.source == moleapi_core::DataSource::LocalFile {
            moleapi_core::protocol_url("http://data.invalid/", false)
        } else {
            moleapi_core::data_url(target_url, config.source)
        }
    } else if stdio {
        moleapi_core::protocol_url("http://mcp.invalid/", false)
    } else if request.protocol.is_tcp() {
        moleapi_core::tcp_url(target_url)
    } else if request.protocol.is_mqtt() {
        moleapi_core::mqtt_url(target_url)
    } else {
        moleapi_core::protocol_url(target_url, ws_url)
    }
    .map_err(|e| ApiError::bad(e.to_string()))?;
    if let Protocol::Socketio { path, .. } = &request.protocol {
        resolved_url.set_path(path);
    }
    for query in request.query.iter().filter(|p| p.enabled) {
        resolved_url
            .query_pairs_mut()
            .append_pair(&query.key, &query.value);
    }
    // Logout and registration/start share this owner gate. Preparation runs outside
    // it; a revoked generation or token cannot cross the final admission boundary.
    let admitted_generation = admission.lock().await;
    if *admitted_generation != generation {
        return Err(ApiError::unauthorized());
    }
    crate::auth::still_authenticated(&s, &owner.0, &headers).await?;
    let mut summary = s
        .protocol_sessions
        .register(
            &owner.0,
            &c.workspace_id,
            &request,
            if stdio {
                "MCP STDIO".into()
            } else {
                mask(&moleapi_core::redact_url(
                    resolved_url.as_str(),
                    Some(&scopes.effective()),
                ))
            },
            feedback,
        )
        .map_err(error)?;
    // Registration comes first: concurrent deletion can now cancel the connecting record.
    match check(&s, &owner.0, &summary.id).await {
        Ok(_) => {}
        Err(e) => {
            let _ = s.protocol_sessions.remove(&owner.0, &summary.id).await;
            return Err(e);
        }
    }
    let data_completions = if request.protocol.is_data() {
        match s.protocol_sessions.configure_data(
            &owner.0,
            &summary.id,
            scopes.effective(),
            &s.script_worker,
        ) {
            Ok(receiver) => Some(receiver),
            Err(e) => {
                let _ = s.protocol_sessions.remove(&owner.0, &summary.id).await;
                return Err(error(e));
            }
        }
    } else {
        None
    };
    if request.protocol.is_tcp()
        && let Err(e) = s.protocol_sessions.configure_tcp(
            &owner.0,
            &summary.id,
            &request,
            scopes.effective(),
            scopes.private_values.iter().any(|value| !value.is_empty()),
        )
    {
        let _ = s.protocol_sessions.remove(&owner.0, &summary.id).await;
        return Err(error(e));
    }
    if let Protocol::Mqtt { config } = &request.protocol
        && let Err(e) = s.protocol_sessions.configure_mqtt(
            &owner.0,
            &summary.id,
            *config.clone(),
            scopes.effective(),
            mask.clone(),
        )
    {
        let _ = s.protocol_sessions.remove(&owner.0, &summary.id).await;
        return Err(error(e));
    }
    if request.protocol.is_socketio()
        && let Err(e) = s.protocol_sessions.configure_socketio(
            &owner.0,
            &summary.id,
            scopes.effective(),
            mask.clone(),
        )
    {
        let _ = s.protocol_sessions.remove(&owner.0, &summary.id).await;
        return Err(error(e));
    }
    if request.protocol.is_a2a()
        && let Err(e) = s
            .protocol_sessions
            .configure_a2a(&owner.0, &summary.id, scopes.effective())
    {
        let _ = s.protocol_sessions.remove(&owner.0, &summary.id).await;
        return Err(error(e));
    }
    if request.protocol.is_mcp()
        && let Err(error) =
            s.protocol_sessions
                .configure_mcp(&owner.0, &summary.id, scopes.effective())
    {
        let _ = s.protocol_sessions.remove(&owner.0, &summary.id).await;
        return Err(crate::protocols::error(error));
    }
    if let Some(method) = grpc_method {
        if let Err(e) = s.protocol_sessions.configure_grpc(
            &owner.0,
            &summary.id,
            method,
            scopes.effective(),
            mask.clone(),
        ) {
            let _ = s.protocol_sessions.remove(&owner.0, &summary.id).await;
            return Err(error(e));
        }
        summary = s
            .protocol_sessions
            .summary(&owner.0, &summary.id)
            .map_err(error)?;
    }
    let prepared_data_request = request.clone();
    if let Err(e) = s.protocol_sessions.start(
        &owner.0,
        &summary.id,
        request,
        moleapi_core::NetworkPolicy {
            allow_private_network: s.local || s.config.allow_private_network,
        },
        mask,
    ) {
        let _ = s.protocol_sessions.remove(&owner.0, &summary.id).await;
        return Err(error(e));
    }
    if let Some(mut completions) = data_completions {
        let context = Arc::new(DataCompletionContext {
            state: s.clone(),
            owner: owner.0.clone(),
            session_id: summary.id.clone(),
            workspace_id: w.id.clone(),
            workspace: w.clone(),
            request: c.request.clone(),
            prepared_request: prepared_data_request,
            admission: admission.clone(),
            generation,
            scopes: tokio::sync::Mutex::new(scopes),
            processing: tokio::sync::watch::channel(None).0,
            mask_state: mask_state.clone(),
        });
        {
            let mut sessions = s.data_sessions.lock().unwrap();
            sessions.retain(|_, context| context.strong_count() > 0);
            sessions.insert(summary.id.clone(), Arc::downgrade(&context));
        }
        tokio::spawn(async move {
            while let Some(completion) = completions.recv().await {
                // Workspace deletion uses this gate for its tombstone and history cascade.
                let _workspace_gate = context
                    .state
                    .webhooks
                    .gates
                    .lock(&context.owner, &context.workspace_id)
                    .await;
                let query_id = completion.query_id.clone();
                let result = save_data_completion(&context, completion).await;
                let message = match result {
                    Ok(Some((history_id, response, notice))) => EventMessage::DataProcessed {
                        query_id: query_id.clone(),
                        history_id,
                        notice,
                        tests: response.tests,
                        logs: response.logs,
                        variable_updates: response.variable_updates,
                    },
                    Ok(None) => break,
                    Err(e) => EventMessage::DataPostprocessError {
                        query_id: query_id.clone(),
                        message: e.message,
                    },
                };
                if context
                    .state
                    .protocol_sessions
                    .data_event(&context.owner, &context.session_id, message)
                    .is_err()
                {
                    let _ = context.state.protocol_sessions.data_event(
                        &context.owner,
                        &context.session_id,
                        EventMessage::DataPostprocessError {
                            query_id: query_id.clone(),
                            message: "Data postprocessing result exceeds session event limits"
                                .into(),
                        },
                    );
                }
                if context.processing.borrow().as_deref() == Some(query_id.as_str()) {
                    context.processing.send_replace(None);
                }
            }
            context
                .state
                .data_sessions
                .lock()
                .unwrap()
                .remove(&context.session_id);
        });
    }
    summary.variable_updates = updates;
    summary.request_updates = request_updates;
    Ok(Json(summary))
}
async fn save_data_completion(
    context: &DataCompletionContext,
    completion: DataCompletion,
) -> Result<Option<(Option<String>, Response, Option<String>)>, ApiError> {
    let guard = context.admission.lock().await;
    if *guard != context.generation
        || check(&context.state, &context.owner, &context.session_id)
            .await
            .is_err()
    {
        return Ok(None);
    }
    let w = owned(&context.state, &context.owner, &context.workspace_id).await?;
    let mut scopes = context.scopes.lock().await;
    let history_cleared = crate::history::cleared_since(
        &context.state,
        &context.owner,
        &context.workspace_id,
        completion.completed_at,
    );
    let collection = context
        .workspace
        .data
        .collections
        .iter()
        .find(|c| c.requests.iter().any(|r| r.id == context.request.id))
        .ok_or_else(ApiError::not_found)?;
    let source = match &context.prepared_request.protocol {
        Protocol::Data { config } => config.source,
        _ => return Err(ApiError::bad("Expected Data request")),
    };
    let terminal_elapsed_ms = completion.elapsed_ms;
    let (status, status_text, elapsed_ms, truncated, body, oversized) = match completion.outcome {
        DataOutcome::Success(result) => {
            let elapsed = result.elapsed_ms;
            let truncated = result.truncated;
            let metadata = serde_json::json!({
                "format":"moleapi-data-result-v1", "query_id":completion.query_id,
                "sql":completion.sql, "columns":result.columns,
                "rows_affected":result.rows_affected, "elapsed_ms":elapsed,
                "truncated":truncated, "limit_reason":result.limit_reason,
            });
            let mut body = metadata.clone();
            body["rows"] = serde_json::to_value(result.rows).map_err(|_| ApiError::internal())?;
            let mut serialized = serde_json::to_string(&body).map_err(|_| ApiError::internal())?;
            let oversized = serialized.len() > 5 * 1024 * 1024;
            if oversized {
                let mut metadata = metadata;
                metadata["rows_withheld"] = "result exceeds the 5 MiB script/history body limit".into();
                serialized = serde_json::to_string(&metadata).map_err(|_| ApiError::internal())?;
            }
            (200, "Data query".to_owned(), elapsed, truncated, serialized, oversized)
        }
        DataOutcome::Error(message) => (500, "Data query error".to_owned(), terminal_elapsed_ms, false,
            serde_json::json!({"format":"moleapi-data-result-v1","query_id":completion.query_id,"sql":completion.sql,"error":message,"elapsed_ms":terminal_elapsed_ms}).to_string(), false),
        DataOutcome::Cancelled { write_outcome_unknown } => (499, "Data query cancelled".to_owned(), terminal_elapsed_ms, false,
            serde_json::json!({"format":"moleapi-data-result-v1","query_id":completion.query_id,"sql":completion.sql,"cancelled":true,"write_outcome_unknown":write_outcome_unknown,"elapsed_ms":terminal_elapsed_ms}).to_string(), false),
    };
    let mut response = Response {
        skipped: false,
        private_auth_values: vec![],
        soap_fault: None,
        request_updates: vec![],
        logs: vec![],
        variable_updates: vec![],
        status,
        status_text,
        headers: vec![Pair {
            id: String::new(),
            key: "content-type".into(),
            value: "application/vnd.moleapi.data+json".into(),
            enabled: true,
            secret: None,
            local_value: None,
        }],
        size_bytes: body.len(),
        body,
        body_base64: None,
        elapsed_ms,
        truncated,
        url: context.prepared_request.url.clone(),
        tests: vec![],
    };
    let notice = if oversized {
        Some("body_limit".to_owned())
    } else if history_cleared {
        Some("history_cleared".to_owned())
    } else {
        None
    };
    let redact_failed_response = if oversized || status != 200 {
        false
    } else {
        execution::postprocess_data(
            &context.state,
            &context.owner,
            &context.workspace,
            collection,
            &context.prepared_request,
            &mut response,
            &mut scopes,
        )
        .await?
    };
    let mut stored = response.clone();
    if matches!(source, DataSource::LocalFile | DataSource::RemoteFile) {
        let mut body: serde_json::Value =
            serde_json::from_str(&stored.body).map_err(|_| ApiError::internal())?;
        if let Some(object) = body.as_object_mut() {
            if status == 200 && !oversized {
                object.remove("rows");
                object.insert(
                    "rows_withheld".into(),
                    "file-backed rows are live-only".into(),
                );
            } else if status != 200 && object.contains_key("error") {
                object.insert(
                    "error".into(),
                    "[WITHHELD: file query diagnostics are live-only]".into(),
                );
            }
        }
        stored.body = body.to_string();
        stored.size_bytes = stored.body.len();
        stored.tests.clear();
    }
    let history_id = if history_cleared {
        None
    } else {
        crate::history::record(
            &context.state,
            &context.owner,
            &w,
            &context.request,
            Some(&scopes.effective()),
            &stored,
            crate::privacy::HistoryPrivacy {
                values: &scopes.private_values,
                redact_failed_response,
            },
        )
        .await?
    };
    context.refresh_mask(&scopes.private_values)?;
    drop(guard);
    if history_id.is_none() && !history_cleared {
        return Ok(None);
    }
    Ok(Some((history_id, response, notice)))
}
pub async fn get(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
) -> Result<Json<SessionSummary>, ApiError> {
    Ok(Json(check(&s, &owner.0, &id).await?))
}
pub async fn events(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
    Query(cursor): Query<Cursor>,
) -> Result<Json<EventBatch>, ApiError> {
    check(&s, &owner.0, &id).await?;
    Ok(Json(
        s.protocol_sessions
            .events(&owner.0, &id, cursor.after)
            .map_err(error)?,
    ))
}
pub async fn send(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
    Json(message): Json<SendMessage>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let summary = check(&s, &owner.0, &id).await?;
    if let SendMessage::DataQuery {
        query_id,
        sql,
        read_only,
    } = &message
        && summary.protocol == "data"
    {
        let context = s
            .data_sessions
            .lock()
            .unwrap()
            .get(&id)
            .and_then(std::sync::Weak::upgrade)
            .ok_or_else(ApiError::not_found)?;
        if let Protocol::Data { config } = &context.prepared_request.protocol
            && !config.query_pre_script.trim().is_empty()
        {
            send_scripted_data_query(
                &s,
                &owner.0,
                &id,
                &context,
                query_id.clone(),
                sql.clone(),
                *read_only,
            )
            .await?;
            return Ok(Json(serde_json::json!({"ok":true})));
        }
    }
    s.protocol_sessions
        .send(&owner.0, &id, message)
        .map_err(error)?;
    Ok(Json(serde_json::json!({"ok":true})))
}

async fn send_scripted_data_query(
    s: &AppState,
    owner: &str,
    id: &str,
    context: &DataCompletionContext,
    query_id: String,
    selected_sql: String,
    read_only: bool,
) -> Result<(), ApiError> {
    let reservation = s
        .protocol_sessions
        .reserve_data_query(owner, id, query_id.clone(), &selected_sql)
        .map_err(error)?;
    s.protocol_sessions
        .data_event(
            owner,
            id,
            EventMessage::DataPreparing {
                query_id: query_id.clone(),
            },
        )
        .map_err(error)?;
    let mut processing = context.processing.subscribe();
    while processing.borrow().is_some() {
        tokio::select! {
            _ = reservation.cancelled() => {
                let _ = s.protocol_sessions.data_event(owner, id, EventMessage::DataCancelled { query_id, write_outcome_unknown:false });
                return Ok(());
            }
            changed = tokio::time::timeout(std::time::Duration::from_secs(30), processing.changed()) => {
                if !matches!(changed, Ok(Ok(()))) {
                    let _ = s.protocol_sessions.data_event(owner, id, EventMessage::DataPreprocessError {
                        query_id, message:"Previous Data query postprocessing is unavailable".into(),
                    });
                    return Ok(());
                }
            }
        }
    }
    let _workspace_gate = s.webhooks.gates.lock(owner, &context.workspace_id).await;
    let admitted = context.admission.lock().await;
    if *admitted != context.generation || check(s, owner, id).await.is_err() {
        return Err(ApiError::unauthorized());
    }
    let mut scopes = context.scopes.lock().await;
    let before = scopes.clone();
    let prepared = tokio::select! {
        _ = reservation.cancelled() => {
            let _ = s.protocol_sessions.data_event(owner, id, EventMessage::DataCancelled { query_id, write_outcome_unknown:false });
            return Ok(());
        }
        outcome = execution::prepare_data_query(s, owner, &context.workspace, &context.prepared_request, &selected_sql, read_only, &mut scopes) => outcome,
    };
    let prepared = match prepared {
        Ok(prepared) => prepared,
        Err(failure) => {
            let _ = context.refresh_mask(&scopes.private_values);
            let _ = s.protocol_sessions.data_event(
                owner,
                id,
                EventMessage::DataPreprocessError {
                    query_id,
                    message: failure.message,
                },
            );
            return Ok(());
        }
    };
    if context.refresh_mask(&scopes.private_values).is_err() {
        let _ = s.protocol_sessions.data_event(
            owner,
            id,
            EventMessage::DataPreprocessError {
                query_id,
                message: "Data query privacy redaction unavailable".into(),
            },
        );
        return Ok(());
    }
    if reservation.cancelled_now() {
        let private = scopes.private_values.clone();
        *scopes = before;
        scopes.private_values.extend(private);
        let _ = s.protocol_sessions.data_event(
            owner,
            id,
            EventMessage::DataCancelled {
                query_id,
                write_outcome_unknown: false,
            },
        );
        return Ok(());
    }
    let event = EventMessage::DataPrepared {
        query_id: query_id.clone(),
        sql_changed: prepared
            .sql
            .as_ref()
            .is_some_and(|sql| sql != &selected_sql),
        tests: prepared.tests,
        logs: prepared.logs,
        variable_updates: prepared.updates,
    };
    if s.protocol_sessions.data_event(owner, id, event).is_err() {
        let private = scopes.private_values.clone();
        *scopes = before;
        scopes.private_values.extend(private);
        let _ = s.protocol_sessions.data_event(
            owner,
            id,
            EventMessage::DataPreprocessError {
                query_id,
                message: "Data query preparation output exceeds session event limits".into(),
            },
        );
        return Ok(());
    }
    let Some(sql) = prepared.sql else {
        let _ = s
            .protocol_sessions
            .data_event(owner, id, EventMessage::DataSkipped { query_id });
        return Ok(());
    };
    context.processing.send_replace(Some(query_id.clone()));
    let source = match &context.prepared_request.protocol {
        Protocol::Data { config } => config,
        _ => unreachable!(),
    };
    let effective_read_only = read_only
        || source.read_only
        || matches!(
            source.source,
            moleapi_core::DataSource::LocalFile | moleapi_core::DataSource::RemoteFile
        );
    if let Err(failure) = reservation.dispatch(sql, effective_read_only) {
        context.processing.send_replace(None);
        let private = scopes.private_values.clone();
        *scopes = before;
        scopes.private_values.extend(private);
        let _ = s.protocol_sessions.data_event(
            owner,
            id,
            EventMessage::DataPreprocessError {
                query_id,
                message: failure.to_string(),
            },
        );
    }
    Ok(())
}
pub async fn close(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
) -> Result<Json<SessionSummary>, ApiError> {
    check(&s, &owner.0, &id).await?;
    Ok(Json(
        s.protocol_sessions
            .close(&owner.0, &id)
            .await
            .map_err(error)?,
    ))
}
pub async fn delete(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    check(&s, &owner.0, &id).await?;
    s.protocol_sessions
        .remove(&owner.0, &id)
        .await
        .map_err(error)?;
    Ok(Json(serde_json::json!({"ok":true})))
}
