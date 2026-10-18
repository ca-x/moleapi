use crate::{
    ApiError, AppState,
    auth::{Identity, SessionAuthentication},
    entities::document,
    storage,
};
use argon2::{
    Argon2, PasswordHasher, PasswordVerifier,
    password_hash::{PasswordHash, SaltString, rand_core::OsRng},
};
use axum::{
    Extension, Json,
    extract::{Path, Request, State, rejection::JsonRejection},
    http::{HeaderValue, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rand::RngCore;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter, QuerySelect, Statement,
    TransactionTrait, sea_query::Expr,
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
const MAX_FAILED_UNLOCKS: usize = 10;
const ATTEMPT_WINDOW: i64 = 5 * 60;
const MAX_LOGO_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    Light,
    Dark,
    System,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    #[default]
    Sidebar,
    Top,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Appearance {
    #[serde(default)]
    pub theme: Theme,
    #[serde(default)]
    pub layout: Layout,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo_png_base64: Option<String>,
}

impl Appearance {
    fn is_default(&self) -> bool {
        self.theme == Theme::Light
            && self.layout == Layout::Sidebar
            && self.logo_png_base64.is_none()
    }
}

fn failed_attempt_retry(attempts: &mut Vec<i64>, now: i64) -> Result<Option<i64>, ApiError> {
    if attempts.len() > MAX_FAILED_UNLOCKS
        || attempts.windows(2).any(|pair| pair[0] > pair[1])
        || attempts.iter().any(|time| *time <= 0 || *time > now)
    {
        return Err(ApiError::internal());
    }
    attempts.retain(|time| *time > now - ATTEMPT_WINDOW);
    Ok((attempts.len() >= MAX_FAILED_UNLOCKS).then(|| (attempts[0] + ATTEMPT_WINDOW - now).max(1)))
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Visibility {
    #[default]
    Public,
    Password,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Create {
    title: String,
    expected_revision: i64,
    document_ids: Vec<String>,
    idempotency_key: String,
    #[serde(default)]
    visibility: Visibility,
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    appearance: Appearance,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Unlock {
    password: String,
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
    #[serde(default)]
    pub appearance: Appearance,
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
    #[serde(default)]
    password_hash: Option<String>,
    #[serde(default)]
    failed_attempts: Vec<i64>,
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
    visibility: Visibility,
    appearance: AppearanceView,
}

#[derive(Serialize)]
struct AppearanceView {
    theme: Theme,
    layout: Layout,
    has_logo: bool,
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
        visibility: if publication.password_hash.is_some() {
            Visibility::Password
        } else {
            Visibility::Public
        },
        appearance: AppearanceView {
            theme: publication.snapshot.appearance.theme.clone(),
            layout: publication.snapshot.appearance.layout.clone(),
            has_logo: publication.snapshot.appearance.logo_png_base64.is_some(),
        },
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
    match (input.visibility, input.password.as_deref()) {
        (Visibility::Public, None) => {}
        (Visibility::Public, Some(_)) => {
            return Err(ApiError::bad(
                "Public publication cannot include a password",
            ));
        }
        (Visibility::Password, Some(password)) if (10..=128).contains(&password.len()) => {}
        (Visibility::Password, _) => {
            return Err(ApiError::bad("Publication password must be 10–128 bytes"));
        }
    }
    Ok(())
}

fn request_digest(input: &Create) -> Result<String, ApiError> {
    let content = serde_json::to_vec(&(&input.title, input.expected_revision, &input.document_ids))
        .map_err(|_| ApiError::internal())?;
    let mut digest = Sha256::new();
    if input.visibility == Visibility::Password {
        digest.update(b"password\0");
    }
    digest.update(content);
    if !input.appearance.is_default() {
        digest.update(b"\0appearance\0");
        digest.update(serde_json::to_vec(&input.appearance).map_err(|_| ApiError::internal())?);
    }
    Ok(hex::encode(digest.finalize()))
}

fn sanitize_logo(appearance: &mut Appearance) -> Result<(), ApiError> {
    let Some(encoded) = appearance.logo_png_base64.as_ref() else {
        return Ok(());
    };
    if encoded.len() > (MAX_LOGO_BYTES / 3 + 1) * 4 {
        return Err(ApiError::bad("PNG logo exceeds 64 KiB"));
    }
    let source = BASE64
        .decode(encoded)
        .map_err(|_| ApiError::bad("Invalid PNG logo"))?;
    if source.is_empty() || source.len() > MAX_LOGO_BYTES {
        return Err(ApiError::bad("PNG logo exceeds 64 KiB"));
    }
    if !has_single_png_stream(&source) {
        return Err(ApiError::bad("Invalid PNG logo"));
    }
    let mut decoder = png::Decoder::new(std::io::Cursor::new(source.as_slice()));
    decoder.set_limits(png::Limits {
        bytes: 2 * 1024 * 1024,
    });
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder
        .read_info()
        .map_err(|_| ApiError::bad("Invalid PNG logo"))?;
    let info = reader.info();
    if !(1..=512).contains(&info.width)
        || !(1..=512).contains(&info.height)
        || info.animation_control().is_some()
    {
        return Err(ApiError::bad(
            "PNG logo must be a static image of at most 512 × 512 pixels",
        ));
    }
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| ApiError::bad("Invalid PNG logo"))?;
    if size > 1024 * 1024 {
        return Err(ApiError::bad("PNG logo is too large"));
    }
    let mut pixels = vec![0; size];
    let frame = reader
        .next_frame(&mut pixels)
        .map_err(|_| ApiError::bad("Invalid PNG logo"))?;
    reader
        .finish()
        .map_err(|_| ApiError::bad("Invalid PNG logo"))?;
    let mut cleaned = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut cleaned, frame.width, frame.height);
        encoder.set_color(frame.color_type);
        encoder.set_depth(frame.bit_depth);
        let mut writer = encoder
            .write_header()
            .map_err(|_| ApiError::bad("Invalid PNG logo"))?;
        writer
            .write_image_data(&pixels[..frame.buffer_size()])
            .map_err(|_| ApiError::bad("Invalid PNG logo"))?;
    }
    if cleaned.len() > MAX_LOGO_BYTES {
        return Err(ApiError::bad("Sanitized PNG logo exceeds 64 KiB"));
    }
    appearance.logo_png_base64 = Some(BASE64.encode(cleaned));
    Ok(())
}

fn has_single_png_stream(source: &[u8]) -> bool {
    if !source.starts_with(b"\x89PNG\r\n\x1a\n") {
        return false;
    }
    let mut offset = 8usize;
    while let Some(header) = source.get(offset..offset.saturating_add(8)) {
        let length = u32::from_be_bytes(header[..4].try_into().expect("four bytes")) as usize;
        let Some(end) = offset
            .checked_add(12)
            .and_then(|end| end.checked_add(length))
        else {
            return false;
        };
        if end > source.len() {
            return false;
        }
        if &header[4..8] == b"IEND" {
            return length == 0 && end == source.len();
        }
        offset = end;
    }
    false
}

async fn hash_password(password: String) -> Result<String, ApiError> {
    tokio::task::spawn_blocking(move || {
        Argon2::default()
            .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
            .map(|hash| hash.to_string())
    })
    .await
    .map_err(|_| ApiError::internal())?
    .map_err(|_| ApiError::internal())
}

async fn password_matches(password: String, hash: String) -> Result<bool, ApiError> {
    tokio::task::spawn_blocking(move || {
        PasswordHash::new(&hash).ok().is_some_and(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
    })
    .await
    .map_err(|_| ApiError::internal())
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
        appearance: input.appearance.clone(),
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
    Json(mut input): Json<Create>,
) -> Result<Json<PublicationView>, ApiError> {
    require_hosted_session(&state, session)?;
    validate(&input)?;
    sanitize_logo(&mut input.appearance)?;
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
        if existing.revoked_at.is_some() {
            return Err(ApiError::conflict(
                "Publication key refers to a revoked version",
            ));
        }
        if existing.request_digest != digest {
            return Err(ApiError::conflict(
                "Publication key was reused with different content",
            ));
        }
        if let Some(hash) = &existing.password_hash
            && !password_matches(input.password.clone().unwrap_or_default(), hash.clone()).await?
        {
            return Err(ApiError::conflict(
                "Publication key was reused with a different password",
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
    let password_hash = match input.password.clone() {
        Some(password) => Some(hash_password(password).await?),
        None => None,
    };
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
        password_hash,
        failed_attempts: Vec::new(),
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

async fn active_publication<C: ConnectionTrait>(
    db: &C,
    id: &str,
) -> Result<(document::Model, Publication), ApiError> {
    if !valid_id(id) {
        return Err(ApiError::not_found());
    }
    let row = document::Entity::find_by_id(format!("dp_{id}"))
        .filter(document::Column::Kind.eq(KIND))
        .one(db)
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
    if storage::get(db, &row.owner, &publication.workspace_id)
        .await?
        .is_none()
    {
        return Err(ApiError::not_found());
    }
    Ok((row, publication))
}

pub(crate) async fn page_exists(state: &AppState, id: &str) -> Result<(), ApiError> {
    if state.local {
        return Err(ApiError::not_found());
    }
    active_publication(&state.db, id).await.map(|_| ())
}

pub(crate) async fn read_snapshot(state: &AppState, id: &str) -> Result<PublicSnapshot, ApiError> {
    if state.local {
        return Err(ApiError::not_found());
    }
    let (_, publication) = active_publication(&state.db, id).await?;
    if publication.password_hash.is_some() {
        return Err(ApiError::forbidden("Publication password required"));
    }
    Ok(publication.snapshot)
}

pub async fn public_get(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    match read_snapshot(&state, &id).await {
        Ok(snapshot) => match serde_json::to_value(snapshot) {
            Ok(value) => public_json_response(StatusCode::OK, value),
            Err(_) => public_json_headers(ApiError::internal().into_response()),
        },
        Err(error) => public_json_headers(error.into_response()),
    }
}

async fn database_now_seconds<C: ConnectionTrait>(db: &C) -> Result<i64, ApiError> {
    let backend = db.get_database_backend();
    let sql = match backend {
        DbBackend::Sqlite => "SELECT CAST(strftime('%s','now') AS INTEGER) AS now_sec",
        DbBackend::Postgres => {
            "SELECT CAST(EXTRACT(EPOCH FROM clock_timestamp()) AS BIGINT) AS now_sec"
        }
        DbBackend::MySql => "SELECT CAST(UNIX_TIMESTAMP(CURRENT_TIMESTAMP) AS SIGNED) AS now_sec",
    };
    let row = db
        .query_one(Statement::from_string(backend, sql))
        .await?
        .ok_or_else(ApiError::internal)?;
    let now: i64 = row.try_get("", "now_sec")?;
    if now <= 0 {
        return Err(ApiError::internal());
    }
    Ok(now)
}

async fn update_publication<C: ConnectionTrait>(
    db: &C,
    row: &document::Model,
    publication: &Publication,
) -> Result<(), ApiError> {
    let revision = row.revision.checked_add(1).ok_or_else(ApiError::internal)?;
    let changed = document::Entity::update_many()
        .col_expr(
            document::Column::Payload,
            Expr::value(serde_json::to_string(publication).map_err(|_| ApiError::internal())?),
        )
        .col_expr(document::Column::Revision, Expr::value(revision))
        .col_expr(document::Column::UpdatedAt, Expr::value(storage::now()))
        .filter(document::Column::Id.eq(&row.id))
        .filter(document::Column::Kind.eq(KIND))
        .filter(document::Column::Revision.eq(row.revision))
        .exec(db)
        .await?;
    if changed.rows_affected != 1 {
        return Err(ApiError::internal());
    }
    Ok(())
}

fn public_json_headers(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'none'; frame-ancestors 'none'"),
    );
    response
}

pub(crate) async fn public_docs_headers(request: Request, next: Next) -> Response {
    let is_public_docs = request.uri().path().starts_with("/public/docs/")
        || request.uri().path().starts_with("/api/public/docs/");
    let response = next.run(request).await;
    if is_public_docs {
        public_json_headers(response)
    } else {
        response
    }
}

fn public_json_response(status: StatusCode, body: serde_json::Value) -> Response {
    public_json_headers((status, Json(body)).into_response())
}

async fn unlock_inner(state: &AppState, id: &str, password: String) -> Result<Response, ApiError> {
    if state.local || !valid_id(id) {
        return Err(ApiError::not_found());
    }
    let tx = state.db.begin().await?;
    document::Entity::update_many()
        .col_expr(
            document::Column::Revision,
            Expr::col(document::Column::Revision).into(),
        )
        .filter(document::Column::Id.eq(format!("dp_{id}")))
        .filter(document::Column::Kind.eq(KIND))
        .exec(&tx)
        .await?;
    let (row, mut publication) = active_publication(&tx, id).await?;
    let Some(hash) = publication.password_hash.clone() else {
        return Err(ApiError::not_found());
    };
    if password.len() > 128 {
        return Ok(public_json_response(
            StatusCode::BAD_REQUEST,
            serde_json::json!({"error":"Invalid publication password"}),
        ));
    }
    let now = database_now_seconds(&tx).await?;
    if let Some(seconds) = failed_attempt_retry(&mut publication.failed_attempts, now)? {
        tx.commit().await?;
        let mut response = public_json_response(
            StatusCode::TOO_MANY_REQUESTS,
            serde_json::json!({"error":"Too many password attempts"}),
        );
        response.headers_mut().insert(
            header::RETRY_AFTER,
            HeaderValue::from_str(&seconds.to_string()).map_err(|_| ApiError::internal())?,
        );
        return Ok(response);
    }
    let valid = password_matches(password, hash).await?;
    if !valid {
        publication.failed_attempts.push(now);
        update_publication(&tx, &row, &publication).await?;
        tx.commit().await?;
        return Ok(public_json_response(
            StatusCode::UNAUTHORIZED,
            serde_json::json!({"error":"Invalid publication password"}),
        ));
    }
    if !publication.failed_attempts.is_empty() {
        publication.failed_attempts.clear();
        update_publication(&tx, &row, &publication).await?;
    }
    tx.commit().await?;
    Ok(public_json_response(
        StatusCode::OK,
        serde_json::to_value(publication.snapshot).map_err(|_| ApiError::internal())?,
    ))
}

pub async fn unlock(
    State(state): State<AppState>,
    Path(id): Path<String>,
    input: Result<Json<Unlock>, JsonRejection>,
) -> Response {
    if let Err(error) = page_exists(&state, &id).await {
        return public_json_headers(error.into_response());
    }
    let Json(input) = match input {
        Ok(input) => input,
        Err(rejection) => {
            return public_json_response(
                if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE {
                    StatusCode::PAYLOAD_TOO_LARGE
                } else {
                    StatusCode::BAD_REQUEST
                },
                serde_json::json!({"error":"Invalid unlock request"}),
            );
        }
    };
    match unlock_inner(&state, &id, input.password).await {
        Ok(response) => response,
        Err(error) => public_json_headers(error.into_response()),
    }
}

#[cfg(test)]
mod tests {
    use super::{ATTEMPT_WINDOW, MAX_FAILED_UNLOCKS, failed_attempt_retry};

    #[test]
    fn password_attempt_window_slides_and_rejects_future_or_corrupt_state() {
        let now = 1_000_000;
        let mut attempts: Vec<i64> = (0..MAX_FAILED_UNLOCKS)
            .map(|index| now + index as i64)
            .collect();
        assert_eq!(
            failed_attempt_retry(&mut attempts, now + ATTEMPT_WINDOW - 1).unwrap(),
            Some(1)
        );
        assert_eq!(
            failed_attempt_retry(&mut attempts, now + ATTEMPT_WINDOW).unwrap(),
            None
        );
        assert_eq!(attempts.len(), MAX_FAILED_UNLOCKS - 1);
        attempts.push(now + ATTEMPT_WINDOW);
        assert_eq!(
            failed_attempt_retry(&mut attempts, now + ATTEMPT_WINDOW).unwrap(),
            Some(1)
        );
        assert!(failed_attempt_retry(&mut vec![now + 1], now).is_err());
        assert!(failed_attempt_retry(&mut vec![now + 1, now], now + 1).is_err());
    }
}
