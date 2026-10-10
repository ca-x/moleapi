use crate::{
    ApiError, AppState,
    entities::{access_token, account, session, setting},
};
use argon2::{
    Argon2, PasswordHasher, PasswordVerifier,
    password_hash::{PasswordHash, SaltString, rand_core::OsRng},
};
use axum::{
    Extension, Json,
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};
use rand::RngCore;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait, sea_query::Expr,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Clone)]
pub struct Identity(pub String);
#[derive(Clone)]
pub(crate) struct SessionAuthentication;

async fn credential(state: &AppState, token: &str) -> Result<Option<(String, bool)>, ApiError> {
    let now = chrono::Utc::now().timestamp();
    if let Some(secret) = token.strip_prefix(crate::access_tokens::PREFIX) {
        if secret.len() != 64 || !secret.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Ok(None);
        }
        return Ok(access_token::Entity::find()
            .filter(access_token::Column::Digest.eq(token_hash(token)))
            .filter(access_token::Column::ExpiresAt.gt(now))
            .one(&state.db)
            .await?
            .map(|token| (token.owner, false)));
    }
    Ok(session::Entity::find_by_id(token_hash(token))
        .filter(session::Column::ExpiresAt.gt(now))
        .one(&state.db)
        .await?
        .map(|session| (session.owner, true)))
}
#[derive(Deserialize)]
pub struct Credentials {
    pub username: String,
    pub password: String,
    pub setup_token: Option<String>,
}
pub fn token_hash(token: &str) -> String {
    hex::encode(Sha256::digest(token))
}
pub async fn setup_required(state: &AppState) -> Result<bool, ApiError> {
    Ok(setting::Entity::find_by_id("registration")
        .one(&state.db)
        .await?
        .is_some_and(|s| s.revision == 0))
}
pub async fn status(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(
        serde_json::json!({"mode":if state.local {"desktop"} else {"server"},"setup_required":!state.local && setup_required(&state).await?,"registration_enabled":state.config.allow_registration}),
    ))
}
async fn new_session(
    state: &AppState,
    owner: &str,
    username: &str,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let token = hex::encode(bytes);
    session::ActiveModel {
        id: Set(token_hash(&token)),
        owner: Set(owner.into()),
        expires_at: Set(chrono::Utc::now().timestamp() + 30 * 86400),
    }
    .insert(&state.db)
    .await?;
    Ok(Json(serde_json::json!({"token":token,"username":username})))
}
pub async fn register(
    State(state): State<AppState>,
    Json(c): Json<Credentials>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if state.local {
        return Err(ApiError::not_found());
    }
    if !(3..=64).contains(&c.username.chars().count())
        || c.username.trim() != c.username
        || c.username.chars().any(char::is_control)
        || c.password.len() < 10
        || c.password.len() > 1024
    {
        return Err(ApiError::bad(
            "Username must be 3–64 characters and password 10–1024 bytes",
        ));
    }
    let password = c.password;
    let hash = tokio::task::spawn_blocking(move || {
        Argon2::default()
            .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
            .map(|h| h.to_string())
    })
    .await
    .map_err(|_| ApiError::internal())?
    .map_err(|_| ApiError::internal())?;
    let tx = state.db.begin().await?;
    // This single row update serializes registrations on every supported engine.
    setting::Entity::update_many()
        .col_expr(
            setting::Column::Revision,
            Expr::col(setting::Column::Revision).add(1),
        )
        .filter(setting::Column::Id.eq("registration"))
        .exec(&tx)
        .await?;
    let count = setting::Entity::find_by_id("registration")
        .one(&tx)
        .await?
        .ok_or_else(ApiError::internal)?
        .revision;
    if count == 1 {
        if c.setup_token.as_deref() != Some(state.config.setup_token.as_str())
            || state.config.setup_token.is_empty()
        {
            return Err(ApiError::forbidden(
                "First registration requires the setup token",
            ));
        }
    } else if !state.config.allow_registration {
        return Err(ApiError::forbidden("Registration is disabled"));
    }
    let id = uuid::Uuid::new_v4().to_string();
    account::ActiveModel {
        id: Set(id.clone()),
        username: Set(c.username.clone()),
        password_hash: Set(hash),
    }
    .insert(&tx)
    .await
    .map_err(|e| {
        if matches!(
            e.sql_err(),
            Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
        ) {
            ApiError::conflict("Username already exists")
        } else {
            ApiError::from(e)
        }
    })?;
    tx.commit().await?;
    new_session(&state, &id, &c.username).await
}
pub async fn login(
    State(state): State<AppState>,
    Json(c): Json<Credentials>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if state.local {
        return Err(ApiError::not_found());
    }
    if c.password.len() > 1024 || !(3..=64).contains(&c.username.chars().count()) {
        return Err(ApiError::unauthorized());
    }
    let user = account::Entity::find()
        .filter(account::Column::Username.eq(&c.username))
        .one(&state.db)
        .await?;
    // A fixed valid Argon2 hash keeps unknown accounts on the expensive verification path.
    let hash=user.as_ref().map(|u|u.password_hash.clone()).unwrap_or_else(||"$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHQxMjM0NTY3OA$wWXgduGU2RRVpbIQnuAyXyYuk7XL8OvFaHvGa8lL9yE".into());
    let valid = tokio::task::spawn_blocking(move || {
        PasswordHash::new(&hash).ok().is_some_and(|h| {
            Argon2::default()
                .verify_password(c.password.as_bytes(), &h)
                .is_ok()
        })
    })
    .await
    .map_err(|_| ApiError::internal())?;
    let Some(user) = user.filter(|_| valid) else {
        return Err(ApiError::unauthorized());
    };
    new_session(&state, &user.id, &user.username).await
}
pub async fn guard(State(state): State<AppState>, mut request: Request, next: Next) -> Response {
    if state.local {
        request.extensions_mut().insert(Identity("local".into()));
        return next.run(request).await;
    }
    let token = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    let Some(token) = token else {
        return ApiError::unauthorized().into_response();
    };
    match credential(&state, token).await {
        Ok(Some((owner, session))) => {
            request.extensions_mut().insert(Identity(owner));
            if session {
                request.extensions_mut().insert(SessionAuthentication);
            }
            next.run(request).await
        }
        Ok(None) => ApiError::unauthorized().into_response(),
        Err(_) => ApiError::internal().into_response(),
    }
}
pub async fn logout(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let gate = state.protocol_admission.owner(&owner.0)?;
    let mut generation = gate.lock().await;
    let tx = state.db.begin().await?;
    if let Some(token) = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        if token.starts_with(crate::access_tokens::PREFIX) {
            access_token::Entity::delete_many()
                .filter(access_token::Column::Digest.eq(token_hash(token)))
                .filter(access_token::Column::Owner.eq(&owner.0))
                .exec(&tx)
                .await?;
        } else {
            session::Entity::delete_by_id(token_hash(token))
                .exec(&tx)
                .await?;
        }
    }
    if state.local {
        tx.commit().await?;
        *generation = generation.checked_add(1).ok_or_else(ApiError::internal)?;
        stop_owner(&state, &owner.0).await;
    } else {
        let revision = crate::credential_revocations::publish(&tx, &owner.0).await?;
        tx.commit().await?;
        crate::credential_revocations::apply_locked(&state, &owner.0, revision, &mut generation)
            .await?;
    }
    Ok(Json(serde_json::json!({"ok":true})))
}

