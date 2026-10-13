use super::storage::{RUNNER, TASK, Task, get, save_task};
use crate::{ApiError, AppState};
use moleapi_core::{ExecutionRunner, RunnerTaskStatus as Status};
use sea_orm::ConnectionTrait;

pub(crate) async fn account_lock<C: ConnectionTrait>(
    db: &C,
    state: &AppState,
    owner: &str,
) -> Result<(), ApiError> {
    if !state.local {
        super::storage::account_lock(db, owner).await?;
    }
    Ok(())
}
pub(crate) async fn cancel_in<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    id: &str,
) -> Result<(), ApiError> {
    let mut task: Task = match get(db, owner, TASK, id).await {
        Ok(task) => task,
        Err(error) if error.status == axum::http::StatusCode::NOT_FOUND => return Ok(()),
        Err(error) => return Err(error),
    };
    if matches!(task.summary.status, Status::Queued | Status::Leased) {
        super::terminal(&mut task, Status::Cancelled);
        save_task(db, owner, &task).await?;
    }
    Ok(())
}
pub(crate) async fn reconcile<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    workspace: &str,
    id: &str,
) -> Result<Option<crate::scheduling::Occurrence>, ApiError> {
    let mut task: Task = match get(db, owner, TASK, id).await {
        Ok(task) => task,
        Err(error) if error.status == axum::http::StatusCode::NOT_FOUND => {
            return Ok(Some(crate::scheduling::Occurrence {
                notification_queued: 0,
                notification_skipped: 0,
                id: id.into(),
                schedule_id: String::new(),
                config_revision: 0,
                manual: false,
                status: "failed".into(),
                started_at: crate::storage::now(),
                finished_at: Some(crate::storage::now()),
                report_id: None,
                passed: 0,
                failed: 0,
                skipped: 0,
                error: Some("Runner task metadata is no longer available".into()),
            }));
        }
        Err(error) => return Err(error),
    };
    if task.summary.workspace_id != workspace {
        return Err(ApiError::not_found());
    }
    if matches!(task.summary.status, Status::Queued | Status::Leased) {
        let runner: ExecutionRunner = get(db, owner, RUNNER, &task.summary.runner_id).await?;
        let expired = task.summary.status == Status::Leased
            && task
                .summary
                .lease_until
                .as_deref()
                .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
                .is_none_or(|value| value <= chrono::Utc::now());
        let current = crate::storage::get(db, owner, workspace).await?;
        if !runner.enabled
            || current.is_none_or(|value| value.revision != task.summary.source_revision)
            || task.credential_revision != crate::credential_revocations::current(db, owner).await?
        {
            super::terminal(&mut task, Status::Cancelled);
        } else if expired {
            super::terminal(&mut task, Status::Failed);
        } else {
            return Ok(None);
        }
        save_task(db, owner, &task).await?;
    }
    let mut record = crate::scheduling::Occurrence {
        notification_queued: 0,
        notification_skipped: 0,
        id: id.into(),
        schedule_id: String::new(),
        config_revision: 0,
        manual: false,
        status: match task.summary.status {
            Status::Completed => "passed",
            Status::Cancelled => "cancelled",
            _ => "failed",
        }
        .into(),
        started_at: task.summary.started_at.unwrap_or(task.summary.queued_at),
        finished_at: task.summary.finished_at,
        report_id: task.summary.report_id.clone(),
        passed: 0,
        failed: 0,
        skipped: 0,
        error: None,
    };
    if let Some(report_id) = task.summary.report_id {
        let report: moleapi_core::SavedRunReport =
            match get(db, owner, "run-report", &report_id).await {
                Ok(report) => report,
                Err(error) if error.status == axum::http::StatusCode::NOT_FOUND => {
                    record.status = "failed".into();
                    record.report_id = None;
                    record.error = Some("Runner report is no longer available".into());
                    return Ok(Some(record));
                }
                Err(error) => return Err(error),
            };
        record.passed = report.summary.passed;
        record.failed = report.summary.failed;
        record.skipped = report.summary.skipped;
        if report.summary.cancelled {
            record.status = "cancelled".into();
        } else if record.failed > 0 || report.summary.stopped_reason.is_some() {
            record.status = "failed".into();
        }
    }
    Ok(Some(record))
}
