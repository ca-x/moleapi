use super::models::{Capture, Inbox};
use crate::{ApiError, entities::document, storage};
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    sea_query::Expr,
};
pub const INBOX: &str = "webhook-inbox";
pub const CAPTURE: &str = "webhook-capture";
pub fn token_id(token: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("wh_{}", hex::encode(Sha256::digest(token.as_bytes())))
}
pub async fn inbox<C: ConnectionTrait>(
    db: &C,
    id: &str,
    owner: Option<&str>,
) -> Result<Option<(document::Model, Inbox)>, ApiError> {
    let mut query = document::Entity::find_by_id(id).filter(document::Column::Kind.eq(INBOX));
    if let Some(owner) = owner {
        query = query.filter(document::Column::Owner.eq(owner));
    }
    let Some(row) = query.one(db).await? else {
        return Ok(None);
    };
    let value: Inbox = serde_json::from_str(&row.payload).map_err(|_| ApiError::internal())?;
    if value.id != row.id || value.workspace_id != row.ref_id || value.revision != row.revision {
        return Err(ApiError::internal());
    }
    Ok(Some((row, value)))
}
pub async fn update<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    inbox: &Inbox,
    previous: i64,
) -> Result<(), ApiError> {
    let result = document::Entity::update_many()
        .col_expr(
            document::Column::Payload,
            Expr::value(serde_json::to_string(inbox).map_err(|_| ApiError::internal())?),
        )
        .col_expr(document::Column::Revision, Expr::value(inbox.revision))
        .col_expr(document::Column::UpdatedAt, Expr::value(&inbox.updated_at))
        .filter(document::Column::Id.eq(&inbox.id))
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(INBOX))
        .filter(document::Column::Revision.eq(previous))
        .exec(db)
        .await?;
    if result.rows_affected != 1 {
        return Err(ApiError::conflict("Webhook receiver revision changed"));
    }
    Ok(())
}
pub async fn captures<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    id: &str,
) -> Result<Vec<Capture>, ApiError> {
    document::Entity::find()
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(CAPTURE))
        .filter(document::Column::RefId.eq(id))
        .order_by_desc(document::Column::UpdatedAt)
        .limit(65)
        .all(db)
        .await?
        .into_iter()
        .map(|row| serde_json::from_str(&row.payload).map_err(|_| ApiError::internal()))
        .collect()
}
pub async fn delete_captures<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    inbox_ids: &[String],
) -> Result<(), ApiError> {
    if !inbox_ids.is_empty() {
        document::Entity::delete_many()
            .filter(document::Column::Owner.eq(owner))
            .filter(document::Column::Kind.eq(CAPTURE))
            .filter(document::Column::RefId.is_in(inbox_ids.iter().cloned()))
            .exec(db)
            .await?;
    }
    Ok(())
}
pub async fn inboxes_for_workspace<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    workspace_id: &str,
) -> Result<Vec<Inbox>, ApiError> {
    document::Entity::find()
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(INBOX))
        .filter(document::Column::RefId.eq(workspace_id))
        .all(db)
        .await?
        .into_iter()
        .map(|row| serde_json::from_str(&row.payload).map_err(|_| ApiError::internal()))
        .collect()
}
pub async fn ensure_workspace<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    id: &str,
) -> Result<(), ApiError> {
    if storage::get(db, owner, id).await?.is_none() {
        return Err(ApiError::not_found());
    }
    Ok(())
}
