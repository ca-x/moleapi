use crate::{ApiError, AppState, auth::Identity, workspaces::owned};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use serde::Deserialize;
#[derive(Deserialize)]
pub struct Import {
    format: String,
    content: String,
}
pub async fn import(
    Json(c): Json<Import>,
) -> Result<Json<moleapi_formats::ImportResult>, ApiError> {
    tokio::task::spawn_blocking(move || moleapi_formats::import(&c.format, &c.content))
        .await
        .map_err(|_| ApiError::internal())?
        .map(Json)
        .map_err(|e| ApiError::bad(e.to_string()))
}
#[derive(Deserialize)]
pub struct Export {
    format: String,
    include_secrets: bool,
}
pub async fn export(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
    Json(c): Json<Export>,
) -> Result<Json<moleapi_formats::ExportResult>, ApiError> {
    let w = owned(&s, &owner.0, &id).await?;
    tokio::task::spawn_blocking(move || moleapi_formats::export(&w, &c.format, c.include_secrets))
        .await
        .map_err(|_| ApiError::internal())?
        .map(Json)
        .map_err(|e| ApiError::bad(e.to_string()))
}

#[derive(Deserialize)]
pub struct VariablePreview {
    content: String,
    expected_revision: i64,
}
pub async fn import_variables_preview(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
    Json(input): Json<VariablePreview>,
) -> Result<Json<moleapi_formats::VariableImport>, ApiError> {
    let workspace = owned(&s, &owner.0, &id).await?;
    if workspace.revision != input.expected_revision {
        return Err(ApiError::conflict("Workspace revision changed"));
    }
    tokio::task::spawn_blocking(move || moleapi_formats::import_postman_variables(&input.content))
        .await
        .map_err(|_| ApiError::internal())?
        .map(Json)
        .map_err(|error| ApiError::bad(error.to_string()))
}

#[derive(Deserialize)]
pub struct VariableExport {
    scope: String,
    environment_id: Option<String>,
    include_secrets: bool,
    expected_revision: i64,
}
pub async fn export_variables(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
    Json(input): Json<VariableExport>,
) -> Result<Json<moleapi_formats::ExportResult>, ApiError> {
    let workspace = owned(&s, &owner.0, &id).await?;
    if workspace.revision != input.expected_revision {
        return Err(ApiError::conflict("Workspace revision changed"));
    }
    tokio::task::spawn_blocking(move || {
        moleapi_formats::export_postman_variables(
            &workspace,
            &input.scope,
            input.environment_id.as_deref(),
            input.include_secrets,
        )
    })
    .await
    .map_err(|_| ApiError::internal())?
    .map(Json)
    .map_err(|error| ApiError::bad(error.to_string()))
}
