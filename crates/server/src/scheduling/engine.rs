use super::{
    models::{Definition, Job, Occurrence},
    storage::{KIND, locked, save, save_run, workspace_lock},
};
use crate::{
    ApiError, AppState,
    auth::Identity,
    entities::{account, document},
};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use chrono::{DateTime, Utc};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, TransactionTrait};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
pub(crate) struct Lifetime {
    state: AppState,
    cancel: CancellationToken,
    slots: Arc<tokio::sync::Semaphore>,
}
impl Drop for Lifetime {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
pub(crate) fn start(state: AppState) -> Arc<Lifetime> {
    let life = Arc::new(Lifetime {
        state,
        cancel: CancellationToken::new(),
        slots: Arc::new(tokio::sync::Semaphore::new(2)),
    });
    let weak = Arc::downgrade(&life);
    let cancellation = life.cancel.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! {_=cancellation.cancelled()=>break,_=tokio::time::sleep(std::time::Duration::from_millis(400))=>{}}
            let Some((state, slots)) = weak
                .upgrade()
                .map(|life| (life.state.clone(), life.slots.clone()))
            else {
                break;
            };
            if let Err(error) = tick(&state, slots, &cancellation).await {
                eprintln!("schedule tick failed: {}", error.message);
            }
        }
    });
    life
}
struct Claim {
    schedule: String,
    owner: String,
    workspace: String,
    job: Job,
    definition: Definition,
}
fn occurrence(claim: &Claim) -> Occurrence {
    Occurrence {
        id: claim.job.id.clone(),
        schedule_id: claim.schedule.clone(),
        config_revision: claim.job.config_revision,
        manual: claim.job.manual,
        status: "running".into(),
        started_at: claim.job.started_at.clone(),
        finished_at: None,
        report_id: None,
        passed: 0,
        failed: 0,
        skipped: 0,
        error: None,
    }
}
async fn tick(
    state: &AppState,
    slots: Arc<tokio::sync::Semaphore>,
    cancel: &CancellationToken,
) -> Result<(), ApiError> {
    let rows = document::Entity::find()
        .filter(document::Column::Kind.eq(KIND))
        .filter(document::Column::Revision.lte(Utc::now().timestamp_millis()))
        .order_by_asc(document::Column::Revision)
        .limit(16)
        .all(&state.db)
        .await?;
    for row in rows {
        if state.project_slots.available_permits() == 0 {
            break;
        }
        let Ok(permit) = slots.clone().try_acquire_owned() else {
            break;
        };
        if let Some(claim) = claim(state, &row.owner, &row.ref_id, &row.id).await? {
            let state = state.clone();
            let cancel = cancel.clone();
            tokio::spawn(async move {
                let _permit = permit;
                if let Err(error) = execute(state, claim, cancel).await {
                    eprintln!("schedule execution failed: {}", error.message);
                }
            });
        }
    }
    Ok(())
}
async fn claim(
    state: &AppState,
    owner: &str,
    workspace: &str,
    id: &str,
) -> Result<Option<Claim>, ApiError> {
    let now = Utc::now();
    let tx = state.db.begin().await?;
    workspace_lock(&tx, owner, workspace).await?;
    let mut schedule = locked(&tx, owner, workspace, id).await?;
    if let Some(job) = &schedule.running {
        if DateTime::parse_from_rfc3339(&job.lease_until)
            .map(|time| time.with_timezone(&Utc) > now)
            .unwrap_or(false)
        {
            return Ok(None);
        }
        let mut interrupted = Occurrence {
            id: job.id.clone(),
            schedule_id: schedule.id.clone(),
            config_revision: job.config_revision,
            manual: job.manual,
            status: "interrupted".into(),
            started_at: job.started_at.clone(),
            finished_at: Some(now.to_rfc3339()),
            report_id: None,
            passed: 0,
            failed: 0,
            skipped: 0,
            error: Some("Execution lease expired; prior network effects were not retried".into()),
        };
        if job.cancel_requested {
            interrupted.status = "cancelled".into();
            interrupted.error = None;
        }
        save_run(&tx, owner, workspace, &interrupted).await?;
        schedule.last_run = Some(interrupted);
        schedule.running = None;
    }
    let queued = schedule.queued.take();
    let due = schedule.definition.enabled
        && schedule
            .next_run_at
            .as_ref()
            .and_then(|time| DateTime::parse_from_rfc3339(time).ok())
            .is_some_and(|time| time.with_timezone(&Utc) <= now);
    if queued.is_none() && !due {
        save(&tx, owner, &schedule).await?;
        tx.commit().await?;
        return Ok(None);
    }
    if !state.local && account::Entity::find_by_id(owner).one(&tx).await?.is_none() {
        schedule.definition.enabled = false;
        schedule.queued = None;
        save(&tx, owner, &schedule).await?;
        tx.commit().await?;
        return Ok(None);
    }
    let manual = queued.is_some();
    let job = Job {
        cancel_requested: false,
        id: queued
            .map(|job| job.id)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        started_at: now.to_rfc3339(),
        lease_until: (now + chrono::Duration::seconds(600)).to_rfc3339(),
        config_revision: schedule.revision,
        manual,
    };
    if !manual {
        match schedule.definition.next(now) {
            Ok(next) => schedule.next_run_at = Some(next.to_rfc3339()),
            Err(_) => {
                schedule.definition.enabled = false;
                schedule.next_run_at = None;
            }
        }
    }
    schedule.running = Some(job.clone());
    let claim = Claim {
        schedule: schedule.id.clone(),
        owner: owner.into(),
        workspace: workspace.into(),
        job,
        definition: schedule.definition.clone(),
    };
    save(&tx, owner, &schedule).await?;
    save_run(&tx, owner, workspace, &occurrence(&claim)).await?;
    tx.commit().await?;
    Ok(Some(claim))
}
async fn execute(
    state: AppState,
    claim: Claim,
    shutdown: CancellationToken,
) -> Result<(), ApiError> {
    let active = running(&state, &claim).await?;
    if active
        .as_ref()
        .is_none_or(|job| job.id != claim.job.id || job.cancel_requested)
    {
        let mut record = occurrence(&claim);
        record.status = "cancelled".into();
        record.finished_at = Some(Utc::now().to_rfc3339());
        return finish(&state, &claim, record).await;
    }
    let input=serde_json::from_value(serde_json::json!({"collection_id":claim.definition.collection_id,"scenario_id":claim.definition.scenario_id,"environment_id":claim.definition.environment_id,"dataset_id":claim.definition.dataset_id,"iterations":claim.definition.iterations,"job_id":claim.job.id})).map_err(|_|ApiError::internal())?;
    let future = crate::runner::run(
        State(state.clone()),
        Extension(Identity(claim.owner.clone())),
        Path(claim.workspace.clone()),
        Json(input),
    );
    tokio::pin!(future);
    let watchdog = async {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let active = running(&state, &claim).await?;
            if active
                .as_ref()
                .is_none_or(|job| job.id != claim.job.id || job.cancel_requested)
            {
                return Ok::<(), ApiError>(());
            }
        }
    };
    tokio::pin!(watchdog);
    let result = tokio::select! { biased;
        _=shutdown.cancelled()=>{let _=state.project_jobs.cancel_job(&claim.owner,Some(&claim.workspace),&claim.job.id);return Ok(());},
        result=&mut future=>result,
        watched=&mut watchdog=>{
            let _=state.project_jobs.cancel_job(&claim.owner,Some(&claim.workspace),&claim.job.id);
            match watched {
                Err(error)=>Err(error),
                Ok(())=>tokio::select!{result=&mut future=>result,_=tokio::time::sleep(std::time::Duration::from_secs(2))=>Ok(Json(serde_json::json!({"cancelled":true}))),_=shutdown.cancelled()=>return Ok(())},
            }
        }
    };
    let mut record = occurrence(&claim);
    record.finished_at = Some(Utc::now().to_rfc3339());
    match result {
        Ok(Json(report)) => {
            record.report_id = report["report_id"].as_str().map(str::to_string);
            record.passed = report["passed"].as_u64().unwrap_or(0) as usize;
            record.failed = report["failed"].as_u64().unwrap_or(0) as usize;
            record.skipped = report["skipped"].as_u64().unwrap_or(0) as usize;
            record.status = if report["cancelled"] == true {
                "cancelled"
            } else if record.failed > 0 || report["stopped_reason"].is_string() {
                "failed"
            } else {
                "passed"
            }
            .into();
            if report.get("report_save_error").is_some() {
                record.error = Some("Run finished but its report could not be saved".into());
            }
        }
        Err(_) => {
            record.status = "failed".into();
            record.error =
                Some("Scheduled execution could not start; check references and capacity".into());
        }
    }
    finish(&state, &claim, record).await
}
async fn finish(state: &AppState, claim: &Claim, record: Occurrence) -> Result<(), ApiError> {
    let tx = state.db.begin().await?;
    if workspace_lock(&tx, &claim.owner, &claim.workspace)
        .await
        .is_err()
    {
        return Ok(());
    }
    let mut schedule = match locked(&tx, &claim.owner, &claim.workspace, &claim.schedule).await {
        Ok(schedule) => schedule,
        Err(_) => return Ok(()),
    };
    if schedule
        .running
        .as_ref()
        .is_some_and(|job| job.id == claim.job.id)
    {
        schedule.running = None;
        schedule.last_run = Some(record.clone());
        save(&tx, &claim.owner, &schedule).await?;
        save_run(&tx, &claim.owner, &claim.workspace, &record).await?;
        tx.commit().await?;
    }
    Ok(())
}

async fn running(state: &AppState, claim: &Claim) -> Result<Option<Job>, ApiError> {
    let row = document::Entity::find_by_id(&claim.schedule)
        .filter(document::Column::Owner.eq(&claim.owner))
        .filter(document::Column::Kind.eq(KIND))
        .filter(document::Column::RefId.eq(&claim.workspace))
        .one(&state.db)
        .await?;
    Ok(row
        .and_then(|row| serde_json::from_str::<super::models::Schedule>(&row.payload).ok())
        .and_then(|schedule| schedule.running))
}