pub(crate) async fn stop_owner(state: &AppState, owner: &str) {
    state.cookies.clear_scope(owner, None);
    state.oauth2_flows.cancel_owner(owner);
    state.oauth1_flows.cancel_owner(owner);
    state.webhooks.cancel_scope(owner, None, None);
    state.project_jobs.stop_owner(owner);
    state.protocol_sessions.close_owner(owner).await;
}

/// Recheck a previously authenticated call under its owner admission gate.
pub(crate) async fn still_authenticated(
    state: &AppState,
    owner: &str,
    headers: &axum::http::HeaderMap,
) -> Result<(), ApiError> {
    if state.local {
        return Ok(());
    }
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(ApiError::unauthorized)?;
    let active = credential(state, token).await?;
    if active.is_none_or(|(identity, _)| identity != owner) {
        return Err(ApiError::unauthorized());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
        routing::get,
    };
    use sea_orm::sea_query::Expr;
    use tower::ServiceExt;
    #[tokio::test]
    async fn session_storage_contains_only_digest_and_expired_sessions_are_rejected() {
        let state = AppState {
            credential_revisions: std::sync::Arc::default(),
            db: crate::storage::connect("sqlite::memory:").await.unwrap(),
            config: crate::Config {
                database_url: "sqlite::memory:".into(),
                setup_token: "setup".into(),
                allow_registration: false,
                allow_private_network: false,
            },
            local: false,
            sync_lock: std::sync::Arc::new(tokio::sync::Mutex::new(())),
            script_slots: std::sync::Arc::new(tokio::sync::Semaphore::new(4)),
            generation_slots: std::sync::Arc::new(tokio::sync::Semaphore::new(4)),
            project_slots: std::sync::Arc::new(tokio::sync::Semaphore::new(2)),
            project_jobs: std::sync::Arc::default(),
            project_runtime: std::sync::Arc::new(
                moleapi_generation::project::ProjectRuntime::new(
                    std::env::current_exe().unwrap(),
                    None,
                )
                .unwrap(),
            ),
            cookies: std::sync::Arc::default(),
            oauth1_slots: std::sync::Arc::new(tokio::sync::Semaphore::new(8)),
            oauth1_flows: std::sync::Arc::default(),
            oauth2_slots: std::sync::Arc::new(tokio::sync::Semaphore::new(8)),
            oauth2_flows: std::sync::Arc::default(),
            protocol_sessions: moleapi_protocols::SessionManager::new(),
            a2a_sources: std::sync::Arc::new(crate::a2a::Sources::default()),
            webhooks: std::sync::Arc::new(crate::webhooks::Hub::default()),
            protocol_admission: std::sync::Arc::new(
                crate::protocol_admission::AdmissionGates::default(),
            ),
            script_worker: std::env::current_exe().unwrap(),
        };
        let Json(result) = new_session(&state, "owner", "username").await.unwrap();
        let token = result["token"].as_str().unwrap();
        let stored = session::Entity::find()
            .one(&state.db)
            .await
            .unwrap()
            .unwrap();
        assert_ne!(stored.id, token);
        assert_eq!(stored.id, token_hash(token));
        assert!(stored.expires_at > chrono::Utc::now().timestamp());
        session::Entity::update_many()
            .col_expr(session::Column::ExpiresAt, Expr::value(0_i64))
            .exec(&state.db)
            .await
            .unwrap();
        let router = Router::new()
            .route("/private", get(|| async { "ok" }))
            .route_layer(axum::middleware::from_fn_with_state(state, guard));
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/private")
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
