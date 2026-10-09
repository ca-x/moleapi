use super::models::{Occurrence, Schedule, ledger_id};
use crate::{ApiError, entities::document, storage};
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    sea_query::Expr,
};
pub(super) const KIND: &str = "schedule";
pub(super) const RUN: &str = "sched-run";
pub(super) async fn workspace_lock<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    workspace: &str,
) -> Result<moleapi_core::Workspace, ApiError> {
    document::Entity::update_many()
        .col_expr(
            document::Column::Revision,
            Expr::col(document::Column::Revision).into(),
        )
        .filter(document::Column::Id.eq(storage::workspace_key(owner, workspace)))
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq("workspace"))
        .exec(db)
        .await?;
    storage::get(db, owner, workspace)
        .await?
        .ok_or_else(ApiError::not_found)
}
pub(super) async fn locked<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    workspace: &str,
    id: &str,
) -> Result<Schedule, ApiError> {
    document::Entity::update_many()
        .col_expr(
            document::Column::Revision,
            Expr::col(document::Column::Revision).into(),
        )
        .filter(document::Column::Id.eq(id))
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(KIND))
        .filter(document::Column::RefId.eq(workspace))
        .exec(db)
        .await?;
    let row = document::Entity::find_by_id(id)
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(KIND))
        .filter(document::Column::RefId.eq(workspace))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)?;
    serde_json::from_str(&row.payload).map_err(|_| ApiError::internal())
}
pub(super) async fn save<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    schedule: &Schedule,
) -> Result<(), ApiError> {
    document::Entity::update_many()
        .col_expr(
            document::Column::Payload,
            Expr::value(serde_json::to_string(schedule).map_err(|_| ApiError::internal())?),
        )
        .col_expr(document::Column::Revision, Expr::value(schedule.wake_at()))
        .col_expr(document::Column::UpdatedAt, Expr::value(storage::now()))
        .filter(document::Column::Id.eq(&schedule.id))
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(KIND))
        .filter(document::Column::RefId.eq(&schedule.workspace_id))
        .exec(db)
        .await?;
    Ok(())
}
pub(super) async fn save_run<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    workspace: &str,
    run: &Occurrence,
) -> Result<(), ApiError> {
    let id = ledger_id(&run.schedule_id, &run.id);
    if document::Entity::find_by_id(&id).one(db).await?.is_some() {
        document::Entity::update_many()
            .col_expr(
                document::Column::Payload,
                Expr::value(serde_json::to_string(run).map_err(|_| ApiError::internal())?),
            )
            .col_expr(document::Column::UpdatedAt, Expr::value(storage::now()))
            .filter(document::Column::Id.eq(id))
            .filter(document::Column::Owner.eq(owner))
            .filter(document::Column::Kind.eq(RUN))
            .exec(db)
            .await?;
    } else {
        storage::insert_doc(db, id, owner, RUN, workspace, 0, run).await?;
    }
    let old = document::Entity::find()
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(RUN))
        .filter(document::Column::RefId.eq(workspace))
        .filter(document::Column::Id.starts_with(format!("sched-run-{}-", run.schedule_id)))
        .order_by_desc(document::Column::UpdatedAt)
        .offset(100)
        .limit(100)
        .all(db)
        .await?;
    if !old.is_empty() {
        document::Entity::delete_many()
            .filter(document::Column::Owner.eq(owner))
            .filter(document::Column::Kind.eq(RUN))
            .filter(document::Column::Id.is_in(old.into_iter().map(|row| row.id)))
            .exec(db)
            .await?;
    }
    Ok(())
}
