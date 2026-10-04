use crate::{ApiError, AppState, auth::Identity, workspaces::owned};
use axum::{Extension, Json, extract::State};
use serde::Deserialize;
use serde_json::{Value, json};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Generate {
    workspace_id: String,
    request_id: String,
    target: String,
    client: String,
    #[serde(default)]
    include_secrets: bool,
}
pub async fn catalog(State(s): State<AppState>) -> Result<Json<Value>, ApiError> {
    let slot = s
        .generation_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::bad("Generation capacity reached; retry shortly"))?;
    tokio::task::spawn_blocking(move || {
        let _slot=slot;
        moleapi_generation::catalog().map(|targets|Json(json!({"engine":moleapi_generation::ENGINE,"targets":targets,"validation":"engine-smoke","scope":"HTTP request snippets; SDK/server generation is not included"})))
    }).await.map_err(|_|ApiError::internal())?.map_err(|_|ApiError::internal())
}
pub async fn generate(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Json(c): Json<Generate>,
) -> Result<Json<moleapi_generation::Snippet>, ApiError> {
    if c.workspace_id.len() > 128
        || c.request_id.len() > 128
        || c.target.len() > 32
        || c.client.len() > 32
    {
        return Err(ApiError::bad("Invalid generation identifiers"));
    }
    let workspace = owned(&s, &owner.0, &c.workspace_id).await?;
    let slot = s
        .generation_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::bad("Generation capacity reached; retry shortly"))?;
    tokio::task::spawn_blocking(move || {
        let _slot = slot;
        moleapi_generation::generate_request(
            &workspace,
            &c.request_id,
            &c.target,
            &c.client,
            c.include_secrets,
        )
    })
    .await
    .map_err(|_| ApiError::internal())?
    .map(Json)
    .map_err(|e| ApiError::bad(e.to_string()))
}
