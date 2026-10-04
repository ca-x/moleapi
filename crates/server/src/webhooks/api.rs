use super::{manager, models::*, storage as vault};
use crate::{ApiError, AppState, auth::Identity, entities::document, storage, workspaces::owned};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, TransactionTrait};
pub(super) fn validate(name: &str, reply: &Reply) -> Result<(), ApiError> {
    if name.trim().is_empty()
        || name.len() > 256
        || !(200..=599).contains(&reply.status)
        || reply.body.len() > 65536
        || reply.headers.len() > 32
    {
        return Err(ApiError::bad(
            "Invalid Webhook receiver response/name limits",
        ));
    }
    if matches!(reply.status, 204 | 304) && !reply.body.is_empty() {
        return Err(ApiError::bad("This response status does not permit a body"));
    }
    let mut bytes = 0;
    for header in reply.headers.iter().filter(|p| p.enabled) {
        bytes += header.key.len() + header.value.len();
        axum::http::HeaderName::from_bytes(header.key.as_bytes())
            .map_err(|_| ApiError::bad("Invalid receiver response header name"))?;
        axum::http::HeaderValue::from_str(&header.value)
            .map_err(|_| ApiError::bad("Invalid receiver response header value"))?;
    }
    if bytes > 16384 {
        return Err(ApiError::bad("Receiver response headers exceed limit"));
    }
    Ok(())
}
pub(super) async fn owned_inbox(s: &AppState, owner: &str, id: &str) -> Result<Inbox, ApiError> {
    let (_, value) = vault::inbox(&s.db, id, Some(owner))
        .await?
        .ok_or_else(ApiError::not_found)?;
    owned(s, owner, &value.workspace_id).await?;
    Ok(value)
}
pub async fn list(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(workspace): Path<String>,
) -> Result<Json<Vec<InboxView>>, ApiError> {
    owned(&s, &owner.0, &workspace).await?;
    Ok(Json(
        vault::inboxes_for_workspace(&s.db, &owner.0, &workspace)
            .await?
            .into_iter()
            .map(Into::into)
            .collect(),
    ))
}
pub async fn create(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(workspace): Path<String>,
    Json(input): Json<Create>,
) -> Result<Json<InboxView>, ApiError> {
    validate(&input.name, &input.response)?;
    owned(&s, &owner.0, &workspace).await?;
    let _admission = s.webhooks.admission.lock().await;
    let _gate = s.webhooks.gates.lock(&owner.0, &workspace).await;
    owned(&s, &owner.0, &workspace).await?;
    let total = document::Entity::find()
        .filter(document::Column::Kind.eq(vault::INBOX))
        .count(&s.db)
        .await?;
    let count = document::Entity::find()
        .filter(document::Column::Kind.eq(vault::INBOX))
        .filter(document::Column::Owner.eq(&owner.0))
        .count(&s.db)
        .await?;
    if total >= 128 || count >= 4 {
        return Err(manager::capacity("Webhook receiver capacity reached"));
    }
    let token = uuid::Uuid::new_v4().simple().to_string();
    let now = storage::now();
    let inbox = Inbox {
        id: vault::token_id(&token),
        workspace_id: workspace.clone(),
        name: input.name,
        token,
        active: true,
        response: input.response,
        config_epoch: 1,
        revision: 1,
        created_at: now.clone(),
        updated_at: now,
        received: 0,
        dropped: 0,
    };
    storage::insert_doc(
        &s.db,
        inbox.id.clone(),
        &owner.0,
        vault::INBOX,
        &workspace,
        1,
        &inbox,
    )
    .await?;
    Ok(Json(inbox.into()))
}
pub async fn get(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
) -> Result<Json<InboxView>, ApiError> {
    Ok(Json(owned_inbox(&s, &owner.0, &id).await?.into()))
}
pub async fn update(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
    Json(input): Json<Update>,
) -> Result<Json<InboxView>, ApiError> {
    validate(&input.name, &input.response)?;
    if input.expected_revision < 1 || input.expected_revision >= i64::MAX - 1 {
        return Err(ApiError::bad("Invalid receiver revision"));
    }
    let initial = owned_inbox(&s, &owner.0, &id).await?;
    let _gate = s.webhooks.gates.lock(&owner.0, &initial.workspace_id).await;
    let mut value = owned_inbox(&s, &owner.0, &id).await?;
    if value.revision != input.expected_revision {
        return Err(ApiError::conflict("Webhook receiver revision changed"));
    }
    if value.config_epoch == u64::MAX {
        return Err(ApiError::bad(
            "Webhook configuration lifetime limit reached",
        ));
    }
    value.config_epoch += 1;
    value.name = input.name;
    value.active = input.active;
    value.response = input.response;
    value.revision += 1;
    value.updated_at = storage::now();
    vault::update(&s.db, &owner.0, &value, input.expected_revision).await?;
    Ok(Json(value.into()))
}
pub async fn delete(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
    Json(input): Json<Revision>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let initial = owned_inbox(&s, &owner.0, &id).await?;
    let _gate = s.webhooks.gates.lock(&owner.0, &initial.workspace_id).await;
    let value = owned_inbox(&s, &owner.0, &id).await?;
    if value.revision != input.expected_revision {
        return Err(ApiError::conflict("Webhook receiver revision changed"));
    }
    let tx = s.db.begin().await?;
    vault::delete_captures(&tx, &owner.0, std::slice::from_ref(&id)).await?;
    document::Entity::delete_many()
        .filter(document::Column::Id.eq(&id))
        .filter(document::Column::Owner.eq(&owner.0))
        .filter(document::Column::Kind.eq(vault::INBOX))
        .exec(&tx)
        .await?;
    tx.commit().await?;
    s.webhooks.cancel_scope(&owner.0, None, Some(&id));
    Ok(Json(serde_json::json!({"deleted":true})))
}
pub async fn clear(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let initial = owned_inbox(&s, &owner.0, &id).await?;
    let _gate = s.webhooks.gates.lock(&owner.0, &initial.workspace_id).await;
    owned_inbox(&s, &owner.0, &id).await?;
    vault::delete_captures(&s.db, &owner.0, std::slice::from_ref(&id)).await?;
    Ok(Json(serde_json::json!({"cleared":true})))
}
