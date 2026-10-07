//! Explicit, owner-bound management of provider credentials.
use super::{GrantInput, configuration, policy, vault};
use crate::{ApiError, AppState, auth::Identity, workspaces::owned};
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::{HeaderMap, header::CACHE_CONTROL},
};
use oauth2::{TokenIntrospectionResponse, TokenResponse};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub(crate) struct Inspection {
    active: bool,
    expires_at: Option<i64>,
    scopes: Vec<String>,
}

pub(crate) async fn introspect(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    headers: HeaderMap,
    Path((workspace, id)): Path<(String, String)>,
    Json(input): Json<GrantInput>,
) -> Result<
    (
        [(axum::http::header::HeaderName, &'static str); 1],
        Json<Inspection>,
    ),
    ApiError,
> {
    if input.workspace_id != workspace {
        return Err(ApiError::bad("OAuth2 workspace mismatch"));
    }
    let config = configuration(&s, &owner.0, &input).await?;
    let (token, revision) = vault::load(&s, &owner.0, &workspace, &id).await?;
    if token.revoked || token.profile != vault::profile(&config)? {
        return Err(ApiError::bad(
            "OAuth2 token does not match this authorization profile",
        ));
    }
    let gate = s.protocol_admission.owner(&owner.0)?;
    let epoch = *gate.lock().await;
    let _permit = s
        .oauth2_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::bad("OAuth2 grant capacity reached"))?;
    let response = moleapi_core::oauth2_introspect(
        &config,
        token.response.access_token().secret(),
        policy(&s),
        input.verify_tls,
    )
    .await
    .map_err(|e| ApiError::bad(e.to_string()))?;
    let _admission = gate.lock().await;
    if *_admission != epoch {
        return Err(ApiError::bad("OAuth2 account context changed"));
    }
    crate::auth::still_authenticated(&s, &owner.0, &headers).await?;
    owned(&s, &owner.0, &workspace).await?;
    let (_, latest) = vault::load(&s, &owner.0, &workspace, &id).await?;
    if latest != revision {
        return Err(ApiError::bad("OAuth2 token changed; inspect again"));
    }
    // Return a bounded projection, never arbitrary provider extensions or identity fields.
    let scopes: Vec<String> = response
        .scopes()
        .map(|items| {
            items
                .iter()
                .map(|scope| scope.as_str().to_owned())
                .collect()
        })
        .unwrap_or_default();
    if scopes.len() > 64
        || scopes
            .iter()
            .any(|scope| scope.len() > 1024 || scope.chars().any(char::is_control))
    {
        return Err(ApiError::bad("OAuth2 introspection scopes exceed limits"));
    }
    Ok((
        [(CACHE_CONTROL, "no-store")],
        Json(Inspection {
            active: response.active(),
            expires_at: response.exp().map(|v| v.timestamp()),
            scopes,
        }),
    ))
}

#[derive(Deserialize)]
pub(crate) struct RevokeInput {
    #[serde(flatten)]
    input: GrantInput,
    #[serde(default)]
    refresh_token: bool,
}
pub(crate) async fn revoke(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    headers: HeaderMap,
    Path((workspace, id)): Path<(String, String)>,
    Json(input): Json<RevokeInput>,
) -> Result<Json<vault::Metadata>, ApiError> {
    if input.input.workspace_id != workspace {
        return Err(ApiError::bad("OAuth2 workspace mismatch"));
    }
    let config = configuration(&s, &owner.0, &input.input).await?;
    let _permit = s
        .oauth2_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::bad("OAuth2 grant capacity reached"))?;
    let gate = s.protocol_admission.owner(&owner.0)?;
    let epoch = *gate.lock().await;
    let (mut token, revision) = vault::lease(&s, &owner.0, &workspace, &id, &config).await?;
    let credential = if input.refresh_token {
        token
            .response
            .refresh_token()
            .map(|value| value.secret().as_str())
    } else {
        Some(token.response.access_token().secret().as_str())
    };
    let outcome = match credential {
        Some(credential) => {
            moleapi_core::oauth2_revoke(
                &config,
                credential,
                input.refresh_token,
                policy(&s),
                input.input.verify_tls,
            )
            .await
        }
        None => Err(anyhow::anyhow!("OAuth2 token has no refresh credential")),
    };
    if let Err(error) = outcome {
        vault::release(&s, &owner.0, token, revision).await?;
        return Err(ApiError::bad(error.to_string()));
    }
    // Provider revocation has already happened, even if this session logged out meanwhile.
    // Retire this exact leased revision so a late refresh cannot make it executable again.
    token.revoked = true;
    token.lease = None;
    if !vault::replace(&s, &owner.0, &token, revision).await? {
        return Err(ApiError::bad("OAuth2 token changed during revocation"));
    }
    let admission = gate.lock().await;
    if *admission != epoch {
        return Err(ApiError::bad("OAuth2 account context changed"));
    }
    crate::auth::still_authenticated(&s, &owner.0, &headers).await?;
    owned(&s, &owner.0, &workspace).await?;
    Ok(Json(token.metadata()))
}

#[derive(Deserialize)]
pub(crate) struct ImportInput {
    #[serde(flatten)]
    input: GrantInput,
    token: oauth2::basic::BasicTokenResponse,
}
pub(crate) async fn import(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    headers: HeaderMap,
    Json(input): Json<ImportInput>,
) -> Result<Json<vault::Metadata>, ApiError> {
    if input.input.label.len() > 128 || input.input.label.chars().any(char::is_control) {
        return Err(ApiError::bad("Invalid OAuth2 token label"));
    }
    let gate = s.protocol_admission.owner(&owner.0)?;
    let epoch = *gate.lock().await;
    let config = configuration(&s, &owner.0, &input.input).await?;
    let admission = gate.lock().await;
    if *admission != epoch {
        return Err(ApiError::bad("OAuth2 account context changed"));
    }
    crate::auth::still_authenticated(&s, &owner.0, &headers).await?;
    owned(&s, &owner.0, &input.input.workspace_id).await?;
    let label = if input.input.label.is_empty() {
        "Imported OAuth2 token".into()
    } else {
        input.input.label
    };
    Ok(Json(
        vault::create(
            &s,
            &owner.0,
            &input.input.workspace_id,
            label,
            &config,
            input.token,
        )
        .await?
        .metadata(),
    ))
}
