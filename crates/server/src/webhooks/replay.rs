use super::{api::owned_inbox, inspect, manager::capacity};
use crate::{ApiError, AppState, auth::Identity, workspaces::owned};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use moleapi_core::{NetworkPolicy, Pair, RequestSpec};
use serde::Deserialize;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Replay {
    pub capture_id: String,
    pub replay_id: String,
    pub destination: String,
    pub method: String,
    pub headers: Vec<Pair>,
    pub query: Vec<Pair>,
    pub body_base64: String,
    pub timeout_ms: u64,
    pub verify_tls: bool,
    pub follow_redirects: bool,
    #[serde(default)]
    pub include_credentials: bool,
}
#[derive(Deserialize)]
pub struct Cancel {
    pub replay_id: String,
}
struct Lease {
    hub: Arc<super::Hub>,
    owner: String,
    id: String,
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.hub
            .replay_tasks
            .lock()
            .unwrap()
            .remove(&(self.owner.clone(), self.id.clone()));
    }
}
fn credential(name: &str) -> bool {
    moleapi_core::sensitive_query_key(name)
        || matches!(
            name.to_ascii_lowercase().as_str(),
            "authorization" | "cookie" | "set-cookie" | "proxy-authorization" | "x-api-key"
        )
}
pub async fn run(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
    headers: axum::http::HeaderMap,
    Json(mut input): Json<Replay>,
) -> Result<Json<moleapi_core::Response>, ApiError> {
    if input.replay_id.is_empty()
        || input.replay_id.len() > 128
        || input.headers.len() > 512
        || input.query.len() > 512
        || input.body_base64.len() > 1400000
    {
        return Err(ApiError::bad("Webhook replay exceeds input limits"));
    }
    let owner_gate = s.protocol_admission.owner(&owner.0)?;
    let admission = owner_gate.lock().await;
    crate::auth::still_authenticated(&s, &owner.0, &headers).await?;
    let inbox = owned_inbox(&s, &owner.0, &id).await?;
    let scope = s.webhooks.gates.lock(&owner.0, &inbox.workspace_id).await;
    owned_inbox(&s, &owner.0, &id).await?;
    let _workspace = owned(&s, &owner.0, &inbox.workspace_id).await?;
    inspect::capture(&s, &owner.0, &inbox, &input.capture_id).await?;
    let body = STANDARD
        .decode(&input.body_base64)
        .map_err(|_| ApiError::bad("Invalid replay Base64 body"))?;
    if body.len() > 1048576 {
        return Err(ApiError::bad("Webhook replay body exceeds 1 MiB"));
    }
    let nominated: Vec<String> = input
        .headers
        .iter()
        .filter(|h| h.enabled && h.key.eq_ignore_ascii_case("connection"))
        .flat_map(|h| h.value.split(',').map(|v| v.trim().to_ascii_lowercase()))
        .collect();
    input.headers.retain(|h| {
        !nominated.contains(&h.key.to_ascii_lowercase())
            && (input.include_credentials || (!credential(&h.key) && h.secret != Some(true)))
    });
    if !input.include_credentials {
        let mut destination = url::Url::parse(&input.destination)
            .map_err(|_| ApiError::bad("Invalid replay destination"))?;
        let public: Vec<(String, String)> = destination
            .query_pairs()
            .filter(|(key, _)| !credential(key))
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        let _ = destination.set_username("");
        let _ = destination.set_password(None);
        destination.set_query(None);
        if !public.is_empty() {
            destination.query_pairs_mut().extend_pairs(public);
        }
        input.destination = destination.to_string();
        input
            .query
            .retain(|q| q.secret != Some(true) && !credential(&q.key));
    }
    let request:RequestSpec=serde_json::from_value(serde_json::json!({"id":input.capture_id,"name":"Webhook replay","method":input.method,"url":input.destination,"description":"","query":input.query,"headers":input.headers,"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":input.timeout_ms,"follow_redirects":input.follow_redirects,"verify_tls":input.verify_tls,"assertions":[],"examples":[]})).map_err(|_|ApiError::bad("Invalid replay request"))?;
    moleapi_core::validate_request(&request, false)
        .map_err(|_| ApiError::bad("Invalid Webhook replay configuration"))?;
    let _slot = s
        .webhooks
        .replay
        .clone()
        .try_acquire_owned()
        .map_err(|_| capacity("Webhook replay capacity reached"))?;
    let stop = CancellationToken::new();
    {
        let mut tasks = s.webhooks.replay_tasks.lock().unwrap();
        if tasks.contains_key(&(owner.0.clone(), input.replay_id.clone())) {
            return Err(ApiError::conflict("Replay ID is already active"));
        }
        tasks.insert(
            (owner.0.clone(), input.replay_id.clone()),
            super::manager::ReplayTask {
                workspace: inbox.workspace_id.clone(),
                inbox: id,
                stop: stop.clone(),
            },
        );
    }
    let _lease = Lease {
        hub: s.webhooks.clone(),
        owner: owner.0.clone(),
        id: input.replay_id,
    };
    let mut values = moleapi_core::VariableScopes::default();
    crate::privacy::request_values(&request, &mut values)?;
    let redactor = crate::privacy::Redactor::new(&values.private_values)?;
    drop(scope);
    drop(admission);
    tokio::select! {biased;_=stop.cancelled()=>Err(ApiError::bad("Webhook replay cancelled")),result=moleapi_core::execute_bytes(&request,body,NetworkPolicy{allow_private_network:s.local||s.config.allow_private_network})=>result.map(Json).map_err(|error| {
        let mut diagnostic = serde_json::Value::String(format!("Webhook replay transport failed: {error}"));
        redactor.scrub(&mut diagnostic);
        ApiError::bad(diagnostic.as_str().unwrap_or("Webhook replay transport failed"))
    })}
}
pub async fn cancel(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Json(input): Json<Cancel>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if let Some(stop) = s
        .webhooks
        .replay_tasks
        .lock()
        .unwrap()
        .get(&(owner.0, input.replay_id))
    {
        stop.stop.cancel();
    }
    Ok(Json(serde_json::json!({"cancelled":true})))
}
