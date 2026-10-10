use crate::{
    ApiError,
    entities::{account, document},
    storage,
};
use moleapi_core::{RunnerTaskSummary, Workspace};
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    sea_query::Expr,
};
use serde::{Deserialize, Serialize};
pub(super) const RUNNER: &str = "execution-runner";
pub(super) const TASK: &str = "runner-task";
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Task {
    pub summary: RunnerTaskSummary,
    pub credential_revision: i64,
    pub lease_digest: Option<String>,
    pub snapshot: Option<Workspace>,
}
pub(super) async fn account_lock<C: ConnectionTrait>(db: &C, owner: &str) -> Result<(), ApiError> {
    account::Entity::update_many()
        .col_expr(
            account::Column::Username,
            Expr::col(account::Column::Username).into(),
        )
        .filter(account::Column::Id.eq(owner))
        .exec(db)
        .await?;
    if account::Entity::find_by_id(owner).one(db).await?.is_none() {
        return Err(ApiError::unauthorized());
    }
    Ok(())
}
pub(super) async fn get<C: ConnectionTrait, T: serde::de::DeserializeOwned>(
    db: &C,
    owner: &str,
    kind: &str,
    id: &str,
) -> Result<T, ApiError> {
    let row = document::Entity::find_by_id(id)
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(kind))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)?;
    serde_json::from_str(&row.payload).map_err(|_| ApiError::internal())
}
pub(super) async fn save<C: ConnectionTrait, T: Serialize>(
    db: &C,
    owner: &str,
    kind: &str,
    id: &str,
    wake: i64,
    value: &T,
) -> Result<(), ApiError> {
    document::Entity::update_many()
        .col_expr(
            document::Column::Payload,
            Expr::value(serde_json::to_string(value).map_err(|_| ApiError::internal())?),
        )
        .col_expr(document::Column::Revision, Expr::value(wake))
        .col_expr(document::Column::UpdatedAt, Expr::value(storage::now()))
        .filter(document::Column::Id.eq(id))
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(kind))
        .exec(db)
        .await?;
    Ok(())
}
pub(super) async fn save_task<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    task: &Task,
) -> Result<(), ApiError> {
    use moleapi_core::RunnerTaskStatus as Status;
    let wake = match task.summary.status {
        Status::Queued => 0,
        Status::Leased => task
            .summary
            .lease_until
            .as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map_or(0, |s| s.timestamp_millis()),
        _ => i64::MAX,
    };
    save(db, owner, TASK, &task.summary.id, wake, task).await?;
    if wake == i64::MAX {
        prune(db, owner, &task.summary.workspace_id).await?;
    }
    Ok(())
}

pub(super) async fn prune<C: sea_orm::ConnectionTrait>(
    db: &C,
    owner: &str,
    workspace: &str,
) -> Result<(), ApiError> {
    let old = document::Entity::find()
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(TASK))
        .filter(document::Column::RefId.eq(workspace))
        .filter(document::Column::Revision.eq(i64::MAX))
        .order_by_desc(document::Column::UpdatedAt)
        .order_by_desc(document::Column::Id)
        .offset(100)
        .limit(1000)
        .all(db)
        .await?;
    if !old.is_empty() {
        document::Entity::delete_many()
            .filter(document::Column::Owner.eq(owner))
            .filter(document::Column::Kind.eq(TASK))
            .filter(document::Column::Id.is_in(old.into_iter().map(|r| r.id)))
            .exec(db)
            .await?;
    }
    Ok(())
}
