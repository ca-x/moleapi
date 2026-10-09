use super::models::{Credentials, Delivery, Target};
use crate::{ApiError, entities::document, storage};
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    sea_query::Expr,
};
pub(super) const TARGET: &str = "notify-target";
pub(super) const SECRET: &str = "notify-secret";
pub(super) const JOB: &str = "notify-job";
pub(super) fn secret_id(id: &str) -> String {
    format!("notify-secret-{id}")
}
pub(super) async fn target<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    workspace: &str,
    id: &str,
) -> Result<(Target, Credentials), ApiError> {
    let row = document::Entity::find_by_id(id)
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(TARGET))
        .filter(document::Column::RefId.eq(workspace))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)?;
    let secret = document::Entity::find_by_id(secret_id(id))
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(SECRET))
        .filter(document::Column::RefId.eq(workspace))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)?;
    Ok((
        serde_json::from_str(&row.payload).map_err(|_| ApiError::internal())?,
        serde_json::from_str(&secret.payload).map_err(|_| ApiError::internal())?,
    ))
}
pub(super) async fn lock<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    workspace: &str,
    id: &str,
    kind: &str,
) -> Result<document::Model, ApiError> {
    document::Entity::update_many()
        .col_expr(
            document::Column::Revision,
            Expr::col(document::Column::Revision).into(),
        )
        .filter(document::Column::Id.eq(id))
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(kind))
        .filter(document::Column::RefId.eq(workspace))
        .exec(db)
        .await?;
    document::Entity::find_by_id(id)
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(kind))
        .filter(document::Column::RefId.eq(workspace))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}
pub(super) async fn save<C: ConnectionTrait, T: serde::Serialize>(
    db: &C,
    owner: &str,
    id: &str,
    kind: &str,
    revision: i64,
    value: &T,
) -> Result<(), ApiError> {
    document::Entity::update_many()
        .col_expr(
            document::Column::Payload,
            Expr::value(serde_json::to_string(value).map_err(|_| ApiError::internal())?),
        )
        .col_expr(document::Column::Revision, Expr::value(revision))
        .col_expr(document::Column::UpdatedAt, Expr::value(storage::now()))
        .filter(document::Column::Id.eq(id))
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(kind))
        .exec(db)
        .await?;
    Ok(())
}
pub(super) async fn retain<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    workspace: &str,
) -> Result<(), ApiError> {
    let rows = document::Entity::find()
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(JOB))
        .filter(document::Column::RefId.eq(workspace))
        .filter(document::Column::Revision.eq(i64::MAX))
        .order_by_desc(document::Column::UpdatedAt)
        .offset(200)
        .limit(1000)
        .all(db)
        .await?;
    if !rows.is_empty() {
        document::Entity::delete_many()
            .filter(document::Column::Owner.eq(owner))
            .filter(document::Column::Kind.eq(JOB))
            .filter(document::Column::Id.is_in(rows.into_iter().map(|row| row.id)))
            .exec(db)
            .await?;
    }
    Ok(())
}
pub(super) async fn save_delivery<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    delivery: &Delivery,
) -> Result<(), ApiError> {
    save(db, owner, &delivery.id, JOB, delivery.wake_at(), delivery).await
}
