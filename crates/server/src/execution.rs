use crate::{ApiError, AppState, auth::Identity, workspaces::owned};
use axum::{Extension, Json, extract::State};
use moleapi_core::{Environment, RequestSpec, Response, Workspace};
use serde::Deserialize;
#[derive(Deserialize)]
pub struct Execute {
    workspace_id: String,
    request: RequestSpec,
    environment_id: Option<String>,
}
pub(crate) fn environment<'a>(
    w: &'a Workspace,
    id: Option<&str>,
) -> Result<Option<&'a Environment>, ApiError> {
    let id = id.or(w.data.active_environment_id.as_deref());
    id.map(|id| {
        w.data
            .environments
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| ApiError::bad("Environment not found"))
    })
    .transpose()
}
pub(crate) async fn perform(
    s: &AppState,
    owner: &str,
    w: &Workspace,
    r: &RequestSpec,
    environment: Option<&Environment>,
) -> Result<Response, ApiError> {
    // Validate before networking so malformed requests get a useful 400.
    moleapi_core::resolve_request(r, environment).map_err(|e| ApiError::bad(e.to_string()))?;
    let response = moleapi_core::execute(
        r,
        environment,
        moleapi_core::NetworkPolicy {
            allow_private_network: s.local || s.config.allow_private_network,
        },
    )
    .await
    .map_err(|e| ApiError::bad(e.to_string()))?;
    crate::history::record(s, owner, w, r, environment, &response).await?;
    Ok(response)
}
pub async fn execute(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Json(c): Json<Execute>,
) -> Result<Json<Response>, ApiError> {
    let w = owned(&s, &owner.0, &c.workspace_id).await?;
    let e = environment(&w, c.environment_id.as_deref())?;
    Ok(Json(perform(&s, &owner.0, &w, &c.request, e).await?))
}
