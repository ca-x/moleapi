use crate::{
    ApiError, AppState,
    auth::{Identity, SessionAuthentication},
    entities::document,
    storage,
};
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::header,
    response::{IntoResponse, Response},
};
use rand::RngCore;
use sea_orm::{
    ColumnTrait, EntityTrait, QueryFilter, QuerySelect, TransactionTrait, sea_query::Expr,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

const KIND: &str = "doc-publication";
const _: () = assert!(KIND.len() <= 16);
const MAX_ACTIVE: usize = 20;
const MAX_HISTORY: usize = 256;
const MAX_SELECTED: usize = 32;
const MAX_SOURCE: usize = 1024 * 1024;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Create {
    title: String,
    expected_revision: i64,
    document_ids: Vec<String>,
    idempotency_key: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct PublishedDocument {
    pub title: String,
    pub folder: Option<String>,
    pub source: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct PublicSnapshot {
    pub title: String,
    pub documents: Vec<PublishedDocument>,
}

#[derive(Deserialize, Serialize)]
struct Publication {
    id: String,
    workspace_id: String,
    source_revision: i64,
    created_at: i64,
    idempotency_key: String,
    request_digest: String,
    #[serde(default)]
    revoked_at: Option<i64>,
    snapshot: PublicSnapshot,
}

#[derive(Serialize)]
pub struct PublicationView {
    id: String,
    source_revision: i64,
    created_at: i64,
    title: String,
    document_titles: Vec<String>,
    public_url: String,
    revoked: bool,
}

fn view(publication: &Publication) -> PublicationView {
    PublicationView {
        id: publication.id.clone(),
        source_revision: publication.source_revision,
        created_at: publication.created_at,
        title: publication.snapshot.title.clone(),
        document_titles: publication
            .snapshot
            .documents
            .iter()
            .map(|document| document.title.clone())
            .collect(),
        public_url: format!("/docs/{}", publication.id),
        revoked: publication.revoked_at.is_some(),
    }
}

fn require_hosted_session(
    state: &AppState,
    session: Option<Extension<SessionAuthentication>>,
) -> Result<(), ApiError> {
    if state.local || session.is_none() {
        return Err(ApiError::forbidden(
            "Document publication requires a hosted login session",
        ));
    }
    Ok(())
}

fn validate(input: &Create) -> Result<(), ApiError> {
    if input.title.trim() != input.title
        || !(1..=128).contains(&input.title.chars().count())
        || input.title.chars().any(char::is_control)
    {
        return Err(ApiError::bad("Publication title must be 1–128 characters"));
    }
    if input.expected_revision < 1 {
        return Err(ApiError::bad("Invalid workspace revision"));
    }
    if !(1..=MAX_SELECTED).contains(&input.document_ids.len()) {
        return Err(ApiError::bad("Select 1–32 Markdown documents"));
    }
    let mut ids = HashSet::new();
    if input
        .document_ids
        .iter()
        .any(|id| id.is_empty() || !ids.insert(id))
    {
        return Err(ApiError::bad("Publication document IDs must be distinct"));
    }
    if input.idempotency_key.trim() != input.idempotency_key
        || !(1..=128).contains(&input.idempotency_key.len())
        || input.idempotency_key.chars().any(char::is_control)
    {
        return Err(ApiError::bad("Invalid publication idempotency key"));
    }
    Ok(())
}

fn request_digest(input: &Create) -> Result<String, ApiError> {
    let content = serde_json::to_vec(&(&input.title, input.expected_revision, &input.document_ids))
        .map_err(|_| ApiError::internal())?;
    Ok(hex::encode(Sha256::digest(content)))
}

fn snapshot(
    workspace: &moleapi_core::Workspace,
    input: &Create,
) -> Result<PublicSnapshot, ApiError> {
    let mut documents = Vec::with_capacity(input.document_ids.len());
    let mut source_bytes = 0usize;
    for id in &input.document_ids {
        let source = workspace
            .data
            .documents
            .iter()
            .find(|document| document.id == *id)
            .ok_or_else(|| ApiError::bad("Selected Markdown document is missing"))?;
        if source.secret {
            return Err(ApiError::bad(
                "Private Markdown documents cannot be published",
            ));
        }
        source_bytes = source_bytes.saturating_add(source.source.len());
        if source_bytes > MAX_SOURCE {
            return Err(ApiError::bad("Publication source exceeds 1 MiB"));
        }
        let folder = source.collection_id.as_ref().and_then(|id| {
            workspace
                .data
                .collections
                .iter()
                .find(|collection| collection.id == *id)
                .map(|collection| collection.name.clone())
        });
        documents.push(PublishedDocument {
            title: source.name.clone(),
            folder,
            source: source.source.clone(),
        });
    }
    Ok(PublicSnapshot {
        title: input.title.clone(),
        documents,
    })
}

async fn lock_workspace<C: sea_orm::ConnectionTrait>(
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

async fn owned_publications<C: sea_orm::ConnectionTrait>(
    db: &C,
    owner: &str,
    workspace: &str,
) -> Result<Vec<Publication>, ApiError> {
    document::Entity::find()
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(KIND))
        .filter(document::Column::RefId.eq(workspace))
        .limit((MAX_HISTORY + 1) as u64)
        .all(db)
        .await?
        .into_iter()
        .map(|row| {
            let publication: Publication =
                serde_json::from_str(&row.payload).map_err(|_| ApiError::internal())?;
            if publication.workspace_id != workspace || row.id != format!("dp_{}", publication.id) {
                return Err(ApiError::internal());
            }
            Ok(publication)
        })
        .collect()
}

pub async fn list(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    session: Option<Extension<SessionAuthentication>>,
    Path(workspace): Path<String>,
) -> Result<Json<Vec<PublicationView>>, ApiError> {
    require_hosted_session(&state, session)?;
    crate::workspaces::owned(&state, &owner.0, &workspace).await?;
    let mut versions: Vec<_> = owned_publications(&state.db, &owner.0, &workspace)
        .await?
        .iter()
        .map(view)
        .collect();
    versions.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(Json(versions))
}

pub async fn create(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    session: Option<Extension<SessionAuthentication>>,
    Path(workspace): Path<String>,
    Json(input): Json<Create>,
) -> Result<Json<PublicationView>, ApiError> {
    require_hosted_session(&state, session)?;
    validate(&input)?;
    let digest = request_digest(&input)?;
    let tx = state.db.begin().await?;
    let source = lock_workspace(&tx, &owner.0, &workspace).await?;
    let previous = owned_publications(&tx, &owner.0, &workspace).await?;
    if previous.len() > MAX_HISTORY {
        return Err(ApiError::bad("Publication history limit reached"));
    }
    if let Some(existing) = previous
        .iter()
        .find(|publication| publication.idempotency_key == input.idempotency_key)
    {
        if existing.request_digest != digest {
            return Err(ApiError::conflict(
                "Publication key was reused with different content",
            ));
        }
        if existing.revoked_at.is_some() {
            return Err(ApiError::conflict(
                "Publication key refers to a revoked version",
            ));
        }
        let result = view(existing);
        tx.commit().await?;
        return Ok(Json(result));
    }
    if source.revision != input.expected_revision {
        return Err(ApiError::conflict(
            "Workspace revision changed before publication",
        ));
    }
    if previous
        .iter()
        .filter(|publication| publication.revoked_at.is_none())
        .count()
        >= MAX_ACTIVE
    {
        return Err(ApiError::bad("Publication limit reached"));
    }
    if previous.len() >= MAX_HISTORY {
        return Err(ApiError::bad("Publication history limit reached"));
    }
    let snapshot = snapshot(&source, &input)?;
    let mut random = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut random);
    let publication = Publication {
        id: hex::encode(random),
        workspace_id: workspace.clone(),
        source_revision: source.revision,
        created_at: chrono::Utc::now().timestamp(),
        idempotency_key: input.idempotency_key,
        request_digest: digest,
        revoked_at: None,
        snapshot,
    };
    storage::insert_doc(
        &tx,
        format!("dp_{}", publication.id),
        &owner.0,
        KIND,
        &workspace,
        1,
        &publication,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(view(&publication)))
}

pub async fn revoke(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    session: Option<Extension<SessionAuthentication>>,
    Path((workspace, id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_hosted_session(&state, session)?;
    if !valid_id(&id) {
        return Err(ApiError::not_found());
    }
    let tx = state.db.begin().await?;
    lock_workspace(&tx, &owner.0, &workspace).await?;
    let row = document::Entity::find_by_id(format!("dp_{id}"))
        .filter(document::Column::Owner.eq(&owner.0))
        .filter(document::Column::Kind.eq(KIND))
        .filter(document::Column::RefId.eq(&workspace))
        .one(&tx)
        .await?
        .ok_or_else(ApiError::not_found)?;
    let mut publication: Publication =
        serde_json::from_str(&row.payload).map_err(|_| ApiError::internal())?;
    if publication.id != id
        || publication.workspace_id != workspace
        || publication.revoked_at.is_some()
    {
        return Err(ApiError::not_found());
    }
    publication.revoked_at = Some(chrono::Utc::now().timestamp());
    let changed = document::Entity::update_many()
        .col_expr(
            document::Column::Payload,
            Expr::value(serde_json::to_string(&publication).map_err(|_| ApiError::internal())?),
        )
        .col_expr(document::Column::UpdatedAt, Expr::value(storage::now()))
        .filter(document::Column::Id.eq(row.id))
        .filter(document::Column::Owner.eq(&owner.0))
        .filter(document::Column::Kind.eq(KIND))
        .exec(&tx)
        .await?;
    if changed.rows_affected != 1 {
        return Err(ApiError::not_found());
    }
    tx.commit().await?;
    Ok(Json(serde_json::json!({"revoked":true})))
}

fn valid_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub async fn public_get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    if !valid_id(&id) || state.local {
        return Err(ApiError::not_found());
    }
    let row = document::Entity::find_by_id(format!("dp_{id}"))
        .filter(document::Column::Kind.eq(KIND))
        .one(&state.db)
        .await?
        .ok_or_else(ApiError::not_found)?;
    let publication: Publication =
        serde_json::from_str(&row.payload).map_err(|_| ApiError::not_found())?;
    if publication.id != id
        || publication.workspace_id != row.ref_id
        || publication.revoked_at.is_some()
    {
        return Err(ApiError::not_found());
    }
    if storage::get(&state.db, &row.owner, &publication.workspace_id)
        .await?
        .is_none()
    {
        return Err(ApiError::not_found());
    }
    Ok((
        [
            (header::CACHE_CONTROL, "no-store"),
            (header::REFERRER_POLICY, "no-referrer"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (
                header::CONTENT_SECURITY_POLICY,
                "default-src 'none'; frame-ancestors 'none'",
            ),
        ],
        Json(publication.snapshot),
    )
        .into_response())
}
