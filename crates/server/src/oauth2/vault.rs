//! Private credentials use owner/workspace documents and revision CAS for refresh leases.
use crate::{ApiError, AppState, entities::document, storage};
use moleapi_core::OAuth2Auth;
use oauth2::{TokenResponse, basic::BasicTokenResponse};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, sea_query::Expr};
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Token {
    pub id: String,
    pub workspace_id: String,
    pub label: String,
    pub profile: String,
    pub client_id: String,
    pub issuer: String,
    pub response: BasicTokenResponse,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub revoked: bool,
    pub lease: Option<Lease>,
}
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Lease {
    pub id: String,
    pub expires_at: i64,
}
#[derive(Clone, Serialize)]
pub(crate) struct Metadata {
    pub id: String,
    pub label: String,
    pub client_id: String,
    pub issuer: String,
    pub token_type: String,
    pub scopes: Vec<String>,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub has_refresh_token: bool,
    pub revoked: bool,
    pub refreshing: bool,
}
impl Token {
    pub fn metadata(&self) -> Metadata {
        Metadata {
            id: self.id.clone(),
            label: self.label.clone(),
            client_id: self.client_id.clone(),
            issuer: self.issuer.clone(),
            token_type: self.response.token_type().as_ref().to_owned(),
            scopes: self
                .response
                .scopes()
                .map(|s| s.iter().map(|s| s.as_str().into()).collect())
                .unwrap_or_default(),
            created_at: self.created_at,
            expires_at: self.expires_at,
            has_refresh_token: self.response.refresh_token().is_some(),
            revoked: self.revoked,
            refreshing: self
                .lease
                .as_ref()
                .is_some_and(|lease| lease.expires_at > chrono::Utc::now().timestamp()),
        }
    }
}
fn key(owner: &str, workspace: &str, id: &str) -> String {
    storage::workspace_key(owner, &format!("oauth2:{workspace}:{id}"))
}
pub(crate) fn profile(config: &OAuth2Auth) -> Result<String, ApiError> {
    use sha2::{Digest, Sha256};
    let mut binding = config.clone();
    binding.token_id = None;
    binding.client_secret.clear();
    binding.password.clear();
    binding.revocation_url.clear();
    binding.introspection_url.clear();
    binding.name.clear();
    binding.prefix.clear();
    binding.auto_refresh = false;
    binding.location = moleapi_core::AuthLocation::Header;
    Ok(hex::encode(Sha256::digest(
        serde_json::to_vec(&binding).map_err(|_| ApiError::internal())?,
    )))
}
fn expiry(response: &BasicTokenResponse) -> Result<Option<i64>, ApiError> {
    response
        .expires_in()
        .map(|duration| {
            let seconds = i64::try_from(duration.as_secs())
                .map_err(|_| ApiError::bad("OAuth2 expiry exceeds supported range"))?;
            chrono::Utc::now()
                .timestamp()
                .checked_add(seconds)
                .ok_or_else(|| ApiError::bad("OAuth2 expiry exceeds supported range"))
        })
        .transpose()
}
fn validate(response: &BasicTokenResponse) -> Result<(), ApiError> {
    let access = response.access_token().secret();
    if access.is_empty()
        || access.len() > 65536
        || access.chars().any(char::is_control)
        || response
            .refresh_token()
            .is_some_and(|t| t.secret().len() > 65536)
        || response.token_type().as_ref().len() > 128
        || response
            .scopes()
            .is_some_and(|s| s.len() > 64 || s.iter().any(|s| s.as_str().len() > 1024))
    {
        return Err(ApiError::bad(
            "OAuth2 credential response exceeds limits or is invalid",
        ));
    }
    Ok(())
}
pub(crate) async fn create(
    s: &AppState,
    owner: &str,
    workspace: &str,
    label: String,
    config: &OAuth2Auth,
    response: BasicTokenResponse,
) -> Result<Token, ApiError> {
    validate(&response)?;
    let token = Token {
        id: uuid::Uuid::new_v4().to_string(),
        workspace_id: workspace.into(),
        label,
        profile: profile(config)?,
        client_id: config.client_id.clone(),
        issuer: url::Url::parse(&config.token_url)
            .map(|u| u.origin().ascii_serialization())
            .unwrap_or_default(),
        expires_at: expiry(&response)?,
        response,
        created_at: chrono::Utc::now().timestamp(),
        revoked: false,
        lease: None,
    };
    use sea_orm::TransactionTrait;
    let tx = s.db.begin().await?;
    // A no-op write locks the parent on all three engines (including SQLite).
    document::Entity::update_many()
        .col_expr(
            document::Column::Revision,
            Expr::col(document::Column::Revision).into(),
        )
        .filter(document::Column::Id.eq(storage::workspace_key(owner, workspace)))
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq("workspace"))
        .exec(&tx)
        .await?;
    if storage::get(&tx, owner, workspace).await?.is_none() {
        return Err(ApiError::not_found());
    }
    use sea_orm::PaginatorTrait;
    if document::Entity::find()
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq("oauth2-token"))
        .filter(document::Column::RefId.eq(workspace))
        .count(&tx)
        .await?
        >= 128
    {
        return Err(ApiError::bad("OAuth2 token capacity reached for workspace"));
    }

    storage::insert_doc(
        &tx,
        key(owner, workspace, &token.id),
        owner,
        "oauth2-token",
        workspace,
        0,
        &token,
    )
    .await?;
    tx.commit().await?;
    Ok(token)
}
pub(crate) async fn load(
    s: &AppState,
    owner: &str,
    workspace: &str,
    id: &str,
) -> Result<(Token, i64), ApiError> {
    let row = document::Entity::find_by_id(key(owner, workspace, id))
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq("oauth2-token"))
        .filter(document::Column::RefId.eq(workspace))
        .one(&s.db)
        .await?
        .ok_or_else(ApiError::not_found)?;
    let token: Token = serde_json::from_str(&row.payload).map_err(|_| ApiError::internal())?;
    if token.id != id || token.workspace_id != workspace {
        return Err(ApiError::internal());
    }
    Ok((token, row.revision))
}
pub(crate) async fn replace(
    s: &AppState,
    owner: &str,
    token: &Token,
    revision: i64,
) -> Result<bool, ApiError> {
    let next = revision.checked_add(1).ok_or_else(ApiError::internal)?;
    Ok(document::Entity::update_many()
        .col_expr(
            document::Column::Payload,
            Expr::value(serde_json::to_string(token).map_err(|_| ApiError::internal())?),
        )
        .col_expr(document::Column::Revision, Expr::value(next))
        .col_expr(document::Column::UpdatedAt, Expr::value(storage::now()))
        .filter(document::Column::Id.eq(key(owner, &token.workspace_id, &token.id)))
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq("oauth2-token"))
        .filter(document::Column::RefId.eq(&token.workspace_id))
        .filter(document::Column::Revision.eq(revision))
        .exec(&s.db)
        .await?
        .rows_affected
        == 1)
}
pub(crate) async fn lease(
    s: &AppState,
    owner: &str,
    workspace: &str,
    id: &str,
    config: &OAuth2Auth,
) -> Result<(Token, i64), ApiError> {
    let (mut token, revision) = load(s, owner, workspace, id).await?;
    if token.revoked || token.profile != profile(config)? {
        return Err(ApiError::bad(
            "OAuth2 token does not match this authorization profile",
        ));
    }
    if token
        .lease
        .as_ref()
        .is_some_and(|l| l.expires_at > chrono::Utc::now().timestamp())
    {
        return Err(ApiError {
            status: axum::http::StatusCode::CONFLICT,
            message: "OAuth2 token refresh is already running; retry shortly".into(),
        });
    }
    token.lease = Some(Lease {
        id: uuid::Uuid::new_v4().to_string(),
        expires_at: chrono::Utc::now().timestamp() + 60,
    });
    if !replace(s, owner, &token, revision).await? {
        return Err(ApiError {
            status: axum::http::StatusCode::CONFLICT,
            message: "OAuth2 token changed; retry the operation".into(),
        });
    }
    Ok((token, revision + 1))
}
pub(crate) async fn finish(
    s: &AppState,
    owner: &str,
    mut token: Token,
    revision: i64,
    response: BasicTokenResponse,
) -> Result<Token, ApiError> {
    validate(&response)?;
    let mut response = response;
    if response.refresh_token().is_none() {
        response.set_refresh_token(token.response.refresh_token().cloned());
    }
    token.expires_at = expiry(&response)?;
    token.response = response;
    token.lease = None;
    if !replace(s, owner, &token, revision).await? {
        return Err(ApiError::bad(
            "OAuth2 token changed before refresh completed",
        ));
    }
    Ok(token)
}
pub(crate) async fn release(
    s: &AppState,
    owner: &str,
    mut token: Token,
    revision: i64,
) -> Result<(), ApiError> {
    token.lease = None;
    replace(s, owner, &token, revision).await?;
    Ok(())
}
pub(crate) async fn remove(
    s: &AppState,
    owner: &str,
    workspace: &str,
    id: &str,
) -> Result<(), ApiError> {
    document::Entity::delete_many()
        .filter(document::Column::Id.eq(key(owner, workspace, id)))
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq("oauth2-token"))
        .filter(document::Column::RefId.eq(workspace))
        .exec(&s.db)
        .await?;
    Ok(())
}
