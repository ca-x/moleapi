use crate::{ApiError, AppState, auth::Identity, entities::document, workspaces};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
};
use futures_util::TryStreamExt;
use moleapi_core::{RequestSpec, Workspace};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct PageQuery {
    before: Option<i64>,
}

#[derive(Deserialize)]
pub struct RestoreRequest {
    expected_revision: i64,
    destination_collection_id: Option<String>,
}

#[derive(Clone, PartialEq)]
struct RequestState {
    collection_id: Option<String>,
    request: Option<RequestSpec>,
}

#[derive(Serialize)]
pub struct VersionSummary {
    revision: i64,
    updated_at: String,
    collection_id: Option<String>,
    name: Option<String>,
    method: Option<String>,
    exists: bool,
}

#[derive(Serialize)]
pub struct VersionPage {
    versions: Vec<VersionSummary>,
    next_before: Option<i64>,
}

#[derive(Serialize)]
pub struct VersionDetail {
    revision: i64,
    updated_at: String,
    collection_id: Option<String>,
    request: Option<RequestSpec>,
}

fn state_at(workspace: &Workspace, request_id: &str) -> RequestState {
    for collection in &workspace.data.collections {
        if let Some(request) = collection
            .requests
            .iter()
            .find(|request| request.id == request_id)
        {
            return RequestState {
                collection_id: Some(collection.id.clone()),
                request: Some(request.clone()),
            };
        }
    }
    RequestState {
        collection_id: None,
        request: None,
    }
}

fn parse_snapshot(row: document::Model, workspace_id: &str) -> Result<Workspace, ApiError> {
    let snapshot: Workspace =
        serde_json::from_str(&row.payload).map_err(|_| ApiError::internal())?;
    if snapshot.id != workspace_id || snapshot.revision != row.revision {
        return Err(ApiError::internal());
    }
    Ok(snapshot)
}

async fn archived(
    state: &AppState,
    owner: &str,
    workspace_id: &str,
    revision: i64,
) -> Result<document::Model, ApiError> {
    if revision < 1 {
        return Err(ApiError::bad("Invalid workspace revision"));
    }
    document::Entity::find()
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq("version"))
        .filter(document::Column::RefId.eq(workspace_id))
        .filter(document::Column::Revision.eq(revision))
        .one(&state.db)
        .await?
        .ok_or_else(ApiError::not_found)
}

pub async fn list(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path((workspace_id, request_id)): Path<(String, String)>,
    Query(query): Query<PageQuery>,
) -> Result<Json<VersionPage>, ApiError> {
    let current = workspaces::owned(&state, &owner.0, &workspace_id).await?;
    let current_request = state_at(&current, &request_id);
    drop(current);
    let mut newer = if let Some(before) = query.before {
        state_at(
            &parse_snapshot(
                archived(&state, &owner.0, &workspace_id, before).await?,
                &workspace_id,
            )?,
            &request_id,
        )
    } else {
        current_request
    };
    let mut rows = document::Entity::find()
        .filter(document::Column::Owner.eq(&owner.0))
        .filter(document::Column::Kind.eq("version"))
        .filter(document::Column::RefId.eq(&workspace_id));
    if let Some(before) = query.before {
        rows = rows.filter(document::Column::Revision.lt(before));
    }
    let mut stream = rows
        .order_by_desc(document::Column::Revision)
        .limit(32)
        .stream(&state.db)
        .await?;
    let mut versions = Vec::new();
    let mut last_revision = None;
    while let Some(row) = stream.try_next().await? {
        let revision = row.revision;
        let snapshot = parse_snapshot(row, &workspace_id)?;
        let prior = state_at(&snapshot, &request_id);
        last_revision = Some(revision);
        if prior != newer {
            versions.push(VersionSummary {
                revision,
                updated_at: snapshot.updated_at,
                collection_id: prior.collection_id.clone(),
                name: prior.request.as_ref().map(|request| request.name.clone()),
                method: prior.request.as_ref().map(|request| request.method.clone()),
                exists: prior.request.is_some(),
            });
        }
        newer = prior;
        if versions.len() == 20 {
            break;
        }
    }
    drop(stream);
    let next_before = if let Some(last) = last_revision {
        document::Entity::find()
            .select_only()
            .column(document::Column::Revision)
            .filter(document::Column::Owner.eq(&owner.0))
            .filter(document::Column::Kind.eq("version"))
            .filter(document::Column::RefId.eq(&workspace_id))
            .filter(document::Column::Revision.lt(last))
            .into_tuple::<i64>()
            .one(&state.db)
            .await?
            .map(|_| last)
    } else {
        None
    };
    Ok(Json(VersionPage {
        versions,
        next_before,
    }))
}

