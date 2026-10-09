use super::{
    models::{Delivery, Event, Target},
    storage::{self, JOB, lock, save_delivery, target},
    transport::{self, Outcome},
};
use crate::{ApiError, AppState, entities::document, storage as store};
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    TransactionTrait,
};
use std::{collections::BTreeSet, sync::Arc};
use tokio_util::sync::CancellationToken;
pub(crate) async fn validate_targets<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    workspace: &str,
    ids: &[String],
) -> Result<(), ApiError> {
    let unique = ids.iter().collect::<BTreeSet<_>>();
    if ids.len() > 20 || unique.len() != ids.len() {
        return Err(ApiError::bad("Select up to 20 unique notification targets"));
    }
    for id in ids {
        target(db, owner, workspace, id).await?;
    }
    Ok(())
}
pub(crate) async fn enqueue<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    workspace: &moleapi_core::Workspace,
    ids: &[String],
    schedule_name: &str,
    previous_status: Option<&str>,
    run: &crate::scheduling::Occurrence,
) -> Result<(usize, usize), ApiError> {
    enqueue_event(
        db,
        owner,
        workspace,
        run,
        Binding {
            ids,
            name: schedule_name,
            previous_status,
            kind: "SCHEDULE_RUN_COMPLETED",
            workspace_name: None,
        },
    )
    .await
}
struct Binding<'a> {
    ids: &'a [String],
    name: &'a str,
    previous_status: Option<&'a str>,
    kind: &'a str,
    workspace_name: Option<&'a str>,
}
async fn enqueue_event<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    workspace: &moleapi_core::Workspace,
    run: &crate::scheduling::Occurrence,
    binding: Binding<'_>,
) -> Result<(usize, usize), ApiError> {
    let Binding {
        ids,
        name: schedule_name,
        previous_status,
        kind,
        workspace_name,
    } = binding;
    if ids.is_empty() {
        return Ok((0, 0));
    }
    let pending = document::Entity::find()
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(JOB))
        .filter(document::Column::RefId.eq(&workspace.id))
        .filter(document::Column::Revision.ne(i64::MAX))
        .limit(1000)
        .all(db)
        .await?
        .len();
    let (mut queued, mut skipped) = (0, 0);
    let privacy = moleapi_formats::RunReportPrivacy::new(workspace);
    for id in ids {
        let Ok((channel, credentials)) = target(db, owner, &workspace.id, id).await else {
            skipped += 1;
            continue;
        };
        if !channel.settings.enabled
            || !channel.settings.statuses.contains(&run.status)
            || (channel.settings.changes_only && previous_status == Some(run.status.as_str()))
        {
            continue;
        }
        if pending + queued >= 1000 {
            skipped += 1;
            continue;
        }
        let mut values = BTreeSet::from([
            credentials.endpoint.clone(),
            credentials.signing_secret.clone(),
            credentials.routing_key.clone(),
        ]);
        if let Some(smtp) = &credentials.smtp {
            values.insert(smtp.password.clone());
            values.insert(smtp.username.clone());
        }
        values.remove("");
        if let Ok(url) = url::Url::parse(&credentials.endpoint) {
            for (_, value) in url.query_pairs() {
                if !value.is_empty() {
                    values.insert(value.into_owned());
                }
            }
        }
        let redactor = crate::privacy::Redactor::new(&values)?;
        let screen = |text: &str| {
            let mut value = serde_json::json!(privacy.screen(text, 256));
            redactor.scrub(&mut value);
            value.as_str().unwrap_or("").to_string()
        };
        let event = Event {
            kind: kind.into(),
            id: run.id.clone(),
            schedule_id: run.schedule_id.clone(),
            job_id: run.id.clone(),
            status: run.status.clone(),
            previous_status: previous_status.map(str::to_string),
            finished_at: run.finished_at.clone().unwrap_or_else(store::now),
            report_id: run.report_id.clone(),
            passed: run.passed,
            failed: run.failed,
            skipped: run.skipped,
            workspace_name: screen(workspace_name.unwrap_or(&workspace.name)),
            schedule_name: screen(schedule_name),
        };
        let delivery = Delivery {
            id: uuid::Uuid::new_v4().to_string(),
            target_id: id.clone(),
            target_revision: channel.revision,
            event,
            status: "pending".into(),
            attempts: 0,
            next_attempt_at: store::now(),
            lease_until: None,
            lease_id: None,
            last_http_status: None,
            last_error: None,
            delivered_at: None,
        };
        store::insert_doc(
            db,
            delivery.id.clone(),
            owner,
            JOB,
            &workspace.id,
            delivery.wake_at(),
            &delivery,
        )
        .await?;
        queued += 1;
    }
    storage::retain(db, owner, &workspace.id).await?;
    Ok((queued, skipped))
}
pub(crate) struct Lifetime {
    state: AppState,
    stop: CancellationToken,
    slots: Arc<tokio::sync::Semaphore>,
}
impl Drop for Lifetime {
    fn drop(&mut self) {
        self.stop.cancel();
    }
}
pub(crate) fn start(state: AppState) -> Arc<Lifetime> {
    let guard = Arc::new(Lifetime {
        state,
        stop: CancellationToken::new(),
        slots: Arc::new(tokio::sync::Semaphore::new(2)),
    });
    let weak = Arc::downgrade(&guard);
    let stop = guard.stop.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! {_=stop.cancelled()=>break,_=tokio::time::sleep(std::time::Duration::from_millis(400))=>{}}
            let Some((state, slots)) = weak
                .upgrade()
                .map(|guard| (guard.state.clone(), guard.slots.clone()))
            else {
                break;
            };
            if let Err(error) = tick(&state, slots, &stop).await {
                eprintln!("notification delivery scan failed: {}", error.message);
            }
        }
    });
    guard
}
struct Claim {
    owner: String,
    workspace: String,
    delivery: Delivery,
    channel: Target,
    credentials: super::models::Credentials,
}
async fn tick(
    state: &AppState,
    slots: Arc<tokio::sync::Semaphore>,
    stop: &CancellationToken,
) -> Result<(), ApiError> {
    let rows = document::Entity::find()
        .filter(document::Column::Kind.eq(JOB))
        .filter(document::Column::Revision.lte(chrono::Utc::now().timestamp_millis()))
        .order_by_asc(document::Column::Revision)
        .limit(16)
        .all(&state.db)
        .await?;
    for row in rows {
        let Ok(permit) = slots.clone().try_acquire_owned() else {
            break;
        };
        if let Some(claim) = claim(state, &row.owner, &row.ref_id, &row.id).await? {
            let state = state.clone();
            let stop = stop.clone();
            tokio::spawn(async move {
                let _permit = permit;
                if let Err(error) = deliver(&state, claim, stop).await {
                    eprintln!("notification delivery could not finish: {}", error.message);
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
    let tx = state.db.begin().await?;
    crate::scheduling::workspace_lock(&tx, owner, workspace).await?;
    let row = lock(&tx, owner, workspace, id, JOB).await?;
    let mut delivery: Delivery =
        serde_json::from_str(&row.payload).map_err(|_| ApiError::internal())?;
    if delivery.wake_at() > chrono::Utc::now().timestamp_millis()
        || !matches!(delivery.status.as_str(), "pending" | "sending")
    {
        tx.commit().await?;
        return Ok(None);
    }
    let selected = target(&tx, owner, workspace, &delivery.target_id).await;
    let valid = selected.as_ref().is_ok_and(|(channel, _)| {
        channel.settings.enabled && channel.revision == delivery.target_revision
    });
    if !valid {
        delivery.status = "cancelled".into();
        delivery.last_error = Some("Notification target was removed, disabled or changed".into());
        save_delivery(&tx, owner, &delivery).await?;
        tx.commit().await?;
        return Ok(None);
    }
    if delivery.attempts >= 3 {
        delivery.status = "failed".into();
        delivery.last_error = Some("Notification retry limit reached".into());
        save_delivery(&tx, owner, &delivery).await?;
        tx.commit().await?;
        return Ok(None);
    }
    let (channel, credentials) = selected?;
    delivery.attempts += 1;
    delivery.status = "sending".into();
    delivery.lease_id = Some(uuid::Uuid::new_v4().to_string());
    delivery.lease_until = Some((chrono::Utc::now() + chrono::Duration::seconds(60)).to_rfc3339());
    save_delivery(&tx, owner, &delivery).await?;
    tx.commit().await?;
    Ok(Some(Claim {
        owner: owner.into(),
        workspace: workspace.into(),
        delivery,
        channel,
        credentials,
    }))
}
async fn deliver(state: &AppState, claim: Claim, stop: CancellationToken) -> Result<(), ApiError> {
    let result = tokio::select! {_=stop.cancelled()=>return Ok(()),result=transport::send(state,&claim.channel.settings,&claim.credentials,&claim.delivery.event,&claim.delivery.id)=>result};
    let tx = state.db.begin().await?;
    if crate::scheduling::workspace_lock(&tx, &claim.owner, &claim.workspace)
        .await
        .is_err()
    {
        return Ok(());
    }
    let row = match lock(&tx, &claim.owner, &claim.workspace, &claim.delivery.id, JOB).await {
        Ok(row) => row,
        Err(_) => return Ok(()),
    };
    let mut delivery: Delivery =
        serde_json::from_str(&row.payload).map_err(|_| ApiError::internal())?;
    if delivery.lease_id != claim.delivery.lease_id {
        return Ok(());
    }
    match result {
        Outcome::Accepted(status) => {
            delivery.status = "sent".into();
            delivery.last_http_status = status;
            delivery.last_error = None;
            delivery.delivered_at = Some(store::now());
        }
        Outcome::Retry(status, error) => {
            delivery.status = if delivery.attempts >= 3 {
                "failed"
            } else {
                "pending"
            }
            .into();
            delivery.last_http_status = status;
            delivery.last_error = Some(error.into());
            delivery.next_attempt_at = (chrono::Utc::now()
                + chrono::Duration::seconds(2i64.pow(delivery.attempts as u32)))
            .to_rfc3339();
        }
        Outcome::Rejected(status, error) => {
            delivery.status = "failed".into();
            delivery.last_http_status = status;
            delivery.last_error = Some(error.into());
        }
    }
    delivery.lease_id = None;
    delivery.lease_until = None;
    save_delivery(&tx, &claim.owner, &delivery).await?;
    storage::retain(&tx, &claim.owner, &claim.workspace).await?;
    tx.commit().await?;
    Ok(())
}

pub(crate) async fn enqueue_run<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    source: &crate::run_reports::Source<'_>,
    report: &moleapi_core::SavedRunReport,
) -> Result<(), ApiError> {
    if source.notification_ids.is_empty() {
        return Ok(());
    }
    let resource = source
        .scenario
        .map_or(&source.collection.id, |scenario| &scenario.id);
    let key = store::workspace_key(
        owner,
        &format!(
            "run-notify:{}:{}:{}",
            source.workspace.id, resource, source.run_origin
        ),
    );
    let previous = document::Entity::find_by_id(&key)
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq("run-notify"))
        .filter(document::Column::RefId.eq(&source.workspace.id))
        .one(db)
        .await?;
    let previous_status = previous
        .as_ref()
        .and_then(|row| serde_json::from_str::<serde_json::Value>(&row.payload).ok())
        .and_then(|value| value["status"].as_str().map(str::to_string));
    let status = if report.summary.cancelled {
        "cancelled"
    } else if report.summary.failed > 0 || report.summary.stopped_reason.is_some() {
        "failed"
    } else {
        "passed"
    };
    let run = crate::scheduling::Occurrence {
        notification_queued: 0,
        notification_skipped: 0,
        id: report.id.clone(),
        schedule_id: format!("{}:{resource}", source.run_origin),
        config_revision: report.workspace_revision,
        manual: true,
        status: status.into(),
        started_at: report.started_at.clone(),
        finished_at: Some(report.finished_at.clone()),
        report_id: Some(report.id.clone()),
        passed: report.summary.passed,
        failed: report.summary.failed,
        skipped: report.summary.skipped,
        error: None,
    };
    enqueue_event(
        db,
        owner,
        source.workspace,
        &run,
        Binding {
            ids: source.notification_ids,
            workspace_name: Some(&report.workspace_name),
            name: report
                .scenario_name
                .as_ref()
                .unwrap_or(&report.collection_name),
            previous_status: previous_status.as_deref(),
            kind: if source.run_origin == "ci" {
                "CI_RUN_COMPLETED"
            } else {
                "RUN_COMPLETED"
            },
        },
    )
    .await?;
    let value = serde_json::json!({"status":status,"report_id":report.id});
    if previous.is_some() {
        storage::save(db, owner, &key, "run-notify", 0, &value).await?;
    } else {
        store::insert_doc(
            db,
            key,
            owner,
            "run-notify",
            &source.workspace.id,
            0,
            &value,
        )
        .await?;
    }
    Ok(())
}
