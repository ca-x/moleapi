use crate::{
    ApiError, AppState,
    auth::{Identity, SessionAuthentication},
    entities::document,
    mock, storage,
};
use axum::{
    Extension, Json,
    extract::{OriginalUri, Path, Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use rand::RngCore;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, TransactionTrait, sea_query::Expr};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const KIND: &str = "mock-share";
const _: () = assert!(KIND.len() <= 16);
const PREFIX: &str = "msp_";
const MAX_SHARES: u64 = 20;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct MockShare {
    pub id: String,
    pub workspace_id: String,
    pub request_id: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub allow_dynamic: bool,
    request_digest: String,
}

pub(crate) struct PublicGrant {
    key: String,
    pub owner: String,
    pub share: MockShare,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Create {
    request_id: String,
    expires_in_days: u16,
    #[serde(default)]
    allow_dynamic: bool,
}

#[derive(Serialize)]
pub struct GrantedShare {
    #[serde(flatten)]
    share: ShareView,
    token: String,
}

#[derive(Serialize)]
pub struct ShareView {
    id: String,
    request_id: String,
    created_at: i64,
    expires_at: i64,
    allow_dynamic: bool,
    stale: bool,
}

fn view(share: &MockShare, stale: bool) -> ShareView {
    ShareView {
        id: share.id.clone(),
        request_id: share.request_id.clone(),
        created_at: share.created_at,
        expires_at: share.expires_at,
        allow_dynamic: share.allow_dynamic,
        stale,
    }
}

fn request_digest(
    workspace: &moleapi_core::Workspace,
    request: &moleapi_core::RequestSpec,
) -> Result<String, ApiError> {
    let bytes = serde_json::to_vec(request).map_err(|_| ApiError::internal())?;
    let mut digest = Sha256::new();
    digest.update(bytes);
    let linked: std::collections::BTreeSet<&str> = request
        .examples
        .iter()
        .filter_map(|example| {
            example
                .mock_schema_link
                .as_ref()
                .map(|link| link.specification_id.as_str())
        })
        .collect();
    for id in linked {
        let spec = workspace
            .data
            .specifications
            .iter()
            .find(|spec| spec.id == id)
            .ok_or_else(ApiError::not_found)?;
        let source = serde_json::to_vec(&(id, &spec.source)).map_err(|_| ApiError::internal())?;
        digest.update(source);
    }
    Ok(hex::encode(digest.finalize()))
}

fn validate_target(
    request: &moleapi_core::RequestSpec,
    allow_dynamic: bool,
) -> Result<(), ApiError> {
    if request.protocol != moleapi_core::Protocol::Http || request.examples.is_empty() {
        return Err(ApiError::bad(
            "Public Mock requires a saved HTTP response example",
        ));
    }
    if !allow_dynamic
        && request
            .examples
            .iter()
            .all(|example| !example.mock_script.is_empty())
    {
        return Err(ApiError::bad(
            "Public Mock requires a static example or explicit dynamic permission",
        ));
    }
    Ok(())
}

fn bound_request<'a>(
    workspace: &'a moleapi_core::Workspace,
    share: &MockShare,
) -> Option<&'a moleapi_core::RequestSpec> {
    workspace
        .data
        .collections
        .iter()
        .flat_map(|collection| &collection.requests)
        .find(|request| request.id == share.request_id)
}

pub(crate) fn request_matches(
    workspace: &moleapi_core::Workspace,
    share: &MockShare,
) -> Result<bool, ApiError> {
    bound_request(workspace, share)
        .map(|request| {
            request_digest(workspace, request).map(|digest| digest == share.request_digest)
        })
        .transpose()
        .map(|result| result.unwrap_or(false))
}

