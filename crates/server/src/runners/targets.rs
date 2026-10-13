use super::{
    hosted,
    registry::validate_labels,
    storage::{RUNNER, TASK, Task, get},
};
use crate::{ApiError, AppState, entities::document};
use moleapi_core::{ExecutionRunner, RunnerTaskStatus};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use std::collections::HashMap;

async fn candidates<C: ConnectionTrait>(
    db: &C,
    owner: &str,
) -> Result<Vec<ExecutionRunner>, ApiError> {
    document::Entity::find()
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(RUNNER))
        .all(db)
        .await?
        .into_iter()
        .map(|row| serde_json::from_str(&row.payload).map_err(|_| ApiError::internal()))
        .collect()
}

fn selectors<'a>(
    runner_id: Option<&'a str>,
    label: Option<&'a str>,
) -> Result<Option<&'a str>, ApiError> {
    match (runner_id, label) {
        (Some(_), Some(_)) => Err(ApiError::bad(
            "Choose a runner ID or a runner label, not both",
        )),
        (Some(""), None) => Err(ApiError::bad("Runner ID cannot be empty")),
        (None, Some(label)) => {
            validate_labels(&[label.to_owned()])?;
            Ok(Some(label))
        }
        _ => Ok(None),
    }
}

pub(crate) async fn validate<C: ConnectionTrait>(
    db: &C,
    state: &AppState,
    owner: &str,
    runner_id: Option<&str>,
    label: Option<&str>,
) -> Result<(), ApiError> {
    let selected_label = selectors(runner_id, label)?;
    if runner_id.is_none() && selected_label.is_none() {
        return Ok(());
    }
    hosted(state)?;
    if let Some(id) = runner_id {
        let runner: ExecutionRunner = get(db, owner, RUNNER, id).await?;
        if !runner.enabled {
            return Err(ApiError::bad("Runner is disabled"));
        }
    } else {
        let candidates = candidates(db, owner).await?;
        if !candidates.iter().any(|runner| {
            runner.enabled
                && runner
                    .labels
                    .iter()
                    .any(|value| Some(value.as_str()) == selected_label)
        }) {
            return Err(ApiError::bad("Runner label has no enabled members"));
        }
    }
    Ok(())
}

pub(crate) async fn resolve<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    runner_id: Option<&str>,
    label: Option<&str>,
) -> Result<ExecutionRunner, ApiError> {
    let selected_label = selectors(runner_id, label)?;
    if let Some(id) = runner_id {
        let runner: ExecutionRunner = get(db, owner, RUNNER, id).await?;
        if !runner.enabled {
            return Err(ApiError::bad("Runner is disabled"));
        }
        return Ok(runner);
    }
    let label =
        selected_label.ok_or_else(|| ApiError::bad("Choose a runner ID or runner label"))?;
    let mut candidates: Vec<_> = candidates(db, owner)
        .await?
        .into_iter()
        .filter(|runner| runner.enabled && runner.labels.iter().any(|value| value == label))
        .collect();
    if candidates.is_empty() {
        return Err(ApiError::bad("Runner label has no enabled members"));
    }
    let rows = document::Entity::find()
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(TASK))
        .filter(document::Column::Revision.lt(i64::MAX))
        .all(db)
        .await?;
    let mut pending: HashMap<String, usize> = HashMap::new();
    for row in rows {
        let task: Task = serde_json::from_str(&row.payload).map_err(|_| ApiError::internal())?;
        if matches!(
            task.summary.status,
            RunnerTaskStatus::Queued | RunnerTaskStatus::Leased
        ) {
            *pending.entry(task.summary.runner_id).or_default() += 1;
        }
    }
    candidates.sort_by(|left, right| {
        pending
            .get(&left.id)
            .copied()
            .unwrap_or(0)
            .cmp(&pending.get(&right.id).copied().unwrap_or(0))
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(candidates.remove(0))
}
