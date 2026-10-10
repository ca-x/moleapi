use super::storage::{RUNNER, TASK, Task, account_lock, get, prune, save_task};
use super::{hosted, selection::validate, terminal};
use crate::{
    ApiError, AppState, auth::Identity, entities::document, scheduling::workspace_lock,
    storage as store, workspaces::owned,
};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use moleapi_core::{
    ExecutionRunner, RunnerSelection, RunnerTaskStatus as Status, RunnerTaskSummary,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, TransactionTrait};
use serde::Deserialize;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Queue {
    runner_id: String,
    expected_revision: i64,
    selection: RunnerSelection,
    #[serde(default = "one")]
    max_attempts: usize,
}
fn one() -> usize {
    1
}
pub(crate) async fn queue(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(workspace): Path<String>,
    Json(input): Json<Queue>,
) -> Result<Json<RunnerTaskSummary>, ApiError> {
    hosted(&state)?;
    if !(1..=3).contains(&input.max_attempts) {
        return Err(ApiError::bad("Task allows 1 to 3 attempts"));
    }
    let tx = state.db.begin().await?;
    account_lock(&tx, &owner.0).await?;
    let source = workspace_lock(&tx, &owner.0, &workspace).await?;
    if source.revision != input.expected_revision {
        return Err(ApiError::conflict("Workspace revision changed"));
    }
    validate(&source, &input.selection)?;
    crate::notifications::validate_targets(
        &tx,
        &owner.0,
        &workspace,
        &input.selection.notification_ids,
    )
    .await?;
    let runner: ExecutionRunner = get(&tx, &owner.0, RUNNER, &input.runner_id).await?;
    if !runner.enabled {
        return Err(ApiError::bad("Runner is disabled"));
    }
    let pending = document::Entity::find()
        .filter(document::Column::Owner.eq(&owner.0))
        .filter(document::Column::Kind.eq(TASK))
        .filter(document::Column::RefId.eq(&workspace))
        .filter(document::Column::Revision.lt(i64::MAX))
        .all(&tx)
        .await?;
    if pending.len() >= 100 {
        return Err(ApiError::bad("Workspace exceeds 100 pending runner tasks"));
    }
    let task = Task {
        summary: RunnerTaskSummary {
            id: uuid::Uuid::new_v4().to_string(),
            workspace_id: workspace.clone(),
            runner_id: input.runner_id,
            selection: input.selection,
            source_revision: source.revision,
            status: Status::Queued,
            attempt: 0,
            max_attempts: input.max_attempts,
            queued_at: store::now(),
            started_at: None,
            lease_until: None,
            finished_at: None,
            report_id: None,
        },
        credential_revision: crate::credential_revocations::current(&tx, &owner.0).await?,
        lease_digest: None,
        snapshot: None,
    };
    store::insert_doc(
        &tx,
        task.summary.id.clone(),
        &owner.0,
        TASK,
        &workspace,
        0,
        &task,
    )
    .await?;
    prune(&tx, &owner.0, &workspace).await?;
    tx.commit().await?;
    Ok(Json(task.summary))
}
pub(crate) async fn tasks(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(workspace): Path<String>,
) -> Result<Json<Vec<RunnerTaskSummary>>, ApiError> {
    hosted(&state)?;
    owned(&state, &owner.0, &workspace).await?;
    let rows: Vec<Task> =
        store::documents(&state.db, &owner.0, TASK, Some(&workspace), 200).await?;
    Ok(Json(rows.into_iter().map(|t| t.summary).collect()))
}
pub(crate) async fn cancel(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path((workspace, id)): Path<(String, String)>,
) -> Result<Json<RunnerTaskSummary>, ApiError> {
    hosted(&state)?;
    let tx = state.db.begin().await?;
    account_lock(&tx, &owner.0).await?;
    workspace_lock(&tx, &owner.0, &workspace).await?;
    let mut task: Task = get(&tx, &owner.0, TASK, &id).await?;
    if task.summary.workspace_id != workspace {
        return Err(ApiError::not_found());
    }
    if matches!(task.summary.status, Status::Queued | Status::Leased) {
        terminal(&mut task, Status::Cancelled);
        save_task(&tx, &owner.0, &task).await?;
    }
    tx.commit().await?;
    Ok(Json(task.summary))
}
