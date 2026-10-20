use crate::{
    ApiError, AppState,
    auth::{Identity, SessionAuthentication},
    entities::{
        account, document, workspace_audit_event, workspace_invitation, workspace_membership,
    },
    storage,
};
use axum::{
    Extension, Json,
    extract::{OriginalUri, Path, Query, Request, State},
    http::HeaderMap,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use moleapi_core::{Workspace, WorkspaceData};
use moleapi_protocols::{EventBatch, SendMessage, SessionSummary};
use rand::RngCore;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait, sea_query::Expr,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::future::Future;

mod dubbo;
pub use dubbo::{shared_dubbo_cancel, shared_dubbo_metadata_import, shared_dubbo_metadata_preview};
mod a2a;
pub use a2a::{shared_a2a_cancel, shared_a2a_discover, shared_a2a_import, shared_a2a_read};
mod mcp;
pub use mcp::shared_mcp_config_export;
mod soap;
pub use soap::{shared_soap_import, shared_soap_import_url, shared_soap_schema};
mod grpc;
pub use grpc::{shared_grpc_import, shared_grpc_reflect, shared_grpc_schema};
mod graphql;
pub use graphql::{shared_graphql_introspect, shared_graphql_schema};

const MAX_MEMBERS: u64 = 100;
const MAX_PENDING: u64 = 20;
const TOKEN_PREFIX: &str = "wi_";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Role {
    Owner,
    Admin,
    Editor,
    Viewer,
}
impl Role {
    fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Admin => "admin",
            Self::Editor => "editor",
            Self::Viewer => "viewer",
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateInvitation {
    role: Role,
    expires_in_days: u8,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptInvitation {
    token: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateRole {
    role: Role,
    expected_revision: i64,
}
#[derive(Deserialize)]
pub struct SharedUpdate {
    name: String,
    data: WorkspaceData,
    expected_revision: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SharedSessionCreate {
    request_id: String,
    environment_id: Option<String>,
    expected_revision: i64,
}
#[derive(Deserialize)]
pub(crate) struct SessionCursor {
    #[serde(default)]
    after: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Access {
    role: Role,
    revision: i64,
    generation: uuid::Uuid,
}

#[derive(Clone)]
pub(crate) struct MemberLive {
    owner: String,
    workspace: String,
    actor: String,
    admission: Access,
    revision: i64,
}
impl MemberLive {
    pub(crate) async fn insert_history(
        &self,
        state: &AppState,
        entry: &moleapi_core::HistoryEntry,
    ) -> Result<(), ApiError> {
        let tx = state.db.begin().await?;
        // Role changes and shared workspace writes take this same canonical row lock.
        let canonical = lock_workspace(&tx, &self.owner, &self.workspace).await?;
        let (_, current) = access(&tx, &self.actor, &self.owner, &self.workspace).await?;
        if current != self.admission
            || current.role == Role::Viewer
            || canonical.revision != self.revision
            || entry.workspace_id != self.workspace
        {
            return Err(ApiError::not_found());
        }
        crate::history::insert_in(&tx, &self.owner, entry).await?;
        tx.commit().await?;
        Ok(())
    }

    pub(crate) async fn check(&self, state: &AppState, actor: &str) -> Result<(), ApiError> {
        if actor != self.actor {
            return Err(ApiError::not_found());
        }
        let (workspace, current) = access(&state.db, actor, &self.owner, &self.workspace).await?;
        if current != self.admission || current.role == Role::Viewer {
            return Err(ApiError::not_found());
        }
        if workspace.revision != self.revision {
            return Err(ApiError::conflict(
                "Workspace changed during shared session",
            ));
        }
        Ok(())
    }
}

fn hosted_session(
    state: &AppState,
    session: Option<Extension<SessionAuthentication>>,
) -> Result<(), ApiError> {
    if state.local || session.is_none() {
        return Err(ApiError::forbidden(
            "Workspace membership requires a hosted login session",
        ));
    }
    Ok(())
}

async fn lock_workspace<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    workspace: &str,
) -> Result<moleapi_core::Workspace, ApiError> {
    document::Entity::update_many()
        .col_expr(
            document::Column::Revision,
            Expr::col(document::Column::Revision).into(),
        )
        .filter(document::Column::Id.eq(storage::workspace_key(owner, workspace)))
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq("workspace"))
        .exec(db)
        .await?;
    storage::get(db, owner, workspace)
        .await?
        .ok_or_else(ApiError::not_found)
}

fn membership_id(owner: &str, workspace: &str, member: &str) -> String {
    hex::encode(Sha256::digest(
        format!("{owner}\0{workspace}\0{member}").as_bytes(),
    ))
}

async fn record_event<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    workspace: &str,
    actor: &str,
    action: &str,
    subject: Option<&str>,
) -> Result<(), ApiError> {
    record_event_result(db, owner, workspace, actor, action, subject, "success").await
}

async fn record_event_result<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    workspace: &str,
    actor: &str,
    action: &str,
    subject: Option<&str>,
    result: &str,
) -> Result<(), ApiError> {
    workspace_audit_event::ActiveModel {
        id: Set(uuid::Uuid::new_v4().to_string()),
        owner_id: Set(owner.into()),
        workspace_id: Set(workspace.into()),
        actor_id: Set(actor.into()),
        action: Set(action.into()),
        subject_id: Set(subject.map(str::to_owned)),
        result: Set(result.into()),
        created_at: Set(chrono::Utc::now().timestamp()),
    }
    .insert(db)
    .await?;
    Ok(())
}

async fn require_manager<C: ConnectionTrait>(
    db: &C,
    actor: &str,
    owner: &str,
    workspace: &str,
) -> Result<(), ApiError> {
    if actor == owner {
        return Ok(());
    }
    let membership =
        workspace_membership::Entity::find_by_id(membership_id(owner, workspace, actor))
            .one(db)
            .await?
            .ok_or_else(ApiError::not_found)?;
    if membership.role != "admin" {
        return Err(ApiError::not_found());
    }
    Ok(())
}

async fn access<C: ConnectionTrait>(
    db: &C,
    actor: &str,
    owner: &str,
    workspace: &str,
) -> Result<(Workspace, Access), ApiError> {
    let data = storage::get(db, owner, workspace)
        .await?
        .ok_or_else(ApiError::not_found)?;
    if actor == owner {
        return Ok((
            data,
            Access {
                role: Role::Owner,
                revision: 0,
                generation: uuid::Uuid::nil(),
            },
        ));
    }
    let member = workspace_membership::Entity::find_by_id(membership_id(owner, workspace, actor))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)?;
    let role = match member.role.as_str() {
        "admin" => Role::Admin,
        "editor" => Role::Editor,
        "viewer" => Role::Viewer,
        _ => return Err(ApiError::internal()),
    };
    Ok((
        data,
        Access {
            role,
            revision: member.revision,
            generation: uuid::Uuid::parse_str(&member.generation)
                .map_err(|_| ApiError::internal())?,
        },
    ))
}

async fn with_access_watch<T>(
    operation: impl Future<Output = Result<T, ApiError>>,
    watch: impl Future<Output = ApiError>,
    final_check: impl Future<Output = Result<(), ApiError>>,
) -> Result<T, ApiError> {
    let result = {
        // Both futures must drop before final_check can acquire a pooled connection.
        tokio::pin!(operation);
        tokio::pin!(watch);
        tokio::select! { biased;
            error = &mut watch => return Err(error),
            result = &mut operation => result,
        }
    };
    final_check.await?;
    result
}

async fn watch_access(
    state: &AppState,
    actor: &str,
    owner: &str,
    workspace: &str,
    admission: Access,
) {
    let period = std::time::Duration::from_millis(200);
    // Fast operations use the final check; a first-tick DB read would contend
    // with their writes on SQLite's single pooled connection.
    let mut check = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
    check.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        check.tick().await;
        let current = access(&state.db, actor, owner, workspace).await;
        if !matches!(current, Ok((_, current)) if current == admission) {
            break;
        }
    }
}

async fn watch_access_revision(
    state: &AppState,
    actor: &str,
    owner: &str,
    workspace: &str,
    admission: Access,
    revision: i64,
    revision_error: &'static str,
) -> ApiError {
    let period = std::time::Duration::from_millis(200);
    let mut check = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
    check.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        check.tick().await;
        match access(&state.db, actor, owner, workspace).await {
            Ok((current_workspace, current)) if current == admission => {
                if current_workspace.revision != revision {
                    return ApiError::conflict(revision_error);
                }
            }
            _ => return ApiError::not_found(),
        }
    }
}

pub async fn shared_get(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    Path((owner, workspace)): Path<(String, String)>,
) -> Result<Json<Workspace>, ApiError> {
    if state.local {
        return Err(ApiError::not_found());
    }
    let (mut data, admission) = access(&state.db, &actor.0, &owner, &workspace).await?;
    if admission.role == Role::Viewer {
        data = viewer_project(data)?;
    }
    let (_, current) = access(&state.db, &actor.0, &owner, &workspace).await?;
    if current != admission {
        return Err(ApiError::not_found());
    }
    Ok(Json(data))
}

