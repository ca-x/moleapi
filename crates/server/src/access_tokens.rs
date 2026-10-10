//! Account-owned automation credentials; plaintext is returned once and never persisted.
use crate::{
    ApiError, AppState,
    auth::{Identity, SessionAuthentication},
    entities::{access_token, account},
};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use rand::RngCore;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait, sea_query::Expr,
};
use serde::Deserialize;
use serde_json::{Value, json};
pub const PREFIX: &str = "moleapi_pat_";
fn management(
    state: &AppState,
    session: Option<Extension<SessionAuthentication>>,
) -> Result<(), ApiError> {
    if state.local {
        return Err(ApiError::not_found());
    }
    if session.is_none() {
        return Err(ApiError::forbidden(
            "Access token management requires a login session",
        ));
    }
    Ok(())
}
fn metadata(token: &access_token::Model) -> Value {
    json!({"id":token.id,"name":token.name,"created_at":token.created_at,"expires_at":token.expires_at,"expired":token.expires_at<=chrono::Utc::now().timestamp()})
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Create {
    name: String,
    #[serde(default = "default_days")]
    expires_in_days: u16,
}
fn default_days() -> u16 {
    90
}
pub async fn list(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    session: Option<Extension<SessionAuthentication>>,
) -> Result<Json<Value>, ApiError> {
    management(&state, session)?;
    let tokens = access_token::Entity::find()
        .filter(access_token::Column::Owner.eq(owner.0))
        .order_by_desc(access_token::Column::CreatedAt)
        .order_by_asc(access_token::Column::Id)
        .all(&state.db)
        .await?;
    Ok(Json(json!(tokens.iter().map(metadata).collect::<Vec<_>>())))
}
pub async fn create(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    session: Option<Extension<SessionAuthentication>>,
    Json(input): Json<Create>,
) -> Result<Json<Value>, ApiError> {
    management(&state, session)?;
    if input.name.trim() != input.name
        || !(1..=80).contains(&input.name.chars().count())
        || input.name.chars().any(char::is_control)
        || !(1..=365).contains(&input.expires_in_days)
    {
        return Err(ApiError::bad(
            "Token name must be 1–80 characters; expiry must be 1–365 days",
        ));
    }
    let tx = state.db.begin().await?;
    // A row update serializes account quota decisions on all three database engines.
    account::Entity::update_many()
        .col_expr(
            account::Column::Username,
            Expr::col(account::Column::Username).into(),
        )
        .filter(account::Column::Id.eq(&owner.0))
        .exec(&tx)
        .await?;
    if account::Entity::find_by_id(&owner.0)
        .one(&tx)
        .await?
        .is_none()
    {
        return Err(ApiError::unauthorized());
    }
    if access_token::Entity::find()
        .filter(access_token::Column::Owner.eq(&owner.0))
        .count(&tx)
        .await?
        >= 100
    {
        return Err(ApiError::bad(
            "Access token limit reached; revoke unused tokens",
        ));
    }
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let plaintext = format!("{PREFIX}{}", hex::encode(bytes));
    let now = chrono::Utc::now().timestamp();
    let token = access_token::ActiveModel {
        id: Set(uuid::Uuid::new_v4().to_string()),
        owner: Set(owner.0),
        name: Set(input.name),
        digest: Set(crate::auth::token_hash(&plaintext)),
        created_at: Set(now),
        expires_at: Set(now + i64::from(input.expires_in_days) * 86400),
    }
    .insert(&tx)
    .await?;
    tx.commit().await?;
    let mut result = metadata(&token);
    result["token"] = json!(plaintext);
    Ok(Json(result))
}
pub async fn revoke(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    session: Option<Extension<SessionAuthentication>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    management(&state, session)?;
    let deleted = access_token::Entity::delete_many()
        .filter(access_token::Column::Owner.eq(&owner.0))
        .filter(access_token::Column::Id.eq(id))
        .exec(&state.db)
        .await?;
    if deleted.rows_affected != 1 {
        return Err(ApiError::not_found());
    }
    crate::auth::invalidate_owner(&state, &owner.0).await?;
    Ok(Json(json!({"revoked":true})))
}
