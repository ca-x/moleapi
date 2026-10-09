use super::{
    models::{Definition, Occurrence, Queued, Schedule},
    storage::{KIND, RUN, locked, save, save_run, workspace_lock},
};
use crate::{
    ApiError, AppState, auth::Identity, entities::document, storage as store, workspaces::owned,
};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use chrono::Utc;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, TransactionTrait};
use serde::Deserialize;
use serde_json::{Value, json};
pub(crate) async fn list(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(workspace): Path<String>,
) -> Result<Json<Vec<Schedule>>, ApiError> {
    owned(&state, &owner.0, &workspace).await?;
    let rows: Vec<Schedule> =
        store::documents(&state.db, &owner.0, KIND, Some(&workspace), 100).await?;
    Ok(Json(rows))
}
pub(crate) async fn create(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(workspace): Path<String>,
    Json(definition): Json<Definition>,
) -> Result<Json<Schedule>, ApiError> {
    let now = Utc::now();
    let next = definition.next(now)?;
    let tx = state.db.begin().await?;
    let data = workspace_lock(&tx, &owner.0, &workspace).await?;
    definition.validate_refs(&data)?;
    let existing = document::Entity::find()
        .filter(document::Column::Owner.eq(&owner.0))
        .filter(document::Column::Kind.eq(KIND))
        .filter(document::Column::RefId.eq(&workspace))
        .all(&tx)
        .await?;
    if existing.len() >= 100 {
        return Err(ApiError::bad("Workspace exceeds 100 schedules"));
    }
    let schedule = Schedule {
        id: uuid::Uuid::new_v4().to_string(),
        workspace_id: workspace.clone(),
        revision: 1,
        definition,
        next_run_at: Some(next.to_rfc3339()),
        queued: None,
        running: None,
        last_run: None,
        created_at: now.to_rfc3339(),
        updated_at: now.to_rfc3339(),
    };
    store::insert_doc(
        &tx,
        schedule.id.clone(),
        &owner.0,
        KIND,
        &workspace,
        schedule.wake_at(),
        &schedule,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(schedule))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Update {
    definition: Definition,
    expected_revision: i64,
}
pub(crate) async fn update(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path((workspace, id)): Path<(String, String)>,
    Json(input): Json<Update>,
) -> Result<Json<Schedule>, ApiError> {
    let next = input.definition.next(Utc::now())?;
    let tx = state.db.begin().await?;
    let data = workspace_lock(&tx, &owner.0, &workspace).await?;
    input.definition.validate_refs(&data)?;
    let mut schedule = locked(&tx, &owner.0, &workspace, &id).await?;
    if schedule.revision != input.expected_revision {
        return Err(ApiError::conflict("Schedule revision changed"));
    }
    let pending = schedule.queued.take();
    if let Some(queued) = &pending {
        save_run(
            &tx,
            &owner.0,
            &workspace,
            &queued_record(&schedule, queued, "cancelled"),
        )
        .await?;
    }
    if let Some(job) = &mut schedule.running {
        job.cancel_requested = true;
    }
    schedule.definition = input.definition;
    schedule.revision = schedule
        .revision
        .checked_add(1)
        .ok_or_else(ApiError::internal)?;
    schedule.next_run_at = Some(next.to_rfc3339());
    schedule.updated_at = store::now();
    save(&tx, &owner.0, &schedule).await?;
    tx.commit().await?;
    if let Some(job) = pending {
        let _ = state
            .project_jobs
            .cancel_job(&owner.0, Some(&workspace), &job.id);
    }
    if let Some(job) = &schedule.running {
        let _ = state
            .project_jobs
            .cancel_job(&owner.0, Some(&workspace), &job.id);
    }
    Ok(Json(schedule))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Revision {
    expected_revision: i64,
}
pub(crate) async fn remove(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path((workspace, id)): Path<(String, String)>,
    Json(input): Json<Revision>,
) -> Result<Json<Value>, ApiError> {
    let tx = state.db.begin().await?;
    workspace_lock(&tx, &owner.0, &workspace).await?;
    let schedule = locked(&tx, &owner.0, &workspace, &id).await?;
    if schedule.revision != input.expected_revision {
        return Err(ApiError::conflict("Schedule revision changed"));
    }
    document::Entity::delete_many()
        .filter(document::Column::Id.eq(&id))
        .filter(document::Column::Owner.eq(&owner.0))
        .filter(document::Column::Kind.eq(KIND))
        .exec(&tx)
        .await?;
    document::Entity::delete_many()
        .filter(document::Column::Owner.eq(&owner.0))
        .filter(document::Column::Kind.eq(RUN))
        .filter(document::Column::RefId.eq(&workspace))
        .filter(document::Column::Id.starts_with(format!("sched-run-{id}-")))
        .exec(&tx)
        .await?;
    tx.commit().await?;
    for job in schedule
        .queued
        .map(|job| job.id)
        .into_iter()
        .chain(schedule.running.map(|job| job.id))
    {
        let _ = state
            .project_jobs
            .cancel_job(&owner.0, Some(&workspace), &job);
    }
    Ok(Json(json!({"deleted":true})))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Preview {
    cron: String,
    timezone: String,
}
pub(crate) async fn preview(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(workspace): Path<String>,
    Json(input): Json<Preview>,
) -> Result<Json<Value>, ApiError> {
    owned(&state, &owner.0, &workspace).await?;
    let definition = Definition {
        name: "Preview".into(),
        cron: input.cron,
        timezone: input.timezone,
        enabled: false,
        collection_id: String::new(),
        scenario_id: None,
        environment_id: None,
        dataset_id: None,
        iterations: None,
    };
    let mut now = Utc::now();
    let mut next = vec![];
    for _ in 0..5 {
        now = definition.next(now)?;
        next.push(now.to_rfc3339());
    }
    Ok(Json(json!({"next":next})))
}
pub(crate) async fn queue(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path((workspace, id)): Path<(String, String)>,
) -> Result<Json<Schedule>, ApiError> {
    let tx = state.db.begin().await?;
    workspace_lock(&tx, &owner.0, &workspace).await?;
    let mut schedule = locked(&tx, &owner.0, &workspace, &id).await?;
    if schedule.queued.is_some() {
        return Err(ApiError::conflict(
            "Schedule already has a queued manual run",
        ));
    }
    schedule.queued = Some(Queued {
        id: uuid::Uuid::new_v4().to_string(),
        requested_at: store::now(),
    });
    if let Some(queued) = &schedule.queued {
        save_run(
            &tx,
            &owner.0,
            &workspace,
            &queued_record(&schedule, queued, "queued"),
        )
        .await?;
    }
    save(&tx, &owner.0, &schedule).await?;
    tx.commit().await?;
    Ok(Json(schedule))
}
pub(crate) async fn cancel(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path((workspace, id)): Path<(String, String)>,
) -> Result<Json<Schedule>, ApiError> {
    let tx = state.db.begin().await?;
    workspace_lock(&tx, &owner.0, &workspace).await?;
    let mut schedule = locked(&tx, &owner.0, &workspace, &id).await?;
    let pending = schedule.queued.take();
    if let Some(queued) = &pending {
        save_run(
            &tx,
            &owner.0,
            &workspace,
            &queued_record(&schedule, queued, "cancelled"),
        )
        .await?;
    }
    if let Some(job) = &mut schedule.running {
        job.cancel_requested = true;
    }
    save(&tx, &owner.0, &schedule).await?;
    tx.commit().await?;
    for job in pending
        .map(|job| job.id)
        .into_iter()
        .chain(schedule.running.as_ref().map(|job| job.id.clone()))
    {
        let _ = state
            .project_jobs
            .cancel_job(&owner.0, Some(&workspace), &job);
    }
    Ok(Json(schedule))
}
pub(crate) async fn history(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path((workspace, id)): Path<(String, String)>,
) -> Result<Json<Vec<Occurrence>>, ApiError> {
    owned(&state, &owner.0, &workspace).await?;
    let row = document::Entity::find_by_id(&id)
        .filter(document::Column::Owner.eq(&owner.0))
        .filter(document::Column::Kind.eq(KIND))
        .filter(document::Column::RefId.eq(&workspace))
        .one(&state.db)
        .await?
        .ok_or_else(ApiError::not_found)?;
    let _: Schedule = serde_json::from_str(&row.payload).map_err(|_| ApiError::internal())?;
    use sea_orm::{QueryOrder, QuerySelect};
    let rows = document::Entity::find()
        .filter(document::Column::Owner.eq(owner.0))
        .filter(document::Column::Kind.eq(RUN))
        .filter(document::Column::RefId.eq(workspace))
        .filter(document::Column::Id.starts_with(format!("sched-run-{id}-")))
        .order_by_desc(document::Column::UpdatedAt)
        .limit(100)
        .all(&state.db)
        .await?;
    Ok(Json(
        rows.into_iter()
            .map(|row| serde_json::from_str(&row.payload).map_err(|_| ApiError::internal()))
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

fn queued_record(schedule: &Schedule, job: &Queued, status: &str) -> Occurrence {
    Occurrence {
        id: job.id.clone(),
        schedule_id: schedule.id.clone(),
        config_revision: schedule.revision,
        manual: true,
        status: status.into(),
        started_at: job.requested_at.clone(),
        finished_at: (status == "cancelled").then(store::now),
        report_id: None,
        passed: 0,
        failed: 0,
        skipped: 0,
        error: None,
    }
}
