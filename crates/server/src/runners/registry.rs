use super::storage::{RUNNER, TASK, Task, account_lock, get, save, save_task};
use super::{hosted, terminal};
use crate::{ApiError, AppState, auth::Identity, entities::document, storage as store};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use moleapi_core::{ExecutionRunner, RunnerTaskStatus as Status};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, TransactionTrait};
use serde::Deserialize;
fn name(value: &str) -> Result<(), ApiError> {
    if value.trim() != value
        || value.is_empty()
        || value.chars().count() > 80
        || value.chars().any(char::is_control)
    {
        return Err(ApiError::bad("Runner name must be 1 to 80 characters"));
    }
    Ok(())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Register {
    name: String,
}
pub(crate) async fn register(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Json(input): Json<Register>,
) -> Result<Json<ExecutionRunner>, ApiError> {
    hosted(&state)?;
    name(&input.name)?;
    let tx = state.db.begin().await?;
    account_lock(&tx, &owner.0).await?;
    let rows = document::Entity::find()
        .filter(document::Column::Owner.eq(&owner.0))
        .filter(document::Column::Kind.eq(RUNNER))
        .all(&tx)
        .await?;
    if rows.len() >= 100 {
        return Err(ApiError::bad("Account exceeds 100 registered runners"));
    }
    let runner = ExecutionRunner {
        id: uuid::Uuid::new_v4().to_string(),
        name: input.name,
        enabled: true,
        revision: 1,
        last_seen_at: None,
        active_task_id: None,
        created_at: store::now(),
    };
    store::insert_doc(
        &tx,
        runner.id.clone(),
        &owner.0,
        RUNNER,
        "",
        runner.revision,
        &runner,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(runner))
}
pub(crate) async fn list(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
) -> Result<Json<Vec<ExecutionRunner>>, ApiError> {
    hosted(&state)?;
    Ok(Json(
        store::documents(&state.db, &owner.0, RUNNER, None, 100).await?,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Update {
    expected_revision: i64,
    name: String,
    enabled: bool,
}
pub(crate) async fn update(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
    Json(input): Json<Update>,
) -> Result<Json<ExecutionRunner>, ApiError> {
    hosted(&state)?;
    name(&input.name)?;
    let tx = state.db.begin().await?;
    account_lock(&tx, &owner.0).await?;
    let mut runner: ExecutionRunner = get(&tx, &owner.0, RUNNER, &id).await?;
    if runner.revision != input.expected_revision {
        return Err(ApiError::conflict("Runner revision changed"));
    }
    runner.name = input.name;
    runner.enabled = input.enabled;
    runner.revision = runner
        .revision
        .checked_add(1)
        .ok_or_else(ApiError::internal)?;
    if !runner.enabled
        && let Some(task_id) = runner.active_task_id.take()
    {
        match get::<_, Task>(&tx, &owner.0, TASK, &task_id).await {
            Ok(mut task) => {
                terminal(&mut task, Status::Cancelled);
                save_task(&tx, &owner.0, &task).await?;
            }
            Err(error) if error.status == axum::http::StatusCode::NOT_FOUND => {}
            Err(error) => return Err(error),
        }
    }

    save(&tx, &owner.0, RUNNER, &id, runner.revision, &runner).await?;
    tx.commit().await?;
    Ok(Json(runner))
}
