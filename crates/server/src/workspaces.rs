use crate::{ApiError, AppState, auth::Identity, entities::document, storage};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use moleapi_core::{Workspace, WorkspaceData};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, TransactionTrait, sea_query::Expr};
use serde::Deserialize;
#[derive(Deserialize)]
pub struct Create {
    id: Option<String>,
    name: String,
    data: WorkspaceData,
}
#[derive(Deserialize)]
pub struct Update {
    name: String,
    data: WorkspaceData,
    expected_revision: i64,
}
#[derive(Deserialize)]
pub struct Revision {
    expected_revision: i64,
}
fn validate(name: &str, data: &WorkspaceData) -> Result<(), ApiError> {
    if name.trim().is_empty() || name.len() > 256 {
        return Err(ApiError::bad("Workspace name must be 1–256 bytes"));
    }
    moleapi_core::validate_workspace(data).map_err(|e| ApiError::bad(e.to_string()))
}
pub async fn owned(state: &AppState, owner: &str, id: &str) -> Result<Workspace, ApiError> {
    storage::get(&state.db, owner, id)
        .await?
        .ok_or_else(ApiError::not_found)
}
pub async fn list(
    State(s): State<AppState>,
    Extension(id): Extension<Identity>,
) -> Result<Json<Vec<Workspace>>, ApiError> {
    Ok(Json(
        storage::documents(&s.db, &id.0, "workspace", None, 10000).await?,
    ))
}
pub async fn create(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Json(mut c): Json<Create>,
) -> Result<Json<Workspace>, ApiError> {
    validate(&c.name, &c.data)?;
    if !s.local {
        moleapi_core::scrub_local_values(&mut c.data);
    }
    let id = c.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    if id.is_empty()
        || matches!(id.as_str(), "." | "..")
        || id.len() > 128
        || id.chars().any(|c| c.is_control() || c == '/' || c == '\\')
    {
        return Err(ApiError::bad("Invalid workspace ID"));
    }
    let tx = s.db.begin().await?;
    let key = storage::workspace_key(&owner.0, &id);
    let previous = document::Entity::find_by_id(&key).one(&tx).await?;
    let revision = match &previous {
        Some(row)
            if row.owner == owner.0 && row.kind == "tombstone" && row.revision < i64::MAX - 1 =>
        {
            row.revision + 1
        }
        Some(_) => return Err(ApiError::conflict("Workspace ID exists")),
        None => 1,
    };
    let w = Workspace {
        id: id.clone(),
        name: c.name,
        revision,
        updated_at: storage::now(),
        data: c.data,
    };
    if let Some(previous) = previous {
        let result = document::Entity::update_many()
            .col_expr(document::Column::Kind, Expr::value("workspace"))
            .col_expr(document::Column::Revision, Expr::value(revision))
            .col_expr(
                document::Column::Payload,
                Expr::value(serde_json::to_string(&w).map_err(|_| ApiError::internal())?),
            )
            .col_expr(document::Column::UpdatedAt, Expr::value(storage::now()))
            .filter(document::Column::Id.eq(&key))
            .filter(document::Column::Owner.eq(&owner.0))
            .filter(document::Column::Kind.eq("tombstone"))
            .filter(document::Column::Revision.eq(previous.revision))
            .exec(&tx)
            .await?;
        if result.rows_affected != 1 {
            return Err(ApiError::conflict("Workspace ID changed during creation"));
        }
    } else {
        storage::insert_doc(&tx, key, &owner.0, "workspace", &id, revision, &w)
            .await
            .map_err(|e| {
                if matches!(
                    e.sql_err(),
                    Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
                ) {
                    ApiError::conflict("Workspace ID exists")
                } else {
                    ApiError::from(e)
                }
            })?;
    }
    tx.commit().await?;
    Ok(Json(w))
}
pub async fn get(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
) -> Result<Json<Workspace>, ApiError> {
    Ok(Json(owned(&s, &owner.0, &id).await?))
}
pub async fn update(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
    Json(mut c): Json<Update>,
) -> Result<Json<Workspace>, ApiError> {
    validate(&c.name, &c.data)?;
    if !s.local {
        moleapi_core::scrub_local_values(&mut c.data);
    }
    if !(1..i64::MAX).contains(&c.expected_revision) {
        return Err(ApiError::bad("Invalid expected revision"));
    }
    let mut w = owned(&s, &owner.0, &id).await?;
    w.name = c.name;
    w.data = c.data;
    let w = storage::replace(&s.db, &owner.0, w, c.expected_revision)
        .await?
        .ok_or_else(|| ApiError::conflict("Workspace revision changed"))?;
    reconcile_sessions(&s, &owner.0, &w).await;
    Ok(Json(w))
}
pub async fn delete(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
    Json(c): Json<Revision>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if !(1..i64::MAX).contains(&c.expected_revision) {
        return Err(ApiError::bad("Invalid expected revision"));
    }
    owned(&s, &owner.0, &id).await?;
    let _webhook_gate = s.webhooks.gates.lock(&owner.0, &id).await;
    let tx = s.db.begin().await?;
    crate::webhooks::cascade(&tx, &owner.0, &id).await?;
    let result = document::Entity::update_many()
        .col_expr(document::Column::Kind, Expr::value("tombstone"))
        .col_expr(document::Column::Payload, Expr::value("{}"))
        .col_expr(
            document::Column::Revision,
            Expr::value(c.expected_revision + 1),
        )
        .col_expr(document::Column::UpdatedAt, Expr::value(storage::now()))
        .filter(document::Column::Kind.eq("workspace"))
        .filter(document::Column::Id.eq(storage::workspace_key(&owner.0, &id)))
        .filter(document::Column::Owner.eq(&owner.0))
        .filter(document::Column::Revision.eq(c.expected_revision))
        .exec(&tx)
        .await?;
    if result.rows_affected != 1 {
        return Err(ApiError::conflict("Workspace revision changed"));
    }
    document::Entity::delete_many()
        .filter(document::Column::Owner.eq(&owner.0))
        .filter(document::Column::RefId.eq(&id))
        .filter(document::Column::Kind.ne("tombstone"))
        .exec(&tx)
        .await?;
    tx.commit().await?;
    s.cookies.clear_scope(&owner.0, Some(&id));
    s.oauth2_flows.cancel_workspace(&owner.0, &id);
    s.oauth1_flows.cancel_workspace(&owner.0, &id);
    s.webhooks.cancel_scope(&owner.0, Some(&id), None);
    s.protocol_sessions.close_workspace(&owner.0, &id).await;
    Ok(Json(serde_json::json!({"ok":true})))
}
pub async fn versions(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
) -> Result<Json<Vec<Workspace>>, ApiError> {
    owned(&s, &owner.0, &id).await?;
    let mut versions: Vec<Workspace> =
        storage::documents(&s.db, &owner.0, "version", Some(&id), 10000).await?;
    versions.sort_by_key(|w| std::cmp::Reverse(w.revision));
    Ok(Json(versions))
}

pub(crate) async fn reconcile_sessions(s: &AppState, owner: &str, w: &Workspace) {
    let ids: Vec<_> = w
        .data
        .collections
        .iter()
        .flat_map(|c| c.requests.iter().map(|r| r.id.clone()))
        .collect();
    s.protocol_sessions
        .reconcile_workspace(owner, &w.id, &ids)
        .await;
}
