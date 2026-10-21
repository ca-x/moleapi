//! Host config export copies reuse native workspace redaction before names become JSON keys.
use crate::{ApiError, AppState, auth::Identity, workspaces::owned};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use moleapi_core::{RequestSpec, Workspace};
use serde::Deserialize;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpConfigExport {
    pub request_id: String,
    #[serde(default)]
    pub include_secrets: bool,
    pub host_name: Option<String>,
}
pub(crate) fn export_request(
    mut workspace: Workspace,
    input: McpConfigExport,
) -> Result<RequestSpec, ApiError> {
    let request = workspace
        .data
        .collections
        .iter_mut()
        .flat_map(|c| &mut c.requests)
        .find(|r| r.id == input.request_id && r.protocol.is_mcp())
        .ok_or_else(ApiError::not_found)?;
    if let Some(name) = input.host_name {
        if name.len() > 4096 {
            return Err(ApiError::bad("MCP Host name exceeds4096 bytes"));
        }
        request.name = name;
    }
    let exported = moleapi_formats::export(&workspace, "moleapi", input.include_secrets)
        .map_err(|e| ApiError::bad(e.to_string()))?;
    let snapshot: Workspace =
        serde_json::from_str(&exported.content).map_err(|_| ApiError::internal())?;
    let mut request = snapshot
        .data
        .collections
        .into_iter()
        .flat_map(|c| c.requests)
        .find(|r| r.id == input.request_id)
        .ok_or_else(ApiError::not_found)?;
    if !input.include_secrets {
        request.auth = serde_json::from_value(
            serde_json::json!({"kind":"none","token":"","username":"","password":""}),
        )
        .map_err(|_| ApiError::internal())?;
    }
    Ok(request)
}
pub async fn export(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(workspace): Path<String>,
    Json(input): Json<McpConfigExport>,
) -> Result<Json<RequestSpec>, ApiError> {
    let canonical = owned(&state, &owner.0, &workspace).await?;
    tokio::task::spawn_blocking(move || export_request(canonical, input))
        .await
        .map_err(|_| ApiError::internal())?
        .map(Json)
}
