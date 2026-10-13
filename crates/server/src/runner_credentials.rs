use crate::{
    ApiError, AppState,
    auth::{Identity, SessionAuthentication},
    entities::document,
    runners::storage::{RUNNER, TASK, Task, account_lock, get, save, save_task},
    scheduling::workspace_lock,
    storage,
};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use moleapi_core::ExecutionRunner;
use rand::RngCore;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, TransactionTrait};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub(crate) const PREFIX: &str = "moleapi_runner_";
const KIND: &str = "runner-credential";

#[derive(Clone)]
pub(crate) struct AgentScope {
    pub(crate) document_id: String,
    pub(crate) credential_id: String,
    pub(crate) runner_id: String,
    pub(crate) workspace_id: String,
}

#[derive(Serialize, Deserialize)]
struct Credential {
    id: String,
    runner_id: String,
    workspace_id: String,
    name: String,
    created_at: i64,
    expires_at: i64,
}

fn management(
    state: &AppState,
    session: Option<Extension<SessionAuthentication>>,
) -> Result<(), ApiError> {
    if state.local {
        return Err(ApiError::not_found());
    }
    if session.is_none() {
        return Err(ApiError::forbidden(
            "Runner credential management requires a login session",
        ));
    }
    Ok(())
}

fn metadata(credential: &Credential) -> Value {
    json!({"id":credential.id,"runner_id":credential.runner_id,"workspace_id":credential.workspace_id,"name":credential.name,"created_at":credential.created_at,"expires_at":credential.expires_at,"expired":credential.expires_at<=chrono::Utc::now().timestamp()})
}

fn row_id(token: &str) -> String {
    format!("{KIND}:{}", crate::auth::token_hash(token))
}

pub(crate) async fn lookup(
    state: &AppState,
    token: &str,
) -> Result<Option<(String, AgentScope)>, ApiError> {
    let Some(secret) = token.strip_prefix(PREFIX) else {
        return Ok(None);
    };
    if secret.len() != 64
        || !secret
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Ok(None);
    }
    let Some(row) = document::Entity::find_by_id(row_id(token))
        .filter(document::Column::Kind.eq(KIND))
        .one(&state.db)
        .await?
    else {
        return Ok(None);
    };
    let credential: Credential =
        serde_json::from_str(&row.payload).map_err(|_| ApiError::internal())?;
    if credential.expires_at <= chrono::Utc::now().timestamp() {
        return Ok(None);
    }
    Ok(Some((
        row.owner,
        AgentScope {
            document_id: row.id,
            credential_id: credential.id,
            runner_id: credential.runner_id,
            workspace_id: credential.workspace_id,
        },
    )))
}

pub(crate) async fn validate<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    scope: &AgentScope,
) -> Result<(), ApiError> {
    let row = document::Entity::find_by_id(&scope.document_id)
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(KIND))
        .filter(document::Column::RefId.eq(&scope.workspace_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::unauthorized)?;
    let credential: Credential =
        serde_json::from_str(&row.payload).map_err(|_| ApiError::internal())?;
    if credential.id != scope.credential_id
        || credential.runner_id != scope.runner_id
        || credential.workspace_id != scope.workspace_id
        || credential.expires_at <= chrono::Utc::now().timestamp()
    {
        return Err(ApiError::unauthorized());
    }
    Ok(())
}

pub(crate) fn agent_route<'a>(method: &axum::http::Method, path: &'a str) -> Option<&'a str> {
    if method != axum::http::Method::POST {
        return None;
    }
    let parts: Vec<_> = path.split('/').filter(|part| !part.is_empty()).collect();
    match parts.as_slice() {
        ["runners", id, "claim"] | ["runners", id, "tasks", _, "heartbeat" | "complete"] => {
            Some(id)
        }
        _ => None,
    }
}