fn viewer_project(mut data: Workspace) -> Result<Workspace, ApiError> {
    let safe =
        moleapi_formats::export(&data, "moleapi", false).map_err(|_| ApiError::internal())?;
    let projected: Workspace =
        serde_json::from_str(&safe.content).map_err(|_| ApiError::internal())?;
    moleapi_core::validate_workspace(&projected.data).map_err(|_| ApiError::internal())?;
    data.name = projected.name;
    data.data = projected.data;
    Ok(data)
}

pub async fn shared_update(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    Path((owner, workspace)): Path<(String, String)>,
    Json(mut input): Json<SharedUpdate>,
) -> Result<Json<Workspace>, ApiError> {
    if state.local {
        return Err(ApiError::not_found());
    }
    let (_, preliminary) = access(&state.db, &actor.0, &owner, &workspace).await?;
    if preliminary.role == Role::Viewer {
        return Err(ApiError::forbidden("Viewer cannot edit this workspace"));
    }
    crate::workspaces::validate(&input.name, &input.data)?;
    moleapi_core::scrub_local_values(&mut input.data);
    if !(1..i64::MAX).contains(&input.expected_revision) {
        return Err(ApiError::bad("Invalid expected revision"));
    }
    let tx = state.db.begin().await?;
    lock_workspace(&tx, &owner, &workspace).await?;
    let (mut current, admission) = access(&tx, &actor.0, &owner, &workspace).await?;
    if admission.role == Role::Viewer {
        return Err(ApiError::forbidden("Viewer cannot edit this workspace"));
    }
    current.name = input.name;
    current.data = input.data;
    let updated = storage::replace_in(&tx, &owner, current, input.expected_revision)
        .await?
        .ok_or_else(|| ApiError::conflict("Workspace revision changed"))?;
    record_event(&tx, &owner, &workspace, &actor.0, "workspace.update", None).await?;
    tx.commit().await?;
    crate::workspaces::reconcile_sessions(&state, &owner, &updated).await;
    let (_, current) = access(&state.db, &actor.0, &owner, &workspace).await?;
    if current != admission {
        return Err(ApiError::not_found());
    }
    Ok(Json(updated))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SharedRunCancel {
    job_id: String,
}

fn member_run_job_id(
    owner: &str,
    workspace: &str,
    actor: &str,
    admission: Access,
    id: &str,
) -> Result<String, ApiError> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(ApiError::bad("Invalid run job ID"));
    }
    let identity = serde_json::to_vec(&(
        owner,
        workspace,
        actor,
        admission.generation,
        admission.revision,
        id,
    ))
    .map_err(|_| ApiError::internal())?;
    Ok(format!("shared-{}", hex::encode(Sha256::digest(identity))))
}

pub async fn shared_run_cancel(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    Path((owner, workspace)): Path<(String, String)>,
    Json(input): Json<SharedRunCancel>,
) -> Result<Json<Value>, ApiError> {
    if state.local {
        return Err(ApiError::not_found());
    }
    let tx = state.db.begin().await?;
    lock_workspace(&tx, &owner, &workspace).await?;
    let (_, admission) = access(&tx, &actor.0, &owner, &workspace).await?;
    if admission.role == Role::Viewer {
        return Err(ApiError::not_found());
    }
    let internal_id = member_run_job_id(&owner, &workspace, &actor.0, admission, &input.job_id)?;
    record_event_result(
        &tx,
        &owner,
        &workspace,
        &actor.0,
        "workspace.run.cancel",
        Some(&input.job_id),
        "requested",
    )
    .await?;
    tx.commit().await?;
    state
        .project_jobs
        .cancel_job(&owner, Some(&workspace), &internal_id)?;
    Ok(Json(json!({"cancelled":true})))
}

pub async fn shared_run(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    Path((owner, workspace)): Path<(String, String)>,
    Json(mut input): Json<crate::runner::Run>,
) -> Result<Json<Value>, ApiError> {
    if state.local {
        return Err(ApiError::not_found());
    }
    let job_id = input.ensure_job_id().to_owned();
    let tx = state.db.begin().await?;
    lock_workspace(&tx, &owner, &workspace).await?;
    let (_, admission) = access(&tx, &actor.0, &owner, &workspace).await?;
    if admission.role == Role::Viewer {
        return Err(ApiError::forbidden("Viewer cannot run this workspace"));
    }
    let internal_id = member_run_job_id(&owner, &workspace, &actor.0, admission, &job_id)?;
    record_event_result(
        &tx,
        &owner,
        &workspace,
        &actor.0,
        "workspace.run",
        Some(&job_id),
        "started",
    )
    .await?;
    tx.commit().await?;
    input.set_job_id(internal_id.clone());
    let run = crate::runner::run(
        State(state.clone()),
        Extension(Identity(owner.clone())),
        Path(workspace.clone()),
        Json(input),
    );
    let watch = async {
        watch_access(&state, &actor.0, &owner, &workspace, admission).await;
        let _ = state
            .project_jobs
            .cancel_job(&owner, Some(&workspace), &internal_id);
        ApiError::not_found()
    };
    let final_check = async {
        let (_, current) = access(&state.db, &actor.0, &owner, &workspace).await?;
        if current != admission {
            return Err(ApiError::not_found());
        }
        Ok(())
    };
    let mut result = with_access_watch(run, watch, final_check).await?;
    result.0["job_id"] = json!(job_id);
    Ok(result)
}

async fn watch_authenticated_revision(
    state: &AppState,
    actor: &str,
    owner: &str,
    workspace: &str,
    admission: Access,
    revision: i64,
    headers: &HeaderMap,
) -> ApiError {
    let period = std::time::Duration::from_millis(200);
    let mut ticks = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        ticks.tick().await;
        if let Err(error) = crate::auth::still_authenticated(state, actor, headers).await {
            return error;
        }
        match access(&state.db, actor, owner, workspace).await {
            Ok((canonical, current)) if current == admission => {
                if canonical.revision != revision {
                    return ApiError::conflict("Workspace changed during shared operation");
                }
            }
            _ => return ApiError::not_found(),
        }
    }
}

pub async fn shared_execute(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    headers: HeaderMap,
    Path((owner, workspace, request_id)): Path<(String, String, String)>,
    Json(input): Json<crate::execution::ExecuteSaved>,
) -> Result<Json<moleapi_core::Response>, ApiError> {
    if state.local {
        return Err(ApiError::not_found());
    }
    let tx = state.db.begin().await?;
    let canonical = lock_workspace(&tx, &owner, &workspace).await?;
    let (_, admission) = access(&tx, &actor.0, &owner, &workspace).await?;
    if admission.role == Role::Viewer {
        return Err(ApiError::forbidden("Viewer cannot execute this workspace"));
    }
    let request = canonical
        .data
        .collections
        .iter()
        .flat_map(|collection| &collection.requests)
        .find(|request| request.id == request_id)
        .cloned()
        .ok_or_else(ApiError::not_found)?;
    let protected = request.protocol.is_soap() || request.protocol.is_dubbo();
    let dubbo = request.protocol.is_dubbo();
    if protected && input.has_scope_overrides() || dubbo && input.has_environment_override() {
        return Err(ApiError::bad(if dubbo {
            "Shared Dubbo uses canonical workspace scopes"
        } else {
            "Shared SOAP uses canonical workspace scopes"
        }));
    }
    record_event_result(
        &tx,
        &owner,
        &workspace,
        &actor.0,
        "request.execute",
        Some(&request_id),
        "started",
    )
    .await?;
    tx.commit().await?;
    let lease = if dubbo {
        dubbo::execution_lease(
            &state,
            &actor.0,
            &owner,
            &workspace,
            &request_id,
            admission,
            input.job_id(),
        )?
    } else {
        None
    };
    let run = crate::execution::execute(
        State(state.clone()),
        Extension(Identity(owner.clone())),
        Json(input.into_execute(workspace.clone(), request, canonical.revision)),
    );
    let watch = async {
        if protected {
            return watch_authenticated_revision(
                &state,
                &actor.0,
                &owner,
                &workspace,
                admission,
                canonical.revision,
                &headers,
            )
            .await;
        }
        watch_access(&state, &actor.0, &owner, &workspace, admission).await;
        ApiError::not_found()
    };
    let final_check = async {
        if protected {
            return finish_specification_preview(
                &state.db,
                &actor.0,
                &owner,
                &workspace,
                (admission, canonical.revision),
                crate::auth::still_authenticated(&state, &actor.0, &headers),
            )
            .await;
        }
        let (_, current) = access(&state.db, &actor.0, &owner, &workspace).await?;
        if current != admission {
            return Err(ApiError::not_found());
        }
        Ok(())
    };
    dubbo::cancelable(
        lease,
        with_access_watch(run, watch, final_check),
        finish_specification_preview(
            &state.db,
            &actor.0,
            &owner,
            &workspace,
            (admission, canonical.revision),
            crate::auth::still_authenticated(&state, &actor.0, &headers),
        ),
    )
    .await
}

