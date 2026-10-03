use crate::{ApiError, AppState, auth::Identity, execution, privacy::Redactor, workspaces::owned};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
};
use moleapi_core::{Protocol, RequestSpec, VariableUpdate};
use moleapi_protocols::{EventBatch, SendMessage, SessionSummary};
use serde::Deserialize;
use std::sync::Arc;
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
    if let Some(id) = &c.request.specification_id
        && !w.data.specifications.iter().any(|s| &s.id == id)
    {
        return Err(ApiError::not_found());
    }
    let environment = execution::environment(&w, c.environment_id.as_deref())?;
    let mut scopes =
        execution::variables(&s, &w, Some(collection), environment, &[], &[], &c.locals)?;
    let (request, mut feedback, mut updates, mut request_updates) =
        execution::prepare_live(&s, &w, &c.request, collection, &mut scopes).await?;
    if request.protocol.is_graphql() {
        if !moleapi_core::graphql_is_subscription(&request)
            .map_err(|e| ApiError::bad(e.to_string()))?
        {
            return Err(ApiError::bad(
                "GraphQL query/mutation requests use the execute API",
            ));
        }
    } else if !request.protocol.is_mqtt()
        && !request.protocol.is_grpc()
        && (request.method != "GET" || request.body_kind != "none")
    {
        return Err(ApiError::bad(
            "SSE and WebSocket connections require GET with body mode None",
        ));
    }
    if request.protocol.is_mqtt() != c.request.protocol.is_mqtt()
        || request.protocol.is_socketio() != c.request.protocol.is_socketio()
        || request.protocol.is_graphql() != c.request.protocol.is_graphql()
        || request.protocol.is_grpc() != c.request.protocol.is_grpc()
        || (!request.protocol.is_mqtt()
            && !request.protocol.is_socketio()
            && !request.protocol.is_graphql()
            && !request.protocol.is_grpc()
            && request.protocol != c.request.protocol)
    {
        return Err(ApiError::bad("Pre scripts cannot change the live protocol"));
    }
    let redactor = Arc::new(Redactor::new(&scopes.private_values)?);
    let mask: Arc<dyn Fn(&str) -> String + Send + Sync> = Arc::new(move |text| {
        let mut value = serde_json::json!(text);
        redactor.scrub(&mut value);
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
    let mut resolved_url = if request.protocol.is_mqtt() {
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
            mask(&moleapi_core::redact_url(
                resolved_url.as_str(),
                Some(&scopes.effective()),
            )),
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
    summary.variable_updates = updates;
    summary.request_updates = request_updates;
    Ok(Json(summary))
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
    check(&s, &owner.0, &id).await?;
    s.protocol_sessions
        .send(&owner.0, &id, message)
        .map_err(error)?;
    Ok(Json(serde_json::json!({"ok":true})))
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
