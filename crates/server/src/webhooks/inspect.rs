use super::{api::owned_inbox, models::*, storage as vault};
use crate::{ApiError, AppState, auth::Identity, workspaces::owned};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use moleapi_core::{Pair, Workspace};
use serde::Serialize;
#[derive(Serialize)]
pub struct CaptureView {
    #[serde(flatten)]
    pub capture: Capture,
    pub body_text: Option<String>,
    pub redacted: bool,
}
fn screening_workspace(
    workspace: &Workspace,
    inbox: &Inbox,
    capture: &Capture,
) -> Result<(Workspace, String), ApiError> {
    let mut workspace = workspace.clone();
    workspace.data.global_variables.push(Pair {
        id: uuid::Uuid::new_v4().to_string(),
        key: "webhook_secret".into(),
        value: inbox.token.clone(),
        enabled: true,
        secret: Some(true),
        local_value: None,
    });
    let body = STANDARD
        .decode(&capture.body_base64)
        .map_err(|_| ApiError::internal())?;
    let text = std::str::from_utf8(&body).unwrap_or_default();
    let content_type = capture
        .headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case("content-type"))
        .and_then(|h| h.value_text.as_deref())
        .unwrap_or("");
    let id = format!("webhook-inspect-{}", capture.id);
    let request:moleapi_core::RequestSpec=serde_json::from_value(serde_json::json!({"id":id,"name":"Webhook capture","method":"POST","url":format!("https://webhook.invalid/?{}",capture.query),"description":"","query":[],"headers":capture.headers.iter().map(|h|serde_json::json!({"id":uuid::Uuid::new_v4().to_string(),"key":h.name,"value":h.value_text.as_deref().unwrap_or(&h.value_base64),"secret":h.value_text.is_none(),"enabled":true})).collect::<Vec<_>>(),"body_kind":if content_type.starts_with("application/x-www-form-urlencoded"){"form"}else{"text"},"body":text,"auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":false,"verify_tls":true,"assertions":[],"examples":[]})).map_err(|_|ApiError::internal())?;
    let mut collection:moleapi_core::Collection=serde_json::from_value(serde_json::json!({"id":uuid::Uuid::new_v4().to_string(),"name":"Webhook inspection","description":"","requests":[]})).map_err(|_|ApiError::internal())?;
    let mut captured = moleapi_core::VariableScopes::default();
    crate::privacy::request_values(&request, &mut captured)?;
    for value in captured.private_values {
        workspace.data.global_variables.push(Pair {
            id: uuid::Uuid::new_v4().to_string(),
            key: "webhook_private".into(),
            value,
            enabled: true,
            secret: Some(true),
            local_value: None,
        });
    }
    collection.requests.push(request);
    workspace.data.collections.push(collection);
    Ok((workspace, id))
}
pub(super) fn view(
    workspace: &Workspace,
    inbox: &Inbox,
    mut capture: Capture,
    include: bool,
) -> Result<CaptureView, ApiError> {
    let body = STANDARD
        .decode(&capture.body_base64)
        .map_err(|_| ApiError::internal())?;
    if include {
        return Ok(CaptureView {
            body_text: std::str::from_utf8(&body).ok().map(String::from),
            capture,
            redacted: false,
        });
    }
    let (screen, id) = screening_workspace(workspace, inbox, &capture)?;
    let safe = moleapi_formats::generation_request(&screen, &id, false).map_err(|_| {
        ApiError::bad("Capture cannot be safely screened; explicit private view is required")
    })?;
    let url = url::Url::parse(&safe.url).map_err(|_| ApiError::internal())?;
    let query = url.query().unwrap_or_default().to_owned();
    let mut changed = query != capture.query;
    capture.query = query;
    let mut headers = Vec::new();
    for (original, pair) in capture.headers.into_iter().zip(safe.headers) {
        let value = pair.value;
        changed |= original.value_text.as_deref() != Some(&value) || original.name != pair.key;
        headers.push(Header {
            name: pair.key,
            value_base64: STANDARD.encode(value.as_bytes()),
            value_text: Some(value),
        });
    }
    capture.headers = headers;
    let body_text = if std::str::from_utf8(&body).is_ok() {
        changed |= safe.body.as_bytes() != body;
        capture.body_base64 = STANDARD.encode(safe.body.as_bytes());
        Some(safe.body)
    } else {
        changed = true;
        capture.body_base64 = String::new();
        None
    };
    Ok(CaptureView {
        capture,
        body_text,
        redacted: changed,
    })
}
pub async fn list(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
    Query(query): Query<Inspect>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if query.search.len() > 256 {
        return Err(ApiError::bad("Capture search exceeds limit"));
    }
    let inbox = owned_inbox(&s, &owner.0, &id).await?;
    let workspace = owned(&s, &owner.0, &inbox.workspace_id).await?;
    let captures = vault::captures(&s.db, &owner.0, &id).await?;
    let mut values = Vec::new();
    for capture in captures.into_iter().filter(|c| c.cursor > query.after) {
        let value = view(&workspace, &inbox, capture, query.include_secrets)?;
        if query.search.is_empty()
            || serde_json::to_string(&value)
                .map_err(|_| ApiError::internal())?
                .contains(&query.search)
        {
            values.push(value);
        }
    }
    values.sort_by_key(|v| v.capture.cursor);
    Ok(Json(
        serde_json::json!({"captures":values,"received":inbox.received,"dropped":inbox.dropped,"revision":inbox.revision}),
    ))
}
pub(super) async fn capture(
    s: &AppState,
    owner: &str,
    inbox: &Inbox,
    id: &str,
) -> Result<Capture, ApiError> {
    vault::captures(&s.db, owner, &inbox.id)
        .await?
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(ApiError::not_found)
}
pub async fn get(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path((id, capture_id)): Path<(String, String)>,
    Query(query): Query<Inspect>,
) -> Result<Json<CaptureView>, ApiError> {
    let inbox = owned_inbox(&s, &owner.0, &id).await?;
    let workspace = owned(&s, &owner.0, &inbox.workspace_id).await?;
    let capture = capture(&s, &owner.0, &inbox, &capture_id).await?;
    Ok(Json(view(
        &workspace,
        &inbox,
        capture,
        query.include_secrets,
    )?))
}
#[derive(serde::Deserialize)]
pub struct Export {
    #[serde(default)]
    pub include_secrets: bool,
}
pub async fn export(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
    Json(input): Json<Export>,
) -> Result<Json<moleapi_formats::ExportResult>, ApiError> {
    let inbox = owned_inbox(&s, &owner.0, &id).await?;
    let workspace = owned(&s, &owner.0, &inbox.workspace_id).await?;
    let mut values = vault::captures(&s.db, &owner.0, &id)
        .await?
        .into_iter()
        .map(|c| view(&workspace, &inbox, c, input.include_secrets))
        .collect::<Result<Vec<_>, _>>()?;
    values.sort_by_key(|v| v.capture.cursor);
    let mut config = inbox;
    if !input.include_secrets {
        // Screen configured responses with the same mature privacy path as captures.
        let synthetic = Capture {
            id: "receiver-response".into(),
            inbox_id: config.id.clone(),
            workspace_id: config.workspace_id.clone(),
            cursor: 0,
            received_at: config.updated_at.clone(),
            method: "POST".into(),
            query: String::new(),
            headers: config
                .response
                .headers
                .iter()
                .map(|h| Header {
                    name: h.key.clone(),
                    value_base64: STANDARD.encode(&h.value),
                    value_text: Some(h.value.clone()),
                })
                .collect(),
            body_base64: STANDARD.encode(&config.response.body),
            body_bytes: config.response.body.len(),
            response_status: config.response.status,
        };
        let mut screen = workspace.clone();
        for h in config
            .response
            .headers
            .iter()
            .filter(|h| h.secret == Some(true))
        {
            screen.data.global_variables.push(Pair {
                id: uuid::Uuid::new_v4().to_string(),
                key: "response_private".into(),
                value: h.value.clone(),
                enabled: true,
                secret: Some(true),
                local_value: None,
            });
        }
        let safe = view(&screen, &config, synthetic.clone(), false)?;
        let (screen, request_id) = screening_workspace(&screen, &config, &synthetic)?;
        let mut named = screen.clone();
        for collection in &mut named.data.collections {
            for request in &mut collection.requests {
                if request.id == request_id {
                    request.body_kind = "text".into();
                    request.body = config.name.clone();
                }
            }
        }
        config.name = moleapi_formats::generation_request(&named, &request_id, false)
            .map_err(|_| ApiError::internal())?
            .body;
        config.response.body = safe.body_text.unwrap_or_default();
        for (h, screened) in config.response.headers.iter_mut().zip(safe.capture.headers) {
            h.value = screened.value_text.unwrap_or_default();
        }
        config.token = String::new();
    }
    let content=serde_json::to_string_pretty(&serde_json::json!({"schema":"moleapi-webhooks/1","receiver":config,"captures":values,"include_secrets":input.include_secrets})).map_err(|_|ApiError::internal())?;
    Ok(Json(moleapi_formats::ExportResult {
        warnings: vec![],
        filename: "webhook-captures.json".into(),
        content,
        mime: "application/json".into(),
    }))
}