/// A removed request permanently loses its public capabilities, even if its ID is reused later.
pub(crate) async fn revoke_removed_requests<C: sea_orm::ConnectionTrait>(
    db: &C,
    owner: &str,
    previous: &moleapi_core::Workspace,
    next: &moleapi_core::Workspace,
) -> Result<(), sea_orm::DbErr> {
    let current_ids: std::collections::HashSet<_> = next
        .data
        .collections
        .iter()
        .flat_map(|collection| &collection.requests)
        .map(|request| request.id.as_str())
        .collect();
    let removed: std::collections::HashSet<_> = previous
        .data
        .collections
        .iter()
        .flat_map(|collection| &collection.requests)
        .map(|request| request.id.as_str())
        .filter(|id| !current_ids.contains(id))
        .collect();
    if removed.is_empty() {
        return Ok(());
    }
    let rows = document::Entity::find()
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(KIND))
        .filter(document::Column::RefId.eq(&previous.id))
        .all(db)
        .await?;
    for row in rows {
        let share: MockShare = serde_json::from_str(&row.payload)
            .map_err(|_| sea_orm::DbErr::Custom("Corrupt Mock share".into()))?;
        if removed.contains(share.request_id.as_str()) {
            document::Entity::delete_by_id(row.id).exec(db).await?;
        }
    }
    Ok(())
}

fn management(
    state: &AppState,
    session: Option<Extension<SessionAuthentication>>,
) -> Result<(), ApiError> {
    if state.local {
        return Ok(());
    }
    if session.is_none() {
        return Err(ApiError::forbidden(
            "Public Mock sharing requires a login session",
        ));
    }
    Ok(())
}

pub(crate) fn routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route(
            "/public/mock/{token}/{example_id}",
            axum::routing::get(direct).options(preflight),
        )
        .route("/public/mocks/{token}/{*path}", axum::routing::any(routed))
}

fn new_token() -> (String, String) {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let token = format!("{PREFIX}{}", hex::encode(bytes));
    let key = format!("ms_{}", hex::encode(Sha256::digest(token.as_bytes())));
    (token, key)
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

async fn owned_shares<C: sea_orm::ConnectionTrait>(
    db: &C,
    owner: &str,
    workspace: &str,
) -> Result<Vec<(String, MockShare)>, ApiError> {
    document::Entity::find()
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(KIND))
        .filter(document::Column::RefId.eq(workspace))
        .all(db)
        .await?
        .into_iter()
        .map(|row| {
            let share: MockShare =
                serde_json::from_str(&row.payload).map_err(|_| ApiError::internal())?;
            if share.workspace_id != workspace {
                return Err(ApiError::internal());
            }
            Ok((row.id, share))
        })
        .collect()
}

pub async fn list(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    session: Option<Extension<SessionAuthentication>>,
    Path(workspace): Path<String>,
) -> Result<Json<Vec<ShareView>>, ApiError> {
    management(&state, session)?;
    let source = crate::workspaces::owned(&state, &owner.0, &workspace).await?;
    let mut shares: Vec<_> = owned_shares(&state.db, &owner.0, &workspace)
        .await?
        .into_iter()
        .map(|(_, share)| request_matches(&source, &share).map(|matches| view(&share, !matches)))
        .collect::<Result<_, _>>()?;
    shares.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(Json(shares))
}

