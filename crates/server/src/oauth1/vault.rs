//! Private owner/workspace token documents. Workspace lock + capacity check are atomic.
use super::configuration::Profile;
use crate::{ApiError, AppState, entities::document, storage};
use moleapi_core::OAuth1Credentials;
use sea_orm::{
    ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, TransactionTrait, sea_query::Expr,
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Token {
    pub id: String,
    pub workspace_id: String,
    pub label: String,
    pub profile: String,
    pub profile_version: u32,
    pub issuer: String,
    pub consumer_key: String,
    pub credentials: OAuth1Credentials,
    pub created_at: i64,
}
#[derive(Serialize)]
pub(crate) struct Metadata {
    pub id: String,
    pub label: String,
    pub issuer: String,
    pub consumer_key: String,
    pub created_at: i64,
    pub profile_version: u32,
}
impl Token {
    pub fn metadata(&self) -> Metadata {
        Metadata {
            id: self.id.clone(),
            label: self.label.clone(),
            issuer: self.issuer.clone(),
            consumer_key: self.consumer_key.clone(),
            created_at: self.created_at,
            profile_version: self.profile_version,
        }
    }
}
pub(super) fn key(owner: &str, workspace: &str, id: &str) -> String {
    storage::workspace_key(owner, &format!("oauth1:{workspace}:{id}"))
}
pub(super) async fn create(
    s: &AppState,
    owner: &str,
    workspace: &str,
    label: String,
    profile: &Profile,
    credentials: OAuth1Credentials,
) -> Result<Token, ApiError> {
    credentials
        .validate()
        .map_err(|e| ApiError::bad(e.to_string()))?;
    let token = Token {
        id: uuid::Uuid::new_v4().to_string(),
        workspace_id: workspace.into(),
        label,
        profile: profile.hash()?,
        profile_version: 1,
        issuer: profile.issuer(),
        consumer_key: profile.consumer_key.clone(),
        credentials,
        created_at: chrono::Utc::now().timestamp(),
    };
    let tx = s.db.begin().await?;
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
    if document::Entity::find()
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq("oauth1-token"))
        .filter(document::Column::RefId.eq(workspace))
        .count(&tx)
        .await?
        >= 128
    {
        return Err(ApiError::bad("OAuth1 token capacity reached for workspace"));
    }
    storage::insert_doc(
        &tx,
        key(owner, workspace, &token.id),
        owner,
        "oauth1-token",
        workspace,
        0,
        &token,
    )
    .await?;
    tx.commit().await?;
    Ok(token)
}
pub(super) async fn load(
    s: &AppState,
    owner: &str,
    workspace: &str,
    id: &str,
) -> Result<(Token, i64), ApiError> {
    let row = document::Entity::find_by_id(key(owner, workspace, id))
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq("oauth1-token"))
        .filter(document::Column::RefId.eq(workspace))
        .one(&s.db)
        .await?
        .ok_or_else(ApiError::not_found)?;
    let token: Token = serde_json::from_str(&row.payload).map_err(|_| ApiError::internal())?;
    if token.id != id || token.workspace_id != workspace || token.profile_version != 1 {
        return Err(ApiError::bad(
            "OAuth1 token profile needs migration; import or authorize again",
        ));
    }
    token
        .credentials
        .validate()
        .map_err(|_| ApiError::internal())?;
    Ok((token, row.revision))
}
pub(super) async fn rename(
    s: &AppState,
    owner: &str,
    mut token: Token,
    revision: i64,
    label: String,
) -> Result<Metadata, ApiError> {
    token.label = label;
    let result = document::Entity::update_many()
        .col_expr(
            document::Column::Payload,
            Expr::value(serde_json::to_string(&token).map_err(|_| ApiError::internal())?),
        )
        .col_expr(
            document::Column::Revision,
            Expr::value(revision.checked_add(1).ok_or_else(ApiError::internal)?),
        )
        .col_expr(document::Column::UpdatedAt, Expr::value(storage::now()))
        .filter(document::Column::Id.eq(key(owner, &token.workspace_id, &token.id)))
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq("oauth1-token"))
        .filter(document::Column::RefId.eq(&token.workspace_id))
        .filter(document::Column::Revision.eq(revision))
        .exec(&s.db)
        .await?;
    if result.rows_affected != 1 {
        return Err(ApiError::conflict(
            "OAuth1 token changed; reload before renaming",
        ));
    }
    Ok(token.metadata())
}