pub async fn shared_execute_draft(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    headers: HeaderMap,
    Path((owner, workspace)): Path<(String, String)>,
    Json(input): Json<crate::execution::ExecuteDraft>,
) -> Result<Json<moleapi_core::Response>, ApiError> {
    if state.local {
        return Err(ApiError::not_found());
    }
    let tx = state.db.begin().await?;
    let canonical = lock_workspace(&tx, &owner, &workspace).await?;
    let (_, admission) = access(&tx, &actor.0, &owner, &workspace).await?;
    if admission.role == Role::Viewer {
        return Err(ApiError::forbidden("Viewer cannot execute this workspace"));
    }
    if canonical.revision != input.expected_workspace_revision {
        return Err(ApiError::conflict("Workspace revision changed"));
    }
    if input.request.protocol != moleapi_core::Protocol::Http
        && !input.request.protocol.is_graphql()
        && !input.request.protocol.is_soap()
        && !input.request.protocol.is_dubbo()
    {
        return Err(ApiError::bad(
            "Draft execution supports HTTP, GraphQL, SOAP and Dubbo requests only",
        ));
    }
    let dubbo = input.request.protocol.is_dubbo();
    if (input.request.protocol.is_soap() || dubbo) && input.has_scope_overrides()
        || dubbo && input.has_environment_override()
    {
        return Err(ApiError::bad(if dubbo {
            "Shared Dubbo uses canonical workspace scopes"
        } else {
            "Shared SOAP uses canonical workspace scopes"
        }));
    }
    if input.request.protocol.is_graphql()
        && moleapi_core::graphql_is_subscription(&input.request)
            .map_err(|error| ApiError::bad(error.to_string()))?
    {
        return Err(ApiError::bad(
            "GraphQL subscriptions require a saved member session",
        ));
    }
    if input.request.id.trim().is_empty() {
        return Err(ApiError::bad("Draft request ID is required"));
    }
    if !canonical
        .data
        .collections
        .iter()
        .any(|collection| collection.id == input.collection_id)
        || canonical.data.collections.iter().any(|collection| {
            collection.id != input.collection_id
                && collection
                    .requests
                    .iter()
                    .any(|request| request.id == input.request.id)
        })
    {
        return Err(ApiError::not_found());
    }
    if dubbo
        && !canonical.data.collections.iter().any(|c| {
            c.id == input.collection_id
                && c.requests
                    .iter()
                    .any(|r| r.id == input.request.id && r.protocol.is_dubbo())
        })
    {
        return Err(ApiError::not_found());
    }
    moleapi_core::validate_request(&input.request, true)
        .map_err(|error| ApiError::bad(error.to_string()))?;
    record_event_result(
        &tx,
        &owner,
        &workspace,
        &actor.0,
        "request.draft.execute",
        Some(&input.request.id),
        "started",
    )
    .await?;
    tx.commit().await?;
    let lease = if dubbo {
        dubbo::execution_lease(
            &state,
            &actor.0,
            &owner,
            &workspace,
            &input.request.id,
            admission,
            input.job_id(),
        )?
    } else {
        None
    };
    let run = crate::execution::execute(
        State(state.clone()),
        Extension(Identity(owner.clone())),
        Json(input.into_execute(workspace.clone())),
    );
    let watch = watch_authenticated_revision(
        &state,
        &actor.0,
        &owner,
        &workspace,
        admission,
        canonical.revision,
        &headers,
    );
    let final_check = finish_specification_preview(
        &state.db,
        &actor.0,
        &owner,
        &workspace,
        (admission, canonical.revision),
        crate::auth::still_authenticated(&state, &actor.0, &headers),
    );
    dubbo::cancelable(
        lease,
        with_access_watch(run, watch, final_check),
        finish_specification_preview(
            &state.db,
            &actor.0,
            &owner,
            &workspace,
            (admission, canonical.revision),
            crate::auth::still_authenticated(&state, &actor.0, &headers),
        ),
    )
    .await
}

pub async fn shared_session_create(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    headers: HeaderMap,
    Path((owner, workspace)): Path<(String, String)>,
    Json(input): Json<SharedSessionCreate>,
) -> Result<Json<SessionSummary>, ApiError> {
    if state.local {
        return Err(ApiError::not_found());
    }
    let tx = state.db.begin().await?;
    let canonical = lock_workspace(&tx, &owner, &workspace).await?;
    let (_, admission) = access(&tx, &actor.0, &owner, &workspace).await?;
    if admission.role == Role::Viewer {
        return Err(ApiError::forbidden(
            "Viewer cannot connect to this workspace",
        ));
    }
    if canonical.revision != input.expected_revision {
        return Err(ApiError::conflict("Workspace revision changed"));
    }
    let request = canonical
        .data
        .collections
        .iter()
        .flat_map(|collection| &collection.requests)
        .find(|request| request.id == input.request_id)
        .cloned()
        .ok_or_else(ApiError::not_found)?;
    if !matches!(
        request.protocol,
        moleapi_core::Protocol::Sse
            | moleapi_core::Protocol::Websocket
            | moleapi_core::Protocol::Data { .. }
            | moleapi_core::Protocol::Graphql { .. }
            | moleapi_core::Protocol::Tcp { .. }
            | moleapi_core::Protocol::Socketio { .. }
            | moleapi_core::Protocol::Grpc { .. }
            | moleapi_core::Protocol::Mqtt { .. }
            | moleapi_core::Protocol::Mcp { .. }
            | moleapi_core::Protocol::A2a { .. }
    ) {
        return Err(ApiError::bad(
            "Shared live sessions support saved SSE, WebSocket, Data, TCP, Socket.IO, gRPC, MQTT, MCP, A2A and GraphQL subscriptions",
        ));
    }
    if request.protocol.is_graphql()
        && !moleapi_core::graphql_is_subscription(&request)
            .map_err(|error| ApiError::bad(error.to_string()))?
    {
        return Err(ApiError::bad(
            "GraphQL session requires a saved subscription",
        ));
    }
    tx.commit().await?;
    let member = MemberLive {
        owner: owner.clone(),
        workspace: workspace.clone(),
        actor: actor.0.clone(),
        admission,
        revision: canonical.revision,
    };
    let (summary, mut guard) = crate::protocols::create_as(
        state.clone(),
        Identity(owner.clone()),
        &actor.0,
        headers.clone(),
        crate::protocols::Create {
            workspace_id: workspace,
            request,
            environment_id: input.environment_id,
            locals: Vec::new(),
        },
        Some(&member),
    )
    .await?;
    member.check(&state, &actor.0).await?;
    let audit = state.db.begin().await?;
    record_event_result(
        &audit,
        &owner,
        &member.workspace,
        &actor.0,
        "protocol.session.start",
        Some(&input.request_id),
        "started",
    )
    .await?;
    audit.commit().await?;
    member.check(&state, &actor.0).await?;
    crate::auth::still_authenticated(&state, &actor.0, &headers).await?;
    {
        let mut sessions = state.member_protocol_sessions.lock().unwrap();
        sessions.retain(|id, entry| state.protocol_sessions.summary(&entry.owner, id).is_ok());
        sessions.insert(summary.id.clone(), member.clone());
    }
    let session_id = summary.id.clone();
    tokio::spawn(async move {
        let period = std::time::Duration::from_millis(200);
        let mut ticks = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
        ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticks.tick().await;
            if member.check(&state, &member.actor).await.is_err()
                || crate::auth::still_authenticated(&state, &member.actor, &headers)
                    .await
                    .is_err()
            {
                state
                    .member_protocol_sessions
                    .lock()
                    .unwrap()
                    .remove(&session_id);
                let _ = state
                    .protocol_sessions
                    .remove(&member.owner, &session_id)
                    .await;
                break;
            }
            match state.protocol_sessions.summary(&member.owner, &session_id) {
                Ok(summary) if summary.state.live() => {}
                _ => {
                    // Match the protocol manager's ten-minute terminal retention.
                    tokio::time::sleep(std::time::Duration::from_secs(10 * 60 + 30)).await;
                    state
                        .member_protocol_sessions
                        .lock()
                        .unwrap()
                        .remove(&session_id);
                    break;
                }
            }
        }
    });
    guard.disarm();
    Ok(Json(summary))
}

async fn member_session(
    state: &AppState,
    actor: &str,
    owner: &str,
    workspace: &str,
    id: &str,
) -> Result<MemberLive, ApiError> {
    let member = state
        .member_protocol_sessions
        .lock()
        .unwrap()
        .get(id)
        .cloned()
        .ok_or_else(ApiError::not_found)?;
    if member.owner != owner || member.workspace != workspace {
        return Err(ApiError::not_found());
    }
    member.check(state, actor).await?;
    let summary = state
        .protocol_sessions
        .summary(owner, id)
        .map_err(crate::protocols::error)?;
    if summary.workspace_id != workspace {
        return Err(ApiError::not_found());
    }
    Ok(member)
}

pub async fn shared_session_get(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    Path((owner, workspace, id)): Path<(String, String, String)>,
) -> Result<Json<SessionSummary>, ApiError> {
    let member = member_session(&state, &actor.0, &owner, &workspace, &id).await?;
    let summary = state
        .protocol_sessions
        .summary(&owner, &id)
        .map_err(crate::protocols::error)?;
    member.check(&state, &actor.0).await?;
    Ok(Json(summary))
}