pub async fn detail(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path((workspace_id, request_id, revision)): Path<(String, String, i64)>,
) -> Result<Json<VersionDetail>, ApiError> {
    workspaces::owned(&state, &owner.0, &workspace_id).await?;
    let row = archived(&state, &owner.0, &workspace_id, revision).await?;
    let snapshot = parse_snapshot(row, &workspace_id)?;
    let request_state = state_at(&snapshot, &request_id);
    Ok(Json(VersionDetail {
        revision,
        updated_at: snapshot.updated_at,
        collection_id: request_state.collection_id,
        request: request_state.request,
    }))
}

pub async fn restore(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path((workspace_id, request_id, revision)): Path<(String, String, i64)>,
    Json(input): Json<RestoreRequest>,
) -> Result<Json<Workspace>, ApiError> {
    if !(1..i64::MAX).contains(&input.expected_revision) {
        return Err(ApiError::bad("Invalid workspace revision"));
    }
    let mut current = workspaces::owned(&state, &owner.0, &workspace_id).await?;
    let row = archived(&state, &owner.0, &workspace_id, revision).await?;
    let snapshot = parse_snapshot(row, &workspace_id)?;
    let target = state_at(&snapshot, &request_id);
    let existing = state_at(&current, &request_id);
    if existing.request == target.request {
        return Err(ApiError::conflict("Request already matches this version"));
    }
    if existing.request.is_some() {
        if input.destination_collection_id.is_some() {
            return Err(ApiError::bad(
                "Destination collection is only valid for a deleted request",
            ));
        }
        for collection in &mut current.data.collections {
            if let Some(position) = collection
                .requests
                .iter()
                .position(|request| request.id == request_id)
            {
                if let Some(request) = target.request {
                    collection.requests[position] = request;
                } else {
                    collection.requests.remove(position);
                }
                break;
            }
        }
    } else {
        let Some(request) = target.request else {
            return Err(ApiError::conflict("Request already matches this version"));
        };
        let historical = target.collection_id.filter(|historical| {
            current
                .data
                .collections
                .iter()
                .any(|collection| collection.id == *historical)
        });
        let destination = if let Some(historical) = historical {
            if input
                .destination_collection_id
                .as_ref()
                .is_some_and(|selected| selected != &historical)
            {
                return Err(ApiError::bad("Historical collection still exists"));
            }
            historical
        } else {
            input
                .destination_collection_id
                .ok_or_else(|| ApiError::bad("Destination collection is required"))?
        };
        let collection = current
            .data
            .collections
            .iter_mut()
            .find(|collection| collection.id == destination)
            .ok_or_else(|| ApiError::bad("Destination collection does not exist"))?;
        collection.requests.push(request);
    }
    workspaces::validate(&current.name, &current.data)?;
    if !state.local {
        moleapi_core::scrub_local_values(&mut current.data);
    }
    let restored =
        workspaces::replace_for_restore(&state, &owner.0, current, input.expected_revision).await?;
    workspaces::reconcile_sessions(&state, &owner.0, &restored).await;
    Ok(Json(restored))
}