pub(crate) fn allowed(method: &axum::http::Method, path: &str, scope: &AgentScope) -> bool {
    agent_route(method, path).is_some_and(|runner| runner == scope.runner_id)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Create {
    workspace_id: String,
    name: String,
    #[serde(default = "default_days")]
    expires_in_days: u16,
}
fn default_days() -> u16 {
    90
}

pub(crate) async fn create(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    session: Option<Extension<SessionAuthentication>>,
    Path(runner_id): Path<String>,
    Json(input): Json<Create>,
) -> Result<Json<Value>, ApiError> {
    management(&state, session)?;
    if input.name.trim() != input.name
        || !(1..=80).contains(&input.name.chars().count())
        || input.name.chars().any(char::is_control)
        || !(1..=365).contains(&input.expires_in_days)
    {
        return Err(ApiError::bad(
            "Credential name must be 1–80 characters; expiry must be 1–365 days",
        ));
    }
    let tx = state.db.begin().await?;
    account_lock(&tx, &owner.0).await?;
    let runner: ExecutionRunner = get(&tx, &owner.0, RUNNER, &runner_id).await?;
    if !runner.enabled {
        return Err(ApiError::bad("Runner is disabled"));
    }
    workspace_lock(&tx, &owner.0, &input.workspace_id).await?;
    let existing = document::Entity::find()
        .filter(document::Column::Owner.eq(&owner.0))
        .filter(document::Column::Kind.eq(KIND))
        .all(&tx)
        .await?;
    if existing.len() >= 100 {
        return Err(ApiError::bad("Account exceeds 100 runner credentials"));
    }
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let token = format!("{PREFIX}{}", hex::encode(bytes));
    let now = chrono::Utc::now().timestamp();
    let credential = Credential {
        id: uuid::Uuid::new_v4().to_string(),
        runner_id,
        workspace_id: input.workspace_id,
        name: input.name,
        created_at: now,
        expires_at: now + i64::from(input.expires_in_days) * 86400,
    };
    storage::insert_doc(
        &tx,
        row_id(&token),
        &owner.0,
        KIND,
        &credential.workspace_id,
        1,
        &credential,
    )
    .await?;
    tx.commit().await?;
    let mut output = metadata(&credential);
    output["token"] = json!(token);
    Ok(Json(output))
}

pub(crate) async fn list(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    session: Option<Extension<SessionAuthentication>>,
    Path(runner_id): Path<String>,
) -> Result<Json<Vec<Value>>, ApiError> {
    management(&state, session)?;
    let _: ExecutionRunner = get(&state.db, &owner.0, RUNNER, &runner_id).await?;
    let rows: Vec<Credential> = storage::documents(&state.db, &owner.0, KIND, None, 100).await?;
    Ok(Json(
        rows.iter()
            .filter(|row| row.runner_id == runner_id)
            .map(metadata)
            .collect(),
    ))
}

pub(crate) async fn revoke(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    session: Option<Extension<SessionAuthentication>>,
    Path((runner_id, id)): Path<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    management(&state, session)?;
    let tx = state.db.begin().await?;
    account_lock(&tx, &owner.0).await?;
    let _: ExecutionRunner = get(&tx, &owner.0, RUNNER, &runner_id).await?;
    let rows = document::Entity::find()
        .filter(document::Column::Owner.eq(&owner.0))
        .filter(document::Column::Kind.eq(KIND))
        .all(&tx)
        .await?;
    let row = rows
        .into_iter()
        .find(|row| {
            serde_json::from_str::<Credential>(&row.payload)
                .ok()
                .is_some_and(|credential| credential.id == id && credential.runner_id == runner_id)
        })
        .ok_or_else(ApiError::not_found)?;
    let mut runner: ExecutionRunner = get(&tx, &owner.0, RUNNER, &runner_id).await?;
    if let Some(task_id) = &runner.active_task_id {
        match get::<_, Task>(&tx, &owner.0, TASK, task_id).await {
            Ok(mut task) if task.agent_credential_id.as_deref() == Some(id.as_str()) => {
                crate::runners::terminal(&mut task, moleapi_core::RunnerTaskStatus::Cancelled);
                save_task(&tx, &owner.0, &task).await?;
                runner.active_task_id = None;
                save(&tx, &owner.0, RUNNER, &runner_id, runner.revision, &runner).await?;
            }
            Ok(_) => {}
            Err(error) if error.status == axum::http::StatusCode::NOT_FOUND => {}
            Err(error) => return Err(error),
        }
    }
    document::Entity::delete_by_id(row.id).exec(&tx).await?;
    tx.commit().await?;
    Ok(Json(json!({"revoked":true})))
}