pub async fn create(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    session: Option<Extension<SessionAuthentication>>,
    Path(workspace): Path<String>,
    Json(input): Json<Create>,
) -> Result<Json<GrantedShare>, ApiError> {
    management(&state, session)?;
    if !(1..=90).contains(&input.expires_in_days) {
        return Err(ApiError::bad("Public Mock expiry must be 1–90 days"));
    }
    let tx = state.db.begin().await?;
    let source = lock_workspace(&tx, &owner.0, &workspace).await?;
    let request = source
        .data
        .collections
        .iter()
        .flat_map(|collection| &collection.requests)
        .find(|request| request.id == input.request_id)
        .ok_or_else(ApiError::not_found)?;
    validate_target(request, input.allow_dynamic)?;
    let now = chrono::Utc::now().timestamp();
    let mut active = 0u64;
    for (key, existing) in owned_shares(&tx, &owner.0, &workspace).await? {
        if existing.expires_at <= now {
            document::Entity::delete_by_id(key).exec(&tx).await?;
        } else {
            active += 1;
        }
    }
    if active >= MAX_SHARES {
        return Err(ApiError::bad("Public Mock share limit reached"));
    }
    let (token, key) = new_token();
    let share = MockShare {
        id: uuid::Uuid::new_v4().to_string(),
        workspace_id: workspace.clone(),
        request_id: input.request_id,
        created_at: now,
        expires_at: now + i64::from(input.expires_in_days) * 86_400,
        allow_dynamic: input.allow_dynamic,
        request_digest: request_digest(&source, request)?,
    };
    storage::insert_doc(&tx, key, &owner.0, KIND, &workspace, 1, &share).await?;
    tx.commit().await?;
    Ok(Json(GrantedShare {
        share: view(&share, false),
        token,
    }))
}

