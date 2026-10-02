use crate::entities::document;
use crate::{ApiError, AppState, auth::Identity, storage, workspaces::owned};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use moleapi_core::HistoryEntry;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
pub async fn history(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
) -> Result<Json<Vec<HistoryEntry>>, ApiError> {
    owned(&s, &owner.0, &id).await?;
    Ok(Json(
        storage::documents(&s.db, &owner.0, "history", Some(&id), 100).await?,
    ))
}
pub async fn clear_history(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    owned(&s, &owner.0, &id).await?;
    document::Entity::delete_many()
        .filter(document::Column::Owner.eq(owner.0))
        .filter(document::Column::Kind.eq("history"))
        .filter(document::Column::RefId.eq(id))
        .exec(&s.db)
        .await?;
    Ok(Json(serde_json::json!({"ok":true})))
}

pub(crate) async fn record(
    s: &AppState,
    owner: &str,
    w: &moleapi_core::Workspace,
    r: &moleapi_core::RequestSpec,
    environment: Option<&moleapi_core::Environment>,
    response: &moleapi_core::Response,
) -> Result<(), ApiError> {
    let mut stored = response.clone();
    stored.url = moleapi_core::redact_url(&response.url, environment);
    let entry = HistoryEntry {
        id: uuid::Uuid::new_v4().to_string(),
        workspace_id: w.id.clone(),
        request_id: r.id.clone(),
        request_name: r.name.clone(),
        method: r.method.clone(),
        url: stored.url.clone(),
        status: stored.status,
        elapsed_ms: stored.elapsed_ms,
        size_bytes: stored.size_bytes,
        created_at: storage::now(),
        response: stored,
    };
    // The workspace may have been deleted while the endpoint was running.
    if storage::get(&s.db, owner, &w.id).await?.is_some() {
        storage::insert_doc(&s.db, entry.id.clone(), owner, "history", &w.id, 0, &entry).await?;
    }
    Ok(())
}
