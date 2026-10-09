use super::{
    models::{Credentials, CredentialsPatch, Delivery, Metadata, Settings, Target},
    storage::{JOB, SECRET, TARGET, lock, save, save_delivery, secret_id, target},
};
use crate::{
    ApiError, AppState, auth::Identity, entities::document, storage as store, workspaces::owned,
};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, TransactionTrait};
use serde::Deserialize;
use serde_json::{Value, json};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Create {
    settings: Settings,
    credentials: Credentials,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Update {
    settings: Settings,
    #[serde(default)]
    credentials: Option<CredentialsPatch>,
    expected_revision: i64,
}
pub(crate) async fn list(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(workspace): Path<String>,
) -> Result<Json<Vec<Metadata>>, ApiError> {
    owned(&state, &owner.0, &workspace).await?;
    let targets: Vec<Target> =
        store::documents(&state.db, &owner.0, TARGET, Some(&workspace), 50).await?;
    let mut values = vec![];
    for item in targets {
        let (_, credentials) = target(&state.db, &owner.0, &workspace, &item.id).await?;
        values.push(item.metadata(&credentials));
    }
    Ok(Json(values))
}
pub(crate) async fn create(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(workspace): Path<String>,
    Json(input): Json<Create>,
) -> Result<Json<Metadata>, ApiError> {
    input.settings.validate()?;
    input.credentials.validate(&input.settings.kind)?;
    let tx = state.db.begin().await?;
    crate::scheduling::workspace_lock(&tx, &owner.0, &workspace).await?;
    let rows = document::Entity::find()
        .filter(document::Column::Owner.eq(&owner.0))
        .filter(document::Column::Kind.eq(TARGET))
        .filter(document::Column::RefId.eq(&workspace))
        .all(&tx)
        .await?;
    if rows.len() >= 50 {
        return Err(ApiError::bad("Workspace exceeds 50 notification targets"));
    }
    let channel = Target {
        id: uuid::Uuid::new_v4().to_string(),
        workspace_id: workspace.clone(),
        revision: 1,
        settings: input.settings,
    };
    store::insert_doc(
        &tx,
        channel.id.clone(),
        &owner.0,
        TARGET,
        &workspace,
        1,
        &channel,
    )
    .await?;
    store::insert_doc(
        &tx,
        secret_id(&channel.id),
        &owner.0,
        SECRET,
        &workspace,
        0,
        &input.credentials,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(channel.metadata(&input.credentials)))
}
pub(crate) async fn update(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path((workspace, id)): Path<(String, String)>,
    Json(input): Json<Update>,
) -> Result<Json<Metadata>, ApiError> {
    input.settings.validate()?;
    let tx = state.db.begin().await?;
    crate::scheduling::workspace_lock(&tx, &owner.0, &workspace).await?;
    lock(&tx, &owner.0, &workspace, &id, TARGET).await?;
    let (mut channel, previous) = target(&tx, &owner.0, &workspace, &id).await?;
    if channel.revision != input.expected_revision {
        return Err(ApiError::conflict("Notification target revision changed"));
    }
    let credentials = input
        .credentials
        .map(|patch| patch.apply(previous.clone()))
        .unwrap_or(previous);
    credentials.validate(&input.settings.kind)?;
    channel.settings = input.settings;
    channel.revision = channel
        .revision
        .checked_add(1)
        .ok_or_else(ApiError::internal)?;
    save(&tx, &owner.0, &id, TARGET, channel.revision, &channel).await?;
    save(&tx, &owner.0, &secret_id(&id), SECRET, 0, &credentials).await?;
    tx.commit().await?;
    Ok(Json(channel.metadata(&credentials)))
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
    crate::scheduling::workspace_lock(&tx, &owner.0, &workspace).await?;
    lock(&tx, &owner.0, &workspace, &id, TARGET).await?;
    let (channel, _) = target(&tx, &owner.0, &workspace, &id).await?;
    if channel.revision != input.expected_revision {
        return Err(ApiError::conflict("Notification target revision changed"));
    }
    document::Entity::delete_many()
        .filter(document::Column::Owner.eq(owner.0))
        .filter(document::Column::Id.is_in([id.clone(), secret_id(&id)]))
        .filter(document::Column::Kind.is_in([TARGET, SECRET]))
        .exec(&tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({"deleted":true})))
}
pub(crate) async fn deliveries(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(workspace): Path<String>,
) -> Result<Json<Vec<Delivery>>, ApiError> {
    owned(&state, &owner.0, &workspace).await?;
    let rows = document::Entity::find()
        .filter(document::Column::Owner.eq(owner.0))
        .filter(document::Column::Kind.eq(JOB))
        .filter(document::Column::RefId.eq(workspace))
        .order_by_desc(document::Column::UpdatedAt)
        .limit(200)
        .all(&state.db)
        .await?;
    Ok(Json(
        rows.into_iter()
            .map(|row| serde_json::from_str(&row.payload).map_err(|_| ApiError::internal()))
            .collect::<Result<Vec<_>, _>>()?,
    ))
}
pub(crate) async fn retry(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path((workspace, id)): Path<(String, String)>,
) -> Result<Json<Delivery>, ApiError> {
    let tx = state.db.begin().await?;
    crate::scheduling::workspace_lock(&tx, &owner.0, &workspace).await?;
    let row = lock(&tx, &owner.0, &workspace, &id, JOB).await?;
    let mut delivery: Delivery =
        serde_json::from_str(&row.payload).map_err(|_| ApiError::internal())?;
    if !matches!(delivery.status.as_str(), "failed" | "cancelled") {
        return Err(ApiError::conflict(
            "Only a terminal failed delivery can be retried",
        ));
    }
    let (channel, _) = target(&tx, &owner.0, &workspace, &delivery.target_id).await?;
    if !channel.settings.enabled {
        return Err(ApiError::bad("Notification target is disabled"));
    }
    delivery.target_revision = channel.revision;
    delivery.status = "pending".into();
    delivery.attempts = 0;
    delivery.next_attempt_at = store::now();
    delivery.lease_id = None;
    delivery.lease_until = None;
    delivery.last_error = None;
    save_delivery(&tx, &owner.0, &delivery).await?;
    tx.commit().await?;
    Ok(Json(delivery))
}
