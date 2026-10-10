use super::storage::{RUNNER, TASK, Task, account_lock, get, save, save_task};
use super::{hosted, selection::validate, terminal};
use crate::{
    ApiError, AppState, auth::Identity, entities::document, scheduling::workspace_lock,
    storage as store,
};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use chrono::Utc;
use moleapi_core::{ExecutionRunner, RunnerClaim, RunnerTaskStatus as Status, RunnerTaskSummary};
use rand::RngCore;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, TransactionTrait};
use serde::Deserialize;
use serde_json::{Value, json};
fn expired(task: &Task) -> bool {
    task.summary
        .lease_until
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .is_none_or(|d| d <= Utc::now())
}
fn lease_until() -> String {
    (Utc::now() + chrono::Duration::seconds(90)).to_rfc3339()
}
pub(crate) async fn claim(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
) -> Result<Json<Option<RunnerClaim>>, ApiError> {
    hosted(&state)?;
    let tx = state.db.begin().await?;
    account_lock(&tx, &owner.0).await?;
    let mut runner: ExecutionRunner = get(&tx, &owner.0, RUNNER, &id).await?;
    if !runner.enabled {
        return Err(ApiError::bad("Runner is disabled"));
    }
    runner.last_seen_at = Some(store::now());
    let epoch = crate::credential_revocations::current(&tx, &owner.0).await?;
    if let Some(active) = &runner.active_task_id {
        match get::<_, Task>(&tx, &owner.0, TASK, active).await {
            Ok(task)
                if task.summary.status == Status::Leased
                    && task.credential_revision == epoch
                    && !expired(&task) =>
            {
                save(&tx, &owner.0, RUNNER, &id, runner.revision, &runner).await?;
                tx.commit().await?;
                return Ok(Json(None));
            }
            Ok(mut task)
                if task.summary.status == Status::Leased && task.credential_revision != epoch =>
            {
                terminal(&mut task, Status::Cancelled);
                save_task(&tx, &owner.0, &task).await?;
            }
            Ok(_) => {}
            Err(error) if error.status == axum::http::StatusCode::NOT_FOUND => {}
            Err(error) => return Err(error),
        }
        runner.active_task_id = None;
    }
    let rows = document::Entity::find()
        .filter(document::Column::Owner.eq(&owner.0))
        .filter(document::Column::Kind.eq(TASK))
        .filter(document::Column::Revision.lte(Utc::now().timestamp_millis()))
        .order_by_asc(document::Column::UpdatedAt)
        .order_by_asc(document::Column::Id)
        .all(&tx)
        .await?;

    for row in rows {
        let mut task: Task =
            serde_json::from_str(&row.payload).map_err(|_| ApiError::internal())?;
        if task.summary.runner_id != id {
            continue;
        }
        if task.credential_revision != epoch {
            terminal(&mut task, Status::Cancelled);
            save_task(&tx, &owner.0, &task).await?;
            continue;
        }
        if task.summary.attempt >= task.summary.max_attempts {
            terminal(&mut task, Status::Failed);
            save_task(&tx, &owner.0, &task).await?;
            continue;
        }
        let workspace = match workspace_lock(&tx, &owner.0, &task.summary.workspace_id).await {
            Ok(w) if w.revision == task.summary.source_revision => w,
            Ok(_) => {
                terminal(&mut task, Status::Cancelled);
                save_task(&tx, &owner.0, &task).await?;
                continue;
            }
            Err(error) if error.status == axum::http::StatusCode::NOT_FOUND => {
                terminal(&mut task, Status::Cancelled);
                save_task(&tx, &owner.0, &task).await?;
                continue;
            }
            Err(error) => return Err(error),
        };
        validate(&workspace, &task.summary.selection)?;
        if serde_json::to_vec(&workspace)
            .map_err(|_| ApiError::internal())?
            .len()
            > 16 * 1024 * 1024
        {
            return Err(ApiError::bad("Runner snapshot exceeds 16 MiB"));
        }
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        let token = hex::encode(bytes);
        task.lease_digest = Some(crate::auth::token_hash(&token));
        task.snapshot = Some(workspace.clone());
        task.summary.status = Status::Leased;
        task.summary.attempt += 1;
        task.summary.started_at = Some(store::now());
        task.summary.lease_until = Some(lease_until());
        runner.active_task_id = Some(task.summary.id.clone());
        save_task(&tx, &owner.0, &task).await?;
        save(&tx, &owner.0, RUNNER, &id, runner.revision, &runner).await?;
        tx.commit().await?;
        return Ok(Json(Some(RunnerClaim {
            task: task.summary,
            lease_token: token,
            workspace,
        })));
    }
    save(&tx, &owner.0, RUNNER, &id, runner.revision, &runner).await?;
    tx.commit().await?;
    Ok(Json(None))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Lease {
    lease_token: String,
}
fn check_token(task: &Task, runner: &str, token: &str) -> Result<(), ApiError> {
    if token.len() != 64
        || task.summary.runner_id != runner
        || task.lease_digest.as_deref() != Some(crate::auth::token_hash(token).as_str())
    {
        return Err(ApiError::conflict("Runner lease is invalid"));
    }
    Ok(())
}
async fn live<C: sea_orm::ConnectionTrait>(
    db: &C,
    owner: &str,
    runner: &ExecutionRunner,
    task: &Task,
) -> Result<(), ApiError> {
    if !runner.enabled
        || task.summary.status != Status::Leased
        || expired(task)
        || task.credential_revision != crate::credential_revocations::current(db, owner).await?
        || store::get(db, owner, &task.summary.workspace_id)
            .await?
            .is_none()
    {
        return Err(ApiError::conflict("Runner task is no longer active"));
    }
    Ok(())
}
pub(crate) async fn heartbeat(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path((runner_id, id)): Path<(String, String)>,
    Json(input): Json<Lease>,
) -> Result<Json<Value>, ApiError> {
    hosted(&state)?;
    let tx = state.db.begin().await?;
    account_lock(&tx, &owner.0).await?;
    let mut runner: ExecutionRunner = get(&tx, &owner.0, RUNNER, &runner_id).await?;
    let mut task: Task = get(&tx, &owner.0, TASK, &id).await?;
    check_token(&task, &runner_id, &input.lease_token)?;
    live(&tx, &owner.0, &runner, &task).await?;
    task.summary.lease_until = Some(lease_until());
    runner.last_seen_at = Some(store::now());
    save_task(&tx, &owner.0, &task).await?;
    save(&tx, &owner.0, RUNNER, &runner_id, runner.revision, &runner).await?;
    tx.commit().await?;
    Ok(Json(json!({"lease_until":task.summary.lease_until})))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Completion {
    lease_token: String,
    report: Option<Value>,
    status: Option<String>,
}
pub(crate) async fn complete(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path((runner_id, id)): Path<(String, String)>,
    Json(mut input): Json<Completion>,
) -> Result<Json<RunnerTaskSummary>, ApiError> {
    hosted(&state)?;
    let tx = state.db.begin().await?;
    account_lock(&tx, &owner.0).await?;
    let mut runner: ExecutionRunner = get(&tx, &owner.0, RUNNER, &runner_id).await?;
    let mut task: Task = get(&tx, &owner.0, TASK, &id).await?;
    check_token(&task, &runner_id, &input.lease_token)?;
    if matches!(
        task.summary.status,
        Status::Completed | Status::Failed | Status::Cancelled
    ) {
        return Ok(Json(task.summary));
    }
    live(&tx, &owner.0, &runner, &task).await?;
    if let Some(report) = input.report.as_mut() {
        if input.status.is_some()
            || serde_json::to_vec(report)
                .map_err(|_| ApiError::internal())?
                .len()
                > 8 * 1024 * 1024
        {
            return Err(ApiError::bad(
                "Completion requires one bounded report or terminal status",
            ));
        }
        let workspace = task.snapshot.as_ref().ok_or_else(ApiError::internal)?;
        let selection = &task.summary.selection;
        let collection = workspace
            .data
            .collections
            .iter()
            .find(|c| c.id == selection.collection_id)
            .ok_or_else(ApiError::internal)?;
        let scenario = selection
            .scenario_id
            .as_ref()
            .and_then(|id| workspace.data.scenarios.iter().find(|s| s.id == *id));
        let environment =
            crate::execution::environment(workspace, selection.environment_id.as_deref())?;
        let scopes = crate::execution::variables(
            &state,
            workspace,
            Some(collection),
            environment,
            &[],
            &[],
            &[],
        )?;
        let results = report["results"]
            .as_array_mut()
            .ok_or_else(|| ApiError::bad("Report results must be an array"))?;
        if results.len() > 1000 {
            return Err(ApiError::bad("Report exceeds 1000 results"));
        }
        let allowed = if let Some(scenario) = scenario {
            moleapi_core::scenario_plan(&workspace.data, scenario)
                .map_err(|e| ApiError::bad(e.to_string()))?
                .into_iter()
                .map(|(_, request, _)| request.id.as_str())
                .collect::<std::collections::BTreeSet<_>>()
        } else {
            moleapi_core::collection_subtree(&workspace.data, collection)
                .map_err(|e| ApiError::bad(e.to_string()))?
                .into_iter()
                .flat_map(|c| c.requests.iter())
                .filter(|r| {
                    selection
                        .request_ids
                        .as_ref()
                        .is_none_or(|ids| ids.contains(&r.id))
                })
                .map(|r| r.id.as_str())
                .collect::<std::collections::BTreeSet<_>>()
        };
        for item in results {
            if let Some(request) = item["request_id"].as_str()
                && !allowed.contains(request)
            {
                return Err(ApiError::bad(
                    "Report references a request outside the selection",
                ));
            }
            if let Some(tests) = item["response"]["tests"].as_array() {
                for test in tests {
                    serde_json::from_value::<moleapi_core::TestResult>(test.clone())
                        .map_err(|_| ApiError::bad("Report contains an invalid test result"))?;
                }
            }
            item.as_object_mut()
                .ok_or_else(|| ApiError::bad("Report result must be an object"))?
                .remove("history_id");
        }
        let source = crate::run_reports::Source {
            workspace,
            collection,
            scenario,
            environment,
            dataset: selection.dataset_id.as_deref(),
            started_at: task
                .summary
                .started_at
                .as_deref()
                .ok_or_else(ApiError::internal)?,
            scopes: &scopes,
            notification_ids: &selection.notification_ids,
            run_origin: "ci",
        };
        task.summary.report_id =
            crate::run_reports::record_in(&tx, &owner.0, &source, report).await?;
        if task.summary.report_id.is_none() {
            return Err(ApiError::conflict("Runner source lineage changed"));
        }
        terminal(&mut task, Status::Completed);
    } else {
        match input.status.as_deref() {
            Some("cancelled") => terminal(&mut task, Status::Cancelled),
            Some("failed") => terminal(&mut task, Status::Failed),
            _ => {
                return Err(ApiError::bad(
                    "Completion requires a report or failed/cancelled status",
                ));
            }
        }
    }
    runner.active_task_id = None;
    runner.last_seen_at = Some(store::now());
    save_task(&tx, &owner.0, &task).await?;
    save(&tx, &owner.0, RUNNER, &runner_id, runner.revision, &runner).await?;
    tx.commit().await?;
    Ok(Json(task.summary))
}