pub async fn shared_session_events(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    Path((owner, workspace, id)): Path<(String, String, String)>,
    Query(cursor): Query<SessionCursor>,
) -> Result<Json<EventBatch>, ApiError> {
    let member = member_session(&state, &actor.0, &owner, &workspace, &id).await?;
    let batch = state
        .protocol_sessions
        .events(&owner, &id, cursor.after)
        .map_err(crate::protocols::error)?;
    member.check(&state, &actor.0).await?;
    Ok(Json(batch))
}

pub async fn shared_session_send(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    Path((owner, workspace, id)): Path<(String, String, String)>,
    Json(message): Json<SendMessage>,
) -> Result<Json<Value>, ApiError> {
    let member = member_session(&state, &actor.0, &owner, &workspace, &id).await?;
    let summary = state
        .protocol_sessions
        .summary(&owner, &id)
        .map_err(crate::protocols::error)?;
    let supported = match summary.protocol.as_str() {
        "websocket" => matches!(
            message,
            SendMessage::Text { .. } | SendMessage::Binary { .. } | SendMessage::Ping { .. }
        ),
        "a2a" => matches!(
            message,
            SendMessage::A2aRequest { .. } | SendMessage::A2aStop { .. }
        ),
        "mcp" => matches!(
            message,
            SendMessage::McpRequest { .. }
                | SendMessage::McpCallback { .. }
                | SendMessage::McpCancel { .. }
        ),
        "mqtt" => matches!(
            message,
            SendMessage::MqttPublish { .. }
                | SendMessage::MqttSubscribe { .. }
                | SendMessage::MqttUnsubscribe { .. }
                | SendMessage::MqttAbort
        ),
        "grpc" => matches!(
            message,
            SendMessage::GrpcMessage { .. } | SendMessage::GrpcHalfClose
        ),
        "socketio" => matches!(
            message,
            SendMessage::SocketioEmit { .. }
                | SendMessage::SocketioListen { .. }
                | SendMessage::SocketioAck { .. }
        ),
        "tcp" => matches!(
            message,
            SendMessage::TcpSend { .. } | SendMessage::TcpHalfClose
        ),
        "data" => matches!(
            message,
            SendMessage::DataQuery { .. }
                | SendMessage::DataCancel { .. }
                | SendMessage::DataSchemaRefresh
        ),
        _ => false,
    };
    if !supported {
        return Err(ApiError::bad(
            "Message is unavailable for this shared session protocol",
        ));
    }
    if matches!(
        summary.protocol.as_str(),
        "data" | "tcp" | "socketio" | "grpc" | "mqtt" | "mcp" | "a2a"
    ) {
        let action = match &message {
            SendMessage::DataQuery { .. } => "protocol.data.query",
            SendMessage::DataCancel { .. } => "protocol.data.cancel",
            SendMessage::TcpSend { .. } => "protocol.tcp.send",
            SendMessage::TcpHalfClose => "protocol.tcp.half_close",
            SendMessage::SocketioEmit { .. } => "protocol.socketio.emit",
            SendMessage::SocketioListen { .. } => "protocol.socketio.listen",
            SendMessage::SocketioAck { .. } => "protocol.socketio.ack",
            SendMessage::GrpcMessage { .. } => "protocol.grpc.message",
            SendMessage::GrpcHalfClose => "protocol.grpc.half_close",
            SendMessage::MqttPublish { .. } => "protocol.mqtt.publish",
            SendMessage::MqttSubscribe { .. } => "protocol.mqtt.subscribe",
            SendMessage::MqttUnsubscribe { .. } => "protocol.mqtt.unsubscribe",
            SendMessage::MqttAbort => "protocol.mqtt.abort",
            SendMessage::A2aRequest { .. } => "protocol.a2a.request",
            SendMessage::A2aStop { .. } => "protocol.a2a.stop",
            SendMessage::McpRequest { .. } => "protocol.mcp.request",
            SendMessage::McpCallback { .. } => "protocol.mcp.callback",
            SendMessage::McpCancel { .. } => "protocol.mcp.cancel",
            _ => "protocol.data.schema",
        };
        let audit = state.db.begin().await?;
        record_event_result(
            &audit,
            &owner,
            &workspace,
            &actor.0,
            action,
            Some(&id),
            "started",
        )
        .await?;
        audit.commit().await?;
        member.check(&state, &actor.0).await?;
    }
    let _ = crate::protocols::send(
        State(state.clone()),
        Extension(Identity(owner.clone())),
        Path(id.clone()),
        Json(message),
    )
    .await?;
    member.check(&state, &actor.0).await?;
    Ok(Json(json!({"ok":true})))
}

pub async fn shared_session_close(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    Path((owner, workspace, id)): Path<(String, String, String)>,
) -> Result<Json<SessionSummary>, ApiError> {
    let member = member_session(&state, &actor.0, &owner, &workspace, &id).await?;
    let summary = state
        .protocol_sessions
        .close(&owner, &id)
        .await
        .map_err(crate::protocols::error)?;
    member.check(&state, &actor.0).await?;
    Ok(Json(summary))
}

pub async fn shared_session_delete(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    Path((owner, workspace, id)): Path<(String, String, String)>,
) -> Result<Json<Value>, ApiError> {
    member_session(&state, &actor.0, &owner, &workspace, &id).await?;
    state
        .protocol_sessions
        .remove(&owner, &id)
        .await
        .map_err(crate::protocols::error)?;
    state.member_protocol_sessions.lock().unwrap().remove(&id);
    Ok(Json(json!({"ok":true})))
}

