pub(crate) mod callbacks;
mod configuration;
pub(crate) mod flows;
mod vault;
use crate::{ApiError, AppState, auth::Identity, workspaces::owned};
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::HeaderMap,
};
use configuration::Input;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::Deserialize;
fn policy(s: &AppState) -> moleapi_core::NetworkPolicy {
    moleapi_core::NetworkPolicy {
        allow_private_network: s.local || s.config.allow_private_network,
    }
}
pub(crate) async fn prepare(
    s: &AppState,
    owner: &str,
    workspace: &str,
    source: &moleapi_core::RequestSpec,
    resolved: &moleapi_core::RequestSpec,
    scopes: &mut moleapi_core::VariableScopes,
) -> Result<moleapi_core::RequestSpec, ApiError> {
    if resolved.auth.kind != "oauth1" {
        return Ok(resolved.clone());
    }
    let c = resolved
        .auth
        .oauth1
        .as_ref()
        .ok_or_else(|| ApiError::bad("Configure OAuth1 authentication"))?;
    let Some(id) = c.token_id.as_deref() else {
        return Ok(resolved.clone());
    };
    let g = source.auth.oauth1.as_ref().and_then(|c| c.grant.as_deref());
    let profile = configuration::profile(c, g, &scopes.effective())?;
    let (token, _) = vault::load(s, owner, workspace, id).await?;
    if token.profile != profile.hash()? {
        return Err(ApiError::bad(
            "OAuth1 token does not match the current environment/profile",
        ));
    }
    scopes.private_values.extend([
        token.credentials.token.clone(),
        token.credentials.secret.clone(),
    ]);
    scopes
        .validate()
        .map_err(|_| ApiError::bad("OAuth1 credentials exceed private execution limits"))?;
    let mut r = resolved.clone();
    let c = r.auth.oauth1.as_mut().unwrap();
    c.token = token.credentials.token;
    c.token_secret = token.credentials.secret;
    c.token_id = None;
    Ok(r)
}
pub(crate) async fn list(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
) -> Result<Json<Vec<vault::Metadata>>, ApiError> {
    owned(&s, &owner.0, &id).await?;
    let tokens: Vec<vault::Token> =
        crate::storage::documents(&s.db, &owner.0, "oauth1-token", Some(&id), 128).await?;
    Ok(Json(tokens.into_iter().map(|t| t.metadata()).collect()))
}
#[derive(Deserialize)]
pub(crate) struct Import {
    #[serde(flatten)]
    input: Input,
    token: moleapi_core::OAuth1Credentials,
}
pub(crate) async fn import(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    headers: HeaderMap,
    Json(input): Json<Import>,
) -> Result<Json<vault::Metadata>, ApiError> {
    let gate = s.protocol_admission.owner(&owner.0)?;
    let _admission = gate.lock().await;
    crate::auth::still_authenticated(&s, &owner.0, &headers).await?;
    let _workspace = s
        .webhooks
        .gates
        .lock(&owner.0, &input.input.workspace_id)
        .await;
    let resolved = configuration::resolve(&s, &owner.0, &input.input, false).await?;
    Ok(Json(
        vault::create(
            &s,
            &owner.0,
            &input.input.workspace_id,
            resolved.label,
            &resolved.profile,
            input.token,
        )
        .await?
        .metadata(),
    ))
}
pub(crate) async fn reveal(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path((workspace, id)): Path<(String, String)>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    owned(&s, &owner.0, &workspace).await?;
    Ok((
        [("cache-control", "no-store")],
        Json(
            vault::load(&s, &owner.0, &workspace, &id)
                .await?
                .0
                .credentials,
        ),
    ))
}
#[derive(Deserialize)]
pub(crate) struct Rename {
    label: String,
}
pub(crate) async fn rename(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path((workspace, id)): Path<(String, String)>,
    Json(input): Json<Rename>,
) -> Result<Json<vault::Metadata>, ApiError> {
    owned(&s, &owner.0, &workspace).await?;
    let label = configuration::label(&input.label)?;
    let (token, revision) = vault::load(&s, &owner.0, &workspace, &id).await?;
    Ok(Json(
        vault::rename(&s, &owner.0, token, revision, label).await?,
    ))
}
pub(crate) async fn remove(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path((workspace, id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    owned(&s, &owner.0, &workspace).await?;
    let result = crate::entities::document::Entity::delete_many()
        .filter(crate::entities::document::Column::Id.eq(vault::key(&owner.0, &workspace, &id)))
        .filter(crate::entities::document::Column::Owner.eq(&owner.0))
        .filter(crate::entities::document::Column::Kind.eq("oauth1-token"))
        .filter(crate::entities::document::Column::RefId.eq(&workspace))
        .exec(&s.db)
        .await?;
    if result.rows_affected != 1 {
        return Err(ApiError::not_found());
    }
    Ok(Json(serde_json::json!({"ok":true})))
}
