pub(crate) mod callbacks;
pub(crate) mod flows;
pub(crate) mod token_actions;
use oauth2::TokenResponse;
mod vault;
use crate::{ApiError, AppState, auth::Identity, workspaces::owned};
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::HeaderMap,
};
use moleapi_core::{NetworkPolicy, OAuth2Auth, VariableUpdate};
use serde::Deserialize;
#[derive(Deserialize)]
pub(crate) struct GrantInput {
    workspace_id: String,
    config: OAuth2Auth,
    #[serde(default)]
    label: String,
    collection_id: Option<String>,
    environment_id: Option<String>,
    #[serde(default)]
    locals: Vec<VariableUpdate>,
    #[serde(default = "verify_tls")]
    verify_tls: bool,
}
fn verify_tls() -> bool {
    true
}
async fn configuration(
    s: &AppState,
    owner: &str,
    input: &GrantInput,
) -> Result<OAuth2Auth, ApiError> {
    let w = owned(s, owner, &input.workspace_id).await?;
    let collection = input
        .collection_id
        .as_deref()
        .map(|id| {
            w.data
                .collections
                .iter()
                .find(|c| c.id == id)
                .ok_or_else(ApiError::not_found)
        })
        .transpose()?;
    let environment = crate::execution::environment(&w, input.environment_id.as_deref())?;
    let scopes =
        crate::execution::variables(s, &w, collection, environment, &[], &[], &input.locals)?;
    let effective = scopes.effective();
    let request:moleapi_core::RequestSpec=serde_json::from_value(serde_json::json!({"id":"oauth2-config","name":"OAuth2","method":"GET","url":"https://oauth2.invalid/","description":"","headers":[],"query":[],"body_kind":"none","body":"","auth":{"kind":"oauth2","token":"","username":"","password":"","oauth2":input.config},"timeout_ms":30000,"verify_tls":input.verify_tls,"follow_redirects":false,"assertions":[],"examples":[]})).map_err(|_|ApiError::bad("Invalid OAuth2 configuration"))?;
    moleapi_core::resolve_request(&request, Some(&effective))
        .map_err(|e| ApiError::bad(e.to_string()))?
        .auth
        .oauth2
        .map(|o| *o)
        .ok_or_else(ApiError::internal)
}
fn policy(s: &AppState) -> NetworkPolicy {
    NetworkPolicy {
        allow_private_network: s.local || s.config.allow_private_network,
    }
}
pub(crate) async fn acquire(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    headers: HeaderMap,
    Json(input): Json<GrantInput>,
) -> Result<Json<vault::Metadata>, ApiError> {
    if input.label.len() > 128 || input.label.chars().any(char::is_control) {
        return Err(ApiError::bad("Invalid OAuth2 token label"));
    }
    let gate = s.protocol_admission.owner(&owner.0)?;
    let epoch = *gate.lock().await;
    let config = configuration(&s, &owner.0, &input).await?;
    let _permit = s
        .oauth2_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError {
            status: axum::http::StatusCode::TOO_MANY_REQUESTS,
            message: "OAuth2 grant capacity reached".into(),
        })?;
    let response = moleapi_core::oauth2_acquire(&config, policy(&s), input.verify_tls)
        .await
        .map_err(|e| ApiError::bad(e.to_string()))?;
    let admission = gate.lock().await;
    if *admission != epoch {
        return Err(ApiError::bad("OAuth2 account context changed"));
    }
    crate::auth::still_authenticated(&s, &owner.0, &headers).await?;
    owned(&s, &owner.0, &input.workspace_id).await?;
    Ok(Json(
        vault::create(
            &s,
            &owner.0,
            &input.workspace_id,
            if input.label.is_empty() {
                "OAuth2 token".into()
            } else {
                input.label
            },
            &config,
            response,
        )
        .await?
        .metadata(),
    ))
}
pub(crate) async fn list(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(workspace): Path<String>,
) -> Result<Json<Vec<vault::Metadata>>, ApiError> {
    owned(&s, &owner.0, &workspace).await?;
    Ok(Json(
        crate::storage::documents::<vault::Token>(
            &s.db,
            &owner.0,
            "oauth2-token",
            Some(&workspace),
            128,
        )
        .await?
        .iter()
        .map(vault::Token::metadata)
        .collect(),
    ))
}
pub(crate) async fn remove(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path((workspace, id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    owned(&s, &owner.0, &workspace).await?;
    vault::remove(&s, &owner.0, &workspace, &id).await?;
    Ok(Json(serde_json::json!({"ok":true})))
}

async fn refresh_token(
    s: &AppState,
    owner: &str,
    workspace: &str,
    id: &str,
    config: &OAuth2Auth,
    verify_tls: bool,
    headers: Option<&HeaderMap>,
) -> Result<vault::Token, ApiError> {
    let _permit = s
        .oauth2_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError {
            status: axum::http::StatusCode::TOO_MANY_REQUESTS,
            message: "OAuth2 grant capacity reached".into(),
        })?;
    let gate = s.protocol_admission.owner(owner)?;
    let epoch = *gate.lock().await;
    let (token, revision) = vault::lease(s, owner, workspace, id, config).await?;
    let response = if let Some(refresh) = token.response.refresh_token() {
        moleapi_core::oauth2_refresh(config, refresh.secret(), policy(s), verify_tls).await
    } else if config.grant == moleapi_core::OAuth2Grant::ClientCredentials {
        moleapi_core::oauth2_acquire(config, policy(s), verify_tls).await
    } else {
        Err(anyhow::anyhow!(
            "OAuth2 token has no refresh credential; authorize again"
        ))
    };
    match response {
        Ok(response) => {
            let admission = gate.lock().await;
            if *admission != epoch {
                vault::release(s, owner, token, revision).await?;
                return Err(ApiError::bad("OAuth2 account context changed"));
            }
            if let Some(headers) = headers
                && let Err(error) = crate::auth::still_authenticated(s, owner, headers).await
            {
                vault::release(s, owner, token, revision).await?;
                return Err(error);
            }
            if owned(s, owner, workspace).await.is_err() {
                vault::release(s, owner, token, revision).await?;
                return Err(ApiError::not_found());
            }
            match vault::finish(s, owner, token.clone(), revision, response).await {
                Ok(token) => Ok(token),
                Err(error) => {
                    vault::release(s, owner, token, revision).await?;
                    Err(error)
                }
            }
        }
        Err(error) => {
            vault::release(s, owner, token, revision).await?;
            Err(ApiError::bad(error.to_string()))
        }
    }
}
pub(crate) async fn refresh(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    headers: HeaderMap,
    Path((workspace, id)): Path<(String, String)>,
    Json(mut input): Json<GrantInput>,
) -> Result<Json<vault::Metadata>, ApiError> {
    if input.workspace_id != workspace {
        return Err(ApiError::bad("OAuth2 workspace mismatch"));
    }
    input.config.token_id = Some(id.clone());
    let config = configuration(&s, &owner.0, &input).await?;
    Ok(Json(
        refresh_token(
            &s,
            &owner.0,
            &workspace,
            &id,
            &config,
            input.verify_tls,
            Some(&headers),
        )
        .await?
        .metadata(),
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
    if input.label.is_empty()
        || input.label.len() > 128
        || input.label.chars().any(char::is_control)
    {
        return Err(ApiError::bad("Invalid OAuth2 token label"));
    }
    let (mut token, revision) = vault::load(&s, &owner.0, &workspace, &id).await?;
    if token
        .lease
        .as_ref()
        .is_some_and(|lease| lease.expires_at > chrono::Utc::now().timestamp())
    {
        return Err(ApiError::bad("OAuth2 token is being refreshed"));
    }
    token.label = input.label;
    if !vault::replace(&s, &owner.0, &token, revision).await? {
        return Err(ApiError::bad("OAuth2 token changed; retry rename"));
    }
    Ok(Json(token.metadata()))
}
pub(crate) async fn reveal(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path((workspace, id)): Path<(String, String)>,
) -> Result<
    (
        [(axum::http::header::HeaderName, &'static str); 1],
        Json<serde_json::Value>,
    ),
    ApiError,
> {
    owned(&s, &owner.0, &workspace).await?;
    let (token, _) = vault::load(&s, &owner.0, &workspace, &id).await?;
    Ok((
        [(axum::http::header::CACHE_CONTROL, "no-store")],
        Json(
            serde_json::json!({"access_token":token.response.access_token().secret(),"refresh_token":token.response.refresh_token().map(|t|t.secret())}),
        ),
    ))
}
pub(crate) async fn prepare(
    s: &AppState,
    owner: &str,
    workspace: &str,
    request: &moleapi_core::RequestSpec,
    scopes: &mut moleapi_core::VariableScopes,
) -> Result<moleapi_core::RequestSpec, ApiError> {
    if request.auth.kind != "oauth2" {
        return Ok(request.clone());
    }
    let config = request
        .auth
        .oauth2
        .as_ref()
        .ok_or_else(|| ApiError::bad("Configure OAuth2 authentication"))?;
    let id = config
        .token_id
        .as_deref()
        .filter(|id| !id.is_empty())
        .ok_or_else(|| ApiError::bad("Acquire and select an OAuth2 token before executing"))?;
    let (mut token, _) = vault::load(s, owner, workspace, id).await?;
    if token.revoked || token.profile != vault::profile(config)? {
        return Err(ApiError::bad(
            "OAuth2 token does not match the current environment/profile",
        ));
    }
    if token
        .expires_at
        .is_some_and(|expires| expires <= chrono::Utc::now().timestamp() + 5)
    {
        if !config.auto_refresh {
            return Err(ApiError::bad(
                "OAuth2 token has expired; refresh or authorize again",
            ));
        }
        token = refresh_token(s, owner, workspace, id, config, request.verify_tls, None).await?;
    }
    if token
        .expires_at
        .is_some_and(|expires| expires <= chrono::Utc::now().timestamp())
    {
        return Err(ApiError::bad(
            "OAuth2 provider returned an expired token; authorize again",
        ));
    }
    if token
        .response
        .token_type()
        .as_ref()
        .eq_ignore_ascii_case("mac")
    {
        return Err(ApiError::bad(
            "OAuth2 MAC access tokens require a dedicated provider",
        ));
    }
    scopes
        .private_values
        .insert(token.response.access_token().secret().clone());
    if let Some(refresh) = token.response.refresh_token() {
        scopes.private_values.insert(refresh.secret().clone());
    }
    scopes
        .validate()
        .map_err(|_| ApiError::bad("OAuth2 credentials exceed private execution limits"))?;
    let mut prepared = request.clone();
    prepared.auth.kind = "apikey".into();
    prepared.auth.api_key = Some(Box::new(moleapi_core::ApiKeyAuth {
        name: config.name.clone(),
        location: config.location,
        value: if config.prefix.is_empty() {
            token.response.access_token().secret().clone()
        } else {
            format!(
                "{} {}",
                config.prefix,
                token.response.access_token().secret()
            )
        },
    }));
    Ok(prepared)
}