pub async fn shared_mock(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    Path((owner, workspace, request_id, example_id)): Path<(String, String, String, String)>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Result<axum::response::Response, ApiError> {
    if state.local {
        return Err(ApiError::not_found());
    }
    let tx = state.db.begin().await?;
    let canonical = lock_workspace(&tx, &owner, &workspace).await?;
    let (_, admission) = access(&tx, &actor.0, &owner, &workspace).await?;
    if admission.role == Role::Viewer {
        return Err(ApiError::forbidden("Viewer cannot preview Mock responses"));
    }
    let exists = canonical
        .data
        .collections
        .iter()
        .flat_map(|collection| &collection.requests)
        .any(|request| {
            request.id == request_id
                && request
                    .examples
                    .iter()
                    .any(|example| example.id == example_id)
        });
    if !exists {
        return Err(ApiError::not_found());
    }
    record_event_result(
        &tx,
        &owner,
        &workspace,
        &actor.0,
        "mock.preview",
        Some(&request_id),
        "started",
    )
    .await?;
    tx.commit().await?;
    let preview = crate::mock::mock(
        State(state.clone()),
        Extension(Identity(owner.clone())),
        Path((workspace.clone(), request_id, example_id)),
        OriginalUri(uri),
        headers,
    );
    let watch = async {
        watch_access(&state, &actor.0, &owner, &workspace, admission).await;
        ApiError::not_found()
    };
    let final_check = async {
        let (_, current) = access(&state.db, &actor.0, &owner, &workspace).await?;
        if current != admission {
            return Err(ApiError::not_found());
        }
        Ok(())
    };
    let mut response = with_access_watch(preview, watch, final_check).await?;
    response.headers_mut().insert(
        "x-moleapi-mock-preview",
        axum::http::HeaderValue::from_static("saved-example"),
    );
    Ok(response)
}

async fn finish_specification_preview<C: ConnectionTrait>(
    db: &C,
    actor: &str,
    owner: &str,
    workspace: &str,
    expected: (Access, i64),
    credential: impl Future<Output = Result<(), ApiError>>,
) -> Result<(), ApiError> {
    credential.await?;
    let (current_workspace, current) = access(db, actor, owner, workspace).await?;
    if current != expected.0 {
        return Err(ApiError::not_found());
    }
    if current_workspace.revision != expected.1 {
        return Err(ApiError::conflict(
            "Workspace changed during specification preview",
        ));
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SharedSpecificationPreview {
    expected_revision: i64,
    name: String,
    source: String,
    specification_id: Option<String>,
    #[serde(default)]
    include_requests: bool,
}

pub async fn shared_specification_preview(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    headers: HeaderMap,
    Path((owner, workspace)): Path<(String, String)>,
    Json(input): Json<SharedSpecificationPreview>,
) -> Result<Json<moleapi_formats::ImportResult>, ApiError> {
    if state.local {
        return Err(ApiError::not_found());
    }
    let tx = state.db.begin().await?;
    let canonical = lock_workspace(&tx, &owner, &workspace).await?;
    let (_, admission) = access(&tx, &actor.0, &owner, &workspace).await?;
    if admission.role == Role::Viewer {
        return Err(ApiError::forbidden("Viewer cannot import specifications"));
    }
    if canonical.revision != input.expected_revision {
        return Err(ApiError::conflict(
            "Workspace changed before specification preview",
        ));
    }
    if input.name.trim().is_empty()
        || input.name.len() > 256
        || input.name.chars().any(char::is_control)
        || input.source.len() > moleapi_core::MAX_BODY
    {
        return Err(ApiError::bad("Invalid specification name or source size"));
    }
    if let Some(id) = &input.specification_id {
        let existing = canonical
            .data
            .specifications
            .iter()
            .find(|spec| &spec.id == id)
            .ok_or_else(ApiError::not_found)?;
        if existing.kind != "openapi" {
            return Err(ApiError::bad("Source replacement supports OpenAPI only"));
        }
    }
    record_event_result(
        &tx,
        &owner,
        &workspace,
        &actor.0,
        "specification.preview",
        input.specification_id.as_deref(),
        "started",
    )
    .await?;
    tx.commit().await?;
    let revision = canonical.revision;
    let preview = async move {
        tokio::task::spawn_blocking(move || {
            let mut parsed = moleapi_formats::import("openapi", &input.source)
                .map_err(|error| ApiError::bad(error.to_string()))?;
            let mut spec = parsed
                .data
                .specifications
                .pop()
                .ok_or_else(|| ApiError::bad("OpenAPI source is missing"))?;
            spec.name = input.name.trim().to_owned();
            let mut data = canonical.data;
            if let Some(id) = input.specification_id {
                spec.id = id.clone();
                let target = data
                    .specifications
                    .iter_mut()
                    .find(|spec| spec.id == id)
                    .ok_or_else(ApiError::not_found)?;
                *target = spec;
            } else {
                data.specifications.push(spec);
                if input.include_requests {
                    data.collections.extend(parsed.data.collections);
                    data.environments.extend(parsed.data.environments);
                }
            }
            moleapi_core::validate_workspace(&data)
                .map_err(|error| ApiError::bad(error.to_string()))?;
            Ok(Json(moleapi_formats::ImportResult {
                name: canonical.name,
                data,
                warnings: parsed.warnings,
            }))
        })
        .await
        .map_err(|_| ApiError::internal())?
    };
    let watch = watch_access_revision(
        &state,
        &actor.0,
        &owner,
        &workspace,
        admission,
        revision,
        "Workspace changed during specification preview",
    );
    let final_check = finish_specification_preview(
        &state.db,
        &actor.0,
        &owner,
        &workspace,
        (admission, revision),
        crate::auth::still_authenticated(&state, &actor.0, &headers),
    );
    with_access_watch(preview, watch, final_check).await
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SharedDatasetPreview {
    dataset: moleapi_core::DatasetSource,
    expected_revision: i64,
}

pub async fn shared_dataset_preview(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    Path((owner, workspace)): Path<(String, String)>,
    Json(input): Json<SharedDatasetPreview>,
) -> Result<Json<moleapi_core::Dataset>, ApiError> {
    if state.local {
        return Err(ApiError::not_found());
    }
    let tx = state.db.begin().await?;
    let canonical = lock_workspace(&tx, &owner, &workspace).await?;
    let (_, admission) = access(&tx, &actor.0, &owner, &workspace).await?;
    if admission.role == Role::Viewer {
        return Err(ApiError::forbidden("Viewer cannot preview datasets"));
    }
    if canonical.revision != input.expected_revision {
        return Err(ApiError::conflict(
            "Workspace changed before dataset preview",
        ));
    }
    record_event_result(
        &tx,
        &owner,
        &workspace,
        &actor.0,
        "dataset.preview",
        None,
        "started",
    )
    .await?;
    tx.commit().await?;
    let result = crate::runner::preview(
        State(state.clone()),
        Extension(Identity(owner.clone())),
        Json(crate::runner::Preview {
            workspace_id: workspace.clone(),
            dataset: input.dataset,
        }),
    )
    .await;
    let (current_workspace, current) = access(&state.db, &actor.0, &owner, &workspace).await?;
    if current != admission {
        return Err(ApiError::not_found());
    }
    if current_workspace.revision != canonical.revision {
        return Err(ApiError::conflict(
            "Workspace changed during dataset preview",
        ));
    }
    result
}

pub async fn shared_validate_mock_schema(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    Path((owner, workspace)): Path<(String, String)>,
    Json(input): Json<crate::mock::ValidateLinkedSchema>,
) -> Result<Json<Value>, ApiError> {
    if state.local {
        return Err(ApiError::not_found());
    }
    let tx = state.db.begin().await?;
    let canonical = lock_workspace(&tx, &owner, &workspace).await?;
    let (_, admission) = access(&tx, &actor.0, &owner, &workspace).await?;
    if admission.role == Role::Viewer {
        return Err(ApiError::forbidden(
            "Viewer cannot validate linked Mock responses",
        ));
    }
    if canonical.revision != input.expected_revision {
        return Err(ApiError::conflict(
            "Workspace changed before linked Mock validation",
        ));
    }
    record_event_result(
        &tx,
        &owner,
        &workspace,
        &actor.0,
        "mock.schema.validate",
        None,
        "started",
    )
    .await?;
    tx.commit().await?;
    let validate = crate::mock::validate_linked_schema(
        State(state.clone()),
        Extension(Identity(owner.clone())),
        Path(workspace.clone()),
        Json(input),
    );
    let watch = watch_access_revision(
        &state,
        &actor.0,
        &owner,
        &workspace,
        admission,
        canonical.revision,
        "Workspace changed during linked Mock validation",
    );
    let final_check = async {
        let (current_workspace, current) = access(&state.db, &actor.0, &owner, &workspace).await?;
        if current != admission {
            return Err(ApiError::not_found());
        }
        if current_workspace.revision != canonical.revision {
            return Err(ApiError::conflict(
                "Workspace changed during linked Mock validation",
            ));
        }
        Ok(())
    };
    with_access_watch(validate, watch, final_check).await
}

pub async fn shared_mock_dispatch(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    Path((owner, workspace, path)): Path<(String, String, String)>,
    OriginalUri(uri): OriginalUri,
    incoming: Request,
) -> Result<axum::response::Response, ApiError> {
    if state.local {
        return Err(ApiError::not_found());
    }
    let tx = state.db.begin().await?;
    let canonical = lock_workspace(&tx, &owner, &workspace).await?;
    let (_, admission) = access(&tx, &actor.0, &owner, &workspace).await?;
    if admission.role == Role::Viewer {
        return Err(ApiError::forbidden("Viewer cannot route Mock requests"));
    }
    record_event_result(
        &tx,
        &owner,
        &workspace,
        &actor.0,
        "mock.route",
        None,
        "started",
    )
    .await?;
    tx.commit().await?;
    let dispatch = crate::mock::dispatch(
        State(state.clone()),
        Extension(Identity(owner.clone())),
        Path((workspace.clone(), path)),
        OriginalUri(uri),
        incoming,
    );
    let watch = watch_access_revision(
        &state,
        &actor.0,
        &owner,
        &workspace,
        admission,
        canonical.revision,
        "Workspace changed during Mock routing",
    );
    let final_check = async {
        let (current_workspace, current) = access(&state.db, &actor.0, &owner, &workspace).await?;
        if current != admission {
            return Err(ApiError::not_found());
        }
        if current_workspace.revision != canonical.revision {
            return Err(ApiError::conflict("Workspace changed during Mock routing"));
        }
        Ok(())
    };
    with_access_watch(dispatch, watch, final_check).await
}

pub async fn shared_mock_calls(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    Path((owner, workspace)): Path<(String, String)>,
) -> Result<Json<Vec<crate::mock_logs::MockCall>>, ApiError> {
    let read = crate::mock_logs::list(
        State(state.clone()),
        Extension(Identity(owner.clone())),
        Path(workspace.clone()),
    );
    editor_read(&state, &actor.0, &owner, &workspace, read).await
}

async fn editor_read<T>(
    state: &AppState,
    actor: &str,
    owner: &str,
    workspace: &str,
    read: impl Future<Output = Result<Json<T>, ApiError>>,
) -> Result<Json<T>, ApiError> {
    if state.local {
        return Err(ApiError::not_found());
    }
    let (_, admission) = access(&state.db, actor, owner, workspace).await?;
    if admission.role == Role::Viewer {
        return Err(ApiError::not_found());
    }
    let result = read.await?;
    let (_, current) = access(&state.db, actor, owner, workspace).await?;
    if current != admission {
        return Err(ApiError::not_found());
    }
    Ok(result)
}

pub async fn shared_history(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    Path((owner, workspace)): Path<(String, String)>,
) -> Result<Json<Vec<moleapi_core::HistoryEntry>>, ApiError> {
    let read = crate::history::history(
        State(state.clone()),
        Extension(Identity(owner.clone())),
        Path(workspace.clone()),
    );
    editor_read(&state, &actor.0, &owner, &workspace, read).await
}

pub async fn shared_reports(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    Path((owner, workspace)): Path<(String, String)>,
    axum::extract::Query(query): axum::extract::Query<crate::run_reports::List>,
) -> Result<Json<crate::run_reports::Page>, ApiError> {
    let read = crate::run_reports::list(
        State(state.clone()),
        Extension(Identity(owner.clone())),
        Path(workspace.clone()),
        axum::extract::Query(query),
    );
    editor_read(&state, &actor.0, &owner, &workspace, read).await
}

pub async fn shared_report(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    Path((owner, workspace, report)): Path<(String, String, String)>,
) -> Result<Json<moleapi_core::SavedRunReport>, ApiError> {
    let read = crate::run_reports::get(
        State(state.clone()),
        Extension(Identity(owner.clone())),
        Path((workspace.clone(), report)),
    );
    editor_read(&state, &actor.0, &owner, &workspace, read).await
}

pub async fn shared_report_export(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    Path((owner, workspace, report)): Path<(String, String, String)>,
    axum::extract::Query(options): axum::extract::Query<crate::run_reports::Export>,
) -> Result<Json<moleapi_formats::ExportResult>, ApiError> {
    if state.local {
        return Err(ApiError::not_found());
    }
    let (_, admission) = access(&state.db, &actor.0, &owner, &workspace).await?;
    if admission.role == Role::Viewer {
        return Err(ApiError::not_found());
    }
    record_event_result(
        &state.db,
        &owner,
        &workspace,
        &actor.0,
        "report.export",
        Some(&report),
        "started",
    )
    .await?;
    let read = crate::run_reports::export(
        State(state.clone()),
        Extension(Identity(owner.clone())),
        Path((workspace.clone(), report)),
        axum::extract::Query(options),
    );
    editor_read(&state, &actor.0, &owner, &workspace, read).await
}

pub async fn shared_report_response(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    Path((owner, workspace, report, position)): Path<(String, String, String, usize)>,
) -> Result<Json<moleapi_core::HistoryEntry>, ApiError> {
    let read = crate::run_reports::response(
        State(state.clone()),
        Extension(Identity(owner.clone())),
        Path((workspace.clone(), report, position)),
    );
    editor_read(&state, &actor.0, &owner, &workspace, read).await
}

fn invitation_view(invite: &workspace_invitation::Model) -> Value {
    json!({
        "id":invite.id,
        "owner_id":invite.owner_id,
        "workspace_id":invite.workspace_id,
        "role":invite.role,
        "created_by":invite.created_by,
        "created_at":invite.created_at,
        "expires_at":invite.expires_at,
        "consumed_by":invite.consumed_by,
        "revoked":invite.revoked_at.is_some(),
    })
}

pub async fn create_invitation(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    session: Option<Extension<SessionAuthentication>>,
    Path(workspace): Path<String>,
    Json(input): Json<CreateInvitation>,
) -> Result<Json<Value>, ApiError> {
    hosted_session(&state, session)?;
    create_invitation_for(&state, &actor.0, &actor.0, &workspace, input).await
}

pub async fn create_invitation_scoped(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    session: Option<Extension<SessionAuthentication>>,
    Path((owner, workspace)): Path<(String, String)>,
    Json(input): Json<CreateInvitation>,
) -> Result<Json<Value>, ApiError> {
    hosted_session(&state, session)?;
    create_invitation_for(&state, &actor.0, &owner, &workspace, input).await
}

async fn create_invitation_for(
    state: &AppState,
    actor: &str,
    owner: &str,
    workspace: &str,
    input: CreateInvitation,
) -> Result<Json<Value>, ApiError> {
    if input.role == Role::Owner || !(1..=14).contains(&input.expires_in_days) {
        return Err(ApiError::bad("Invalid invitation role or expiry"));
    }
    let tx = state.db.begin().await?;
    lock_workspace(&tx, owner, workspace).await?;
    require_manager(&tx, actor, owner, workspace).await?;
    let now = chrono::Utc::now().timestamp();
    let pending = workspace_invitation::Entity::find()
        .filter(workspace_invitation::Column::OwnerId.eq(owner))
        .filter(workspace_invitation::Column::WorkspaceId.eq(workspace))
        .filter(workspace_invitation::Column::ConsumedBy.is_null())
        .filter(workspace_invitation::Column::RevokedAt.is_null())
        .filter(workspace_invitation::Column::ExpiresAt.gt(now))
        .count(&tx)
        .await?;
    if pending >= MAX_PENDING {
        return Err(ApiError::bad("Workspace invitation limit reached"));
    }
    let mut random = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut random);
    let token = format!("{TOKEN_PREFIX}{}", hex::encode(random));
    let invite = workspace_invitation::ActiveModel {
        id: Set(uuid::Uuid::new_v4().to_string()),
        owner_id: Set(owner.into()),
        workspace_id: Set(workspace.into()),
        token_digest: Set(hex::encode(Sha256::digest(token.as_bytes()))),
        role: Set(input.role.as_str().into()),
        created_by: Set(actor.into()),
        created_at: Set(now),
        expires_at: Set(now + i64::from(input.expires_in_days) * 86400),
        consumed_by: Set(None),
        revoked_at: Set(None),
        revision: Set(1),
    }
    .insert(&tx)
    .await?;
    record_event(
        &tx,
        owner,
        workspace,
        actor,
        "invitation.create",
        Some(&invite.id),
    )
    .await?;
    tx.commit().await?;
    let mut response = invitation_view(&invite);
    response["token"] = json!(token);
    Ok(Json(response))
}

pub async fn list_invitations(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    session: Option<Extension<SessionAuthentication>>,
    Path(workspace): Path<String>,
) -> Result<Json<Vec<Value>>, ApiError> {
    hosted_session(&state, session)?;
    list_invitations_for(&state, &actor.0, &actor.0, &workspace).await
}

pub async fn list_invitations_scoped(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    session: Option<Extension<SessionAuthentication>>,
    Path((owner, workspace)): Path<(String, String)>,
) -> Result<Json<Vec<Value>>, ApiError> {
    hosted_session(&state, session)?;
    list_invitations_for(&state, &actor.0, &owner, &workspace).await
}

async fn list_invitations_for(
    state: &AppState,
    actor: &str,
    owner: &str,
    workspace: &str,
) -> Result<Json<Vec<Value>>, ApiError> {
    storage::get(&state.db, owner, workspace)
        .await?
        .ok_or_else(ApiError::not_found)?;
    require_manager(&state.db, actor, owner, workspace).await?;
    let invites = workspace_invitation::Entity::find()
        .filter(workspace_invitation::Column::OwnerId.eq(owner))
        .filter(workspace_invitation::Column::WorkspaceId.eq(workspace))
        .order_by_desc(workspace_invitation::Column::CreatedAt)
        .limit(256)
        .all(&state.db)
        .await?;
    Ok(Json(invites.iter().map(invitation_view).collect()))
}

pub async fn accept_invitation(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    session: Option<Extension<SessionAuthentication>>,
    Json(input): Json<AcceptInvitation>,
) -> Result<Json<Value>, ApiError> {
    hosted_session(&state, session)?;
    if input.token.len() != TOKEN_PREFIX.len() + 64
        || !input.token.starts_with(TOKEN_PREFIX)
        || !input.token[TOKEN_PREFIX.len()..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(ApiError::not_found());
    }
    let digest = hex::encode(Sha256::digest(input.token.as_bytes()));
    let preliminary = workspace_invitation::Entity::find()
        .filter(workspace_invitation::Column::TokenDigest.eq(&digest))
        .one(&state.db)
        .await?
        .ok_or_else(ApiError::not_found)?;
    if preliminary.owner_id == actor.0 {
        return Err(ApiError::not_found());
    }
    let tx = state.db.begin().await?;
    let workspace = lock_workspace(&tx, &preliminary.owner_id, &preliminary.workspace_id).await?;
    workspace_invitation::Entity::update_many()
        .col_expr(
            workspace_invitation::Column::Revision,
            Expr::col(workspace_invitation::Column::Revision).into(),
        )
        .filter(workspace_invitation::Column::Id.eq(&preliminary.id))
        .exec(&tx)
        .await?;
    let invite = workspace_invitation::Entity::find_by_id(&preliminary.id)
        .one(&tx)
        .await?
        .ok_or_else(ApiError::not_found)?;
    if invite.token_digest != digest
        || invite.revoked_at.is_some()
        || invite.expires_at <= chrono::Utc::now().timestamp()
    {
        return Err(ApiError::not_found());
    }
    let id = membership_id(&invite.owner_id, &invite.workspace_id, &actor.0);
    let existing = workspace_membership::Entity::find_by_id(&id)
        .one(&tx)
        .await?;
    if let Some(consumed) = &invite.consumed_by {
        if consumed != &actor.0 || existing.is_none() {
            return Err(ApiError::not_found());
        }
    } else {
        if existing.is_some() {
            return Err(ApiError::conflict(
                "Account already belongs to this workspace",
            ));
        }
        if existing.is_none() {
            let count = workspace_membership::Entity::find()
                .filter(workspace_membership::Column::OwnerId.eq(&invite.owner_id))
                .filter(workspace_membership::Column::WorkspaceId.eq(&invite.workspace_id))
                .count(&tx)
                .await?;
            if count >= MAX_MEMBERS {
                return Err(ApiError::bad("Workspace member limit reached"));
            }
            workspace_membership::ActiveModel {
                id: Set(id),
                owner_id: Set(invite.owner_id.clone()),
                workspace_id: Set(invite.workspace_id.clone()),
                member_id: Set(actor.0.clone()),
                role: Set(invite.role.clone()),
                generation: Set(uuid::Uuid::new_v4().to_string()),
                revision: Set(1),
                created_at: Set(chrono::Utc::now().timestamp()),
                updated_at: Set(chrono::Utc::now().timestamp()),
            }
            .insert(&tx)
            .await?;
            record_event(
                &tx,
                &invite.owner_id,
                &invite.workspace_id,
                &actor.0,
                "membership.accept",
                Some(&actor.0),
            )
            .await?;
        }
        let mut active: workspace_invitation::ActiveModel = invite.clone().into();
        active.consumed_by = Set(Some(actor.0.clone()));
        active.revision = Set(invite.revision + 1);
        active.update(&tx).await?;
    }
    tx.commit().await?;
    Ok(Json(json!({
        "owner_id":invite.owner_id,
        "workspace_id":invite.workspace_id,
        "workspace_name":workspace.name,
        "member_id":actor.0,
        "role":existing.as_ref().map_or(invite.role, |member| member.role.clone()),
    })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SharedListQuery {
    cursor: Option<String>,
    limit: Option<u64>,
}

#[derive(Serialize)]
pub struct SharedPage {
    items: Vec<Value>,
    next_cursor: Option<String>,
}

pub async fn list_shared(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    Query(query): Query<SharedListQuery>,
) -> Result<Json<SharedPage>, ApiError> {
    if state.local {
        return Ok(Json(SharedPage {
            items: Vec::new(),
            next_cursor: None,
        }));
    }
    let limit = query.limit.unwrap_or(100);
    if !(1..=100).contains(&limit) {
        return Err(ApiError::bad(
            "Shared workspace page requires 1 to 100 items",
        ));
    }
    let mut request = workspace_membership::Entity::find()
        .filter(workspace_membership::Column::MemberId.eq(&actor.0))
        .order_by_desc(workspace_membership::Column::UpdatedAt)
        .order_by_desc(workspace_membership::Column::Id);
    if let Some(cursor) = query.cursor {
        if cursor.len() > 256 {
            return Err(ApiError::bad("Invalid shared workspace cursor"));
        }
        let bytes = URL_SAFE_NO_PAD
            .decode(cursor)
            .map_err(|_| ApiError::bad("Invalid shared workspace cursor"))?;
        let (timestamp, id): (i64, String) = serde_json::from_slice(&bytes)
            .map_err(|_| ApiError::bad("Invalid shared workspace cursor"))?;
        if timestamp < 0 || id.len() != 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(ApiError::bad("Invalid shared workspace cursor"));
        }
        request = request.filter(
            Condition::any()
                .add(workspace_membership::Column::UpdatedAt.lt(timestamp))
                .add(
                    Condition::all()
                        .add(workspace_membership::Column::UpdatedAt.eq(timestamp))
                        .add(workspace_membership::Column::Id.lt(id)),
                ),
        );
    }
    let mut memberships = request.limit(limit + 1).all(&state.db).await?;
    let next_cursor = if memberships.len() > limit as usize {
        memberships.truncate(limit as usize);
        memberships
            .last()
            .map(|member| {
                serde_json::to_vec(&(member.updated_at, &member.id))
                    .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
            })
            .transpose()
            .map_err(|_| ApiError::internal())?
    } else {
        None
    };
    let mut result = Vec::new();
    for membership in memberships {
        if let Some(workspace) =
            storage::get(&state.db, &membership.owner_id, &membership.workspace_id).await?
        {
            let Some(owner_account) = account::Entity::find_by_id(&membership.owner_id)
                .one(&state.db)
                .await?
            else {
                continue;
            };
            let privacy = moleapi_formats::RunReportPrivacy::new(&workspace);
            let workspace_name = privacy.screen(&workspace.name, 256);
            let workspace_name = if workspace_name.trim().is_empty() {
                "[REDACTED]".into()
            } else {
                workspace_name
            };
            result.push(json!({
                "owner_id":membership.owner_id,
                "owner_username":owner_account.username,
                "workspace_id":membership.workspace_id,
                "workspace_name":workspace_name,
                "revision":workspace.revision,
                "role":membership.role,
            }));
        }
    }
    Ok(Json(SharedPage {
        items: result,
        next_cursor,
    }))
}

pub async fn remove_member(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    session: Option<Extension<SessionAuthentication>>,
    Path((owner, workspace, member)): Path<(String, String, String)>,
) -> Result<Json<Value>, ApiError> {
    hosted_session(&state, session)?;
    let tx = state.db.begin().await?;
    lock_workspace(&tx, &owner, &workspace).await?;
    require_manager(&tx, &actor.0, &owner, &workspace).await?;
    let id = membership_id(&owner, &workspace, &member);
    let affected = workspace_membership::Entity::delete_by_id(id)
        .exec(&tx)
        .await?;
    if affected.rows_affected != 1 {
        return Err(ApiError::not_found());
    }
    record_event(
        &tx,
        &owner,
        &workspace,
        &actor.0,
        "membership.remove",
        Some(&member),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({"removed":true})))
}

pub async fn list_members(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    session: Option<Extension<SessionAuthentication>>,
    Path((owner, workspace)): Path<(String, String)>,
) -> Result<Json<Vec<Value>>, ApiError> {
    hosted_session(&state, session)?;
    storage::get(&state.db, &owner, &workspace)
        .await?
        .ok_or_else(ApiError::not_found)?;
    require_manager(&state.db, &actor.0, &owner, &workspace).await?;
    let owner_account = account::Entity::find_by_id(&owner)
        .one(&state.db)
        .await?
        .ok_or_else(ApiError::not_found)?;
    let mut result = vec![
        json!({"member_id":owner,"username":owner_account.username,"role":"owner","revision":0}),
    ];
    let members = workspace_membership::Entity::find()
        .filter(workspace_membership::Column::OwnerId.eq(&owner))
        .filter(workspace_membership::Column::WorkspaceId.eq(&workspace))
        .order_by_asc(workspace_membership::Column::CreatedAt)
        .limit(MAX_MEMBERS)
        .all(&state.db)
        .await?;
    for member in members {
        if let Some(account) = account::Entity::find_by_id(&member.member_id)
            .one(&state.db)
            .await?
        {
            result.push(json!({
                "member_id":member.member_id,
                "username":account.username,
                "role":member.role,
                "revision":member.revision,
                "created_at":member.created_at,
            }));
        }
    }
    Ok(Json(result))
}

pub async fn list_audit(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    session: Option<Extension<SessionAuthentication>>,
    Path((owner, workspace)): Path<(String, String)>,
) -> Result<Json<Vec<Value>>, ApiError> {
    hosted_session(&state, session)?;
    let (_, admission) = access(&state.db, &actor.0, &owner, &workspace).await?;
    if !matches!(admission.role, Role::Owner | Role::Admin) {
        return Err(ApiError::not_found());
    }
    let events = workspace_audit_event::Entity::find()
        .filter(workspace_audit_event::Column::OwnerId.eq(&owner))
        .filter(workspace_audit_event::Column::WorkspaceId.eq(&workspace))
        .order_by_desc(workspace_audit_event::Column::CreatedAt)
        .order_by_desc(workspace_audit_event::Column::Id)
        .limit(100)
        .all(&state.db)
        .await?;
    let (_, current) = access(&state.db, &actor.0, &owner, &workspace).await?;
    if current != admission {
        return Err(ApiError::not_found());
    }
    Ok(Json(
        events
            .into_iter()
            .map(|event| {
                json!({
                    "id":event.id,
                    "owner_id":event.owner_id,
                    "workspace_id":event.workspace_id,
                    "actor_id":event.actor_id,
                    "action":event.action,
                    "subject_id":event.subject_id,
                    "result":event.result,
                    "created_at":event.created_at,
                })
            })
            .collect(),
    ))
}

pub async fn change_role(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    session: Option<Extension<SessionAuthentication>>,
    Path((owner, workspace, member)): Path<(String, String, String)>,
    Json(input): Json<UpdateRole>,
) -> Result<Json<Value>, ApiError> {
    hosted_session(&state, session)?;
    if input.role == Role::Owner || input.expected_revision < 1 {
        return Err(ApiError::bad("Invalid member role or revision"));
    }
    let tx = state.db.begin().await?;
    lock_workspace(&tx, &owner, &workspace).await?;
    require_manager(&tx, &actor.0, &owner, &workspace).await?;
    let current =
        workspace_membership::Entity::find_by_id(membership_id(&owner, &workspace, &member))
            .one(&tx)
            .await?
            .ok_or_else(ApiError::not_found)?;
    if current.revision != input.expected_revision {
        return Err(ApiError::conflict("Member role changed"));
    }
    let revision = current
        .revision
        .checked_add(1)
        .ok_or_else(ApiError::internal)?;
    let mut update: workspace_membership::ActiveModel = current.into();
    update.role = Set(input.role.as_str().into());
    update.revision = Set(revision);
    update.updated_at = Set(chrono::Utc::now().timestamp());
    update.update(&tx).await?;
    record_event(
        &tx,
        &owner,
        &workspace,
        &actor.0,
        "membership.role",
        Some(&member),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(
        json!({"member_id":member,"role":input.role,"revision":revision}),
    ))
}

pub async fn revoke_invitation(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    session: Option<Extension<SessionAuthentication>>,
    Path((owner, workspace, invitation)): Path<(String, String, String)>,
) -> Result<Json<Value>, ApiError> {
    hosted_session(&state, session)?;
    let tx = state.db.begin().await?;
    lock_workspace(&tx, &owner, &workspace).await?;
    require_manager(&tx, &actor.0, &owner, &workspace).await?;
    let current = workspace_invitation::Entity::find_by_id(&invitation)
        .filter(workspace_invitation::Column::OwnerId.eq(&owner))
        .filter(workspace_invitation::Column::WorkspaceId.eq(&workspace))
        .one(&tx)
        .await?
        .ok_or_else(ApiError::not_found)?;
    if current.consumed_by.is_some() {
        return Err(ApiError::bad("Accepted invitation requires member removal"));
    }
    if current.revoked_at.is_none() {
        let revision = current
            .revision
            .checked_add(1)
            .ok_or_else(ApiError::internal)?;
        let mut update: workspace_invitation::ActiveModel = current.into();
        update.revoked_at = Set(Some(chrono::Utc::now().timestamp()));
        update.revision = Set(revision);
        update.update(&tx).await?;
        record_event(
            &tx,
            &owner,
            &workspace,
            &actor.0,
            "invitation.revoke",
            Some(&invitation),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(Json(json!({"revoked":true})))
}

pub(crate) async fn cascade<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    workspace: &str,
) -> Result<(), sea_orm::DbErr> {
    workspace_audit_event::Entity::delete_many()
        .filter(workspace_audit_event::Column::OwnerId.eq(owner))
        .filter(workspace_audit_event::Column::WorkspaceId.eq(workspace))
        .exec(db)
        .await?;
    workspace_invitation::Entity::delete_many()
        .filter(workspace_invitation::Column::OwnerId.eq(owner))
        .filter(workspace_invitation::Column::WorkspaceId.eq(workspace))
        .exec(db)
        .await?;
    workspace_membership::Entity::delete_many()
        .filter(workspace_membership::Column::OwnerId.eq(owner))
        .filter(workspace_membership::Column::WorkspaceId.eq(workspace))
        .exec(db)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn completed_operation_releases_pending_watcher_before_final_database_read() {
        use sea_orm::{ConnectOptions, Database, DbBackend, Statement};
        use std::time::Duration;
        for operation_succeeds in [true, false] {
            let mut options = ConnectOptions::new("sqlite::memory:");
            options
                .min_connections(1)
                .max_connections(1)
                .acquire_timeout(Duration::from_millis(200));
            let db = Database::connect(options).await.unwrap();
            let held = tokio::sync::Notify::new();
            let watch = async {
                let transaction = db.begin().await.unwrap();
                held.notify_one();
                std::future::pending::<()>().await;
                drop(transaction);
                ApiError::not_found()
            };
            let operation = async {
                held.notified().await;
                if operation_succeeds {
                    Ok(())
                } else {
                    Err(ApiError::bad("operation failed"))
                }
            };
            let checked = std::sync::atomic::AtomicBool::new(false);
            let final_check = async {
                db.execute(Statement::from_string(
                    DbBackend::Sqlite,
                    "SELECT 1".to_owned(),
                ))
                .await
                .map_err(|error| {
                    eprintln!("watcher lifetime regression: {error:?}");
                    ApiError::from(error)
                })?;
                checked.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            };
            let result = with_access_watch(operation, watch, final_check).await;
            assert!(
                checked.load(std::sync::atomic::Ordering::SeqCst),
                "final permission DB read must run after releasing the watcher connection"
            );
            if operation_succeeds {
                assert!(result.is_ok());
            } else {
                assert_eq!(
                    result.unwrap_err().status,
                    axum::http::StatusCode::BAD_REQUEST
                );
            }
        }
    }

    #[tokio::test]
    async fn denied_watcher_releases_running_operation_and_skips_final_check() {
        use sea_orm::{ConnectOptions, Database, DbBackend, Statement};
        use std::time::Duration;
        let mut options = ConnectOptions::new("sqlite::memory:");
        options
            .min_connections(1)
            .max_connections(1)
            .acquire_timeout(Duration::from_millis(200));
        let db = Database::connect(options).await.unwrap();
        let held = tokio::sync::Notify::new();
        let operation = async {
            let transaction = db.begin().await.unwrap();
            held.notify_one();
            std::future::pending::<()>().await;
            drop(transaction);
            Ok(())
        };
        let watch = async {
            held.notified().await;
            ApiError::forbidden("access revoked")
        };
        let checked = std::sync::atomic::AtomicBool::new(false);
        let final_check = async {
            checked.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        };
        let error = with_access_watch(operation, watch, final_check)
            .await
            .unwrap_err();
        assert_eq!(error.status, axum::http::StatusCode::FORBIDDEN);
        assert!(!checked.load(std::sync::atomic::Ordering::SeqCst));
        db.execute(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT 1".to_owned(),
        ))
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn ready_watcher_has_priority_and_final_denial_withholds_private_output() {
        let checked = std::sync::atomic::AtomicBool::new(false);
        let error = with_access_watch(
            async { Ok("private output") },
            async { ApiError::not_found() },
            async {
                checked.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            },
        )
        .await
        .unwrap_err();
        assert_eq!(error.status, axum::http::StatusCode::NOT_FOUND);
        assert!(!checked.load(std::sync::atomic::Ordering::SeqCst));
        let error = with_access_watch(
            async { Ok("private output") },
            std::future::pending::<ApiError>(),
            async { Err(ApiError::conflict("revision changed")) },
        )
        .await
        .unwrap_err();
        assert_eq!(error.status, axum::http::StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn specification_preview_rechecks_after_a_pending_credential_check() {
        for change in ["role", "remove", "revision"] {
            let db = storage::connect("sqlite::memory:").await.unwrap();
            let workspace: Workspace = serde_json::from_value(json!({"id":"w","name":"Private","revision":1,"updated_at":"now","data":{"schema_version":1,"collections":[],"environments":[],"active_environment_id":null}})).unwrap();
            storage::insert_doc(
                &db,
                storage::workspace_key("owner", "w"),
                "owner",
                "workspace",
                "w",
                1,
                &workspace,
            )
            .await
            .unwrap();
            workspace_membership::ActiveModel {
                id: Set(membership_id("owner", "w", "editor")),
                owner_id: Set("owner".into()),
                workspace_id: Set("w".into()),
                member_id: Set("editor".into()),
                role: Set("editor".into()),
                generation: Set(uuid::Uuid::new_v4().to_string()),
                revision: Set(1),
                created_at: Set(0),
                updated_at: Set(0),
            }
            .insert(&db)
            .await
            .unwrap();
            let (_, admission) = access(&db, "editor", "owner", "w").await.unwrap();
            let credential = async {
                // A successful credential read may suspend while another instance commits a change.
                match change {
                    "role" => {
                        workspace_membership::Entity::update_many()
                            .col_expr(workspace_membership::Column::Role, Expr::value("viewer"))
                            .exec(&db)
                            .await
                            .unwrap();
                    }
                    "remove" => {
                        workspace_membership::Entity::delete_many()
                            .exec(&db)
                            .await
                            .unwrap();
                    }
                    _ => {
                        let mut next = workspace.clone();
                        next.revision = 2;
                        document::Entity::update_many()
                            .col_expr(
                                document::Column::Payload,
                                Expr::value(serde_json::to_string(&next).unwrap()),
                            )
                            .exec(&db)
                            .await
                            .unwrap();
                    }
                }
                Ok(())
            };
            let result = finish_specification_preview(
                &db,
                "editor",
                "owner",
                "w",
                (admission, 1),
                credential,
            )
            .await;
            assert!(
                result.is_err(),
                "private preview must be withheld after {change} during credential validation"
            );
        }
    }

    #[test]
    fn viewer_projection_accepts_valid_workspace_when_pretty_export_exceeds_import_limit() {
        const LIMIT: usize = 20 * 1024 * 1024;
        let variables = (0..1000)
            .map(|index| {
                json!({"id":format!("v{index}"),"key":format!("key{index}"),"value":"x","enabled":true})
            })
            .collect::<Vec<_>>();
        let mut data = json!({
            "schema_version":1,
            "collections":[],
            "environments":[],
            "active_environment_id":null,
            "global_variables":variables,
            "specifications":(0..4).map(|index|json!({"id":format!("s{index}"),"name":"Spec","kind":"openapi","dialect":"3.0","source":"{}"})).collect::<Vec<_>>(),
        });
        let base_len = serde_json::to_vec(&data).unwrap().len();
        let filler = (LIMIT - base_len - 1024) / 4 - 20;
        let source = json!({"description":"x".repeat(filler)}).to_string();
        for specification in data["specifications"].as_array_mut().unwrap() {
            specification["source"] = json!(source);
        }
        let workspace = Workspace {
            id: "large".into(),
            name: "Large workspace".into(),
            revision: 1,
            updated_at: "now".into(),
            data: serde_json::from_value(data).unwrap(),
        };
        moleapi_core::validate_workspace(&workspace.data).unwrap();
        let exported = moleapi_formats::export(&workspace, "moleapi", false).unwrap();
        assert!(exported.content.len() > LIMIT);
        let projected = viewer_project(workspace).unwrap();
        assert_eq!(projected.id, "large");
        assert_eq!(projected.data.specifications.len(), 4);
    }
}