pub async fn revoke(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    session: Option<Extension<SessionAuthentication>>,
    Path((workspace, share_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    management(&state, session)?;
    let tx = state.db.begin().await?;
    lock_workspace(&tx, &owner.0, &workspace).await?;
    let key = owned_shares(&tx, &owner.0, &workspace)
        .await?
        .into_iter()
        .find(|(_, share)| share.id == share_id)
        .map(|(key, _)| key)
        .ok_or_else(ApiError::not_found)?;
    document::Entity::delete_by_id(key).exec(&tx).await?;
    tx.commit().await?;
    Ok(Json(serde_json::json!({"revoked":true})))
}

pub async fn rotate(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    session: Option<Extension<SessionAuthentication>>,
    Path((workspace, share_id)): Path<(String, String)>,
) -> Result<Json<GrantedShare>, ApiError> {
    management(&state, session)?;
    let tx = state.db.begin().await?;
    lock_workspace(&tx, &owner.0, &workspace).await?;
    let (old_key, mut share) = owned_shares(&tx, &owner.0, &workspace)
        .await?
        .into_iter()
        .find(|(_, share)| share.id == share_id)
        .ok_or_else(ApiError::not_found)?;
    if share.expires_at <= chrono::Utc::now().timestamp() {
        return Err(ApiError::bad("Expired public Mock share cannot be rotated"));
    }
    let current = storage::get(&tx, &owner.0, &workspace)
        .await?
        .ok_or_else(ApiError::not_found)?;
    let request = bound_request(&current, &share).ok_or_else(ApiError::not_found)?;
    validate_target(request, share.allow_dynamic)?;
    share.request_digest = request_digest(&current, request)?;
    let (token, key) = new_token();
    document::Entity::delete_by_id(old_key).exec(&tx).await?;
    storage::insert_doc(&tx, key, &owner.0, KIND, &workspace, 1, &share).await?;
    tx.commit().await?;
    Ok(Json(GrantedShare {
        share: view(&share, false),
        token,
    }))
}

async fn lookup(state: &AppState, token: &str) -> Result<PublicGrant, ApiError> {
    if token.len() != PREFIX.len() + 64
        || !token.starts_with(PREFIX)
        || !token[PREFIX.len()..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(ApiError::not_found());
    }
    let key = format!("ms_{}", hex::encode(Sha256::digest(token.as_bytes())));
    let row = document::Entity::find_by_id(&key)
        .one(&state.db)
        .await?
        .filter(|row| row.kind == KIND)
        .ok_or_else(ApiError::not_found)?;
    let share: MockShare = serde_json::from_str(&row.payload).map_err(|_| ApiError::not_found())?;
    if row.ref_id != share.workspace_id || share.expires_at <= chrono::Utc::now().timestamp() {
        return Err(ApiError::not_found());
    }
    Ok(PublicGrant {
        key,
        owner: row.owner,
        share,
    })
}

pub(crate) async fn still_active(state: &AppState, grant: &PublicGrant) -> Result<(), ApiError> {
    let row = document::Entity::find_by_id(&grant.key)
        .one(&state.db)
        .await?
        .filter(|row| {
            row.kind == KIND && row.owner == grant.owner && row.ref_id == grant.share.workspace_id
        })
        .ok_or_else(ApiError::not_found)?;
    let current: MockShare =
        serde_json::from_str(&row.payload).map_err(|_| ApiError::not_found())?;
    if current.id != grant.share.id
        || current.request_id != grant.share.request_id
        || current.allow_dynamic != grant.share.allow_dynamic
        || current.expires_at <= chrono::Utc::now().timestamp()
    {
        return Err(ApiError::not_found());
    }
    let workspace = storage::get(&state.db, &grant.owner, &grant.share.workspace_id)
        .await?
        .ok_or_else(ApiError::not_found)?;
    if !request_matches(&workspace, &current)? {
        return Err(ApiError::not_found());
    }
    Ok(())
}

fn safe_uri(path: &str, original: &Uri) -> Result<Uri, ApiError> {
    format!(
        "/mock/{path}{}",
        original
            .query()
            .map_or(String::new(), |query| format!("?{query}"))
    )
    .parse()
    .map_err(|_| ApiError::bad("Invalid public Mock path"))
}

pub(crate) fn public_response(result: Result<Response, ApiError>) -> Response {
    let mut response = match result {
        Ok(response) => response,
        Err(error) => match error.status {
            StatusCode::NOT_FOUND | StatusCode::CONFLICT => ApiError::not_found().into_response(),
            StatusCode::BAD_REQUEST | StatusCode::PAYLOAD_TOO_LARGE => {
                ApiError::bad("Invalid public Mock request").into_response()
            }
            StatusCode::TOO_MANY_REQUESTS => ApiError {
                status: StatusCode::TOO_MANY_REQUESTS,
                message: "Public Mock capacity reached".into(),
            }
            .into_response(),
            _ => ApiError::internal().into_response(),
        },
    };
    response
        .headers_mut()
        .insert("cache-control", HeaderValue::from_static("no-store"));
    response
        .headers_mut()
        .insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    response
        .headers_mut()
        .insert("access-control-allow-origin", HeaderValue::from_static("*"));
    response
}

pub async fn preflight() -> Response {
    let mut response = public_response(Ok(StatusCode::NO_CONTENT.into_response()));
    response.headers_mut().insert(
        "access-control-allow-methods",
        HeaderValue::from_static("GET, POST, PUT, PATCH, DELETE, OPTIONS"),
    );
    response.headers_mut().insert(
        "access-control-allow-headers",
        HeaderValue::from_static("*"),
    );
    response
}

pub async fn direct(
    State(state): State<AppState>,
    Path((token, example)): Path<(String, String)>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Response {
    let result = async {
        let grant = lookup(&state, &token).await?;
        let _permit = state
            .public_mock_slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| ApiError {
                status: StatusCode::TOO_MANY_REQUESTS,
                message: "Public Mock capacity reached".into(),
            })?;
        let safe = safe_uri(&example, &uri)?;
        mock::mock_for(&state, &grant, &example, &safe, &headers).await
    }
    .await;
    public_response(result)
}

pub async fn routed(
    State(state): State<AppState>,
    Path((token, path)): Path<(String, String)>,
    OriginalUri(uri): OriginalUri,
    incoming: Request,
) -> Response {
    if incoming.method() == Method::OPTIONS {
        return preflight().await;
    }
    let result = async {
        let grant = lookup(&state, &token).await?;
        let _permit = state
            .public_mock_slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| ApiError {
                status: StatusCode::TOO_MANY_REQUESTS,
                message: "Public Mock capacity reached".into(),
            })?;
        let safe = safe_uri(&path, &uri)?;
        mock::dispatch_for(&state, &grant, &path, &safe, incoming).await
    }
    .await;
    public_response(result)
}
