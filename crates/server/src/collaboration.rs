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
    extract::{Path, Query, State},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use moleapi_core::{Workspace, WorkspaceData};
use rand::RngCore;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait, sea_query::Expr,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::future::Future;

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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Access {
    role: Role,
    revision: i64,
    generation: uuid::Uuid,
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

    let run = crate::runner::run(
        State(state.clone()),
        Extension(Identity(owner.clone())),
        Path(workspace.clone()),
        Json(input),
    );
    let watch = async {
        let mut check = tokio::time::interval(std::time::Duration::from_millis(200));
        check.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            check.tick().await;
            let current = access(&state.db, &actor.0, &owner, &workspace).await;
            if !matches!(current, Ok((_, current)) if current == admission) {
                break;
            }
        }
    };
    tokio::pin!(run);
    tokio::pin!(watch);
    let result = tokio::select! {
        biased;
        _ = &mut watch => {
            let _ = state.project_jobs.cancel_job(&owner, Some(&workspace), &job_id);
            return Err(ApiError::not_found());
        }
        result = &mut run => result,
    };
    let (_, current) = access(&state.db, &actor.0, &owner, &workspace).await?;
    if current != admission {
        return Err(ApiError::not_found());
    }
    result
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
            result.push(json!({
                "owner_id":membership.owner_id,
                "owner_username":owner_account.username,
                "workspace_id":membership.workspace_id,
                "workspace_name":workspace.name,
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
