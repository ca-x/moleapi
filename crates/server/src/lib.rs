mod auth;
mod entities;
mod execution;
mod formats;
mod graphql;
mod grpc;
mod history;
mod mock;
mod privacy;
mod protocol_admission;
mod protocols;
mod runner;
mod soap;
mod storage;
mod sync;
mod workspaces;

use axum::{
    Json, Router,
    extract::DefaultBodyLimit,
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
};
pub use moleapi_script_runtime::dispatch_worker as dispatch_script_worker;
use sea_orm::DatabaseConnection;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub setup_token: String,
    pub allow_registration: bool,
    pub allow_private_network: bool,
}
#[derive(Clone)]
struct AppState {
    db: DatabaseConnection,
    config: Config,
    local: bool,
    sync_lock: Arc<tokio::sync::Mutex<()>>,
    script_slots: Arc<tokio::sync::Semaphore>,
    script_worker: PathBuf,
    protocol_sessions: Arc<moleapi_protocols::SessionManager>,
    protocol_admission: Arc<protocol_admission::AdmissionGates>,
}
#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}
impl ApiError {
    fn bad(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }
    fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: message.into(),
        }
    }
    fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            message: message.into(),
        }
    }
    fn unauthorized() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: "Invalid or expired session".into(),
        }
    }
    fn not_found() -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: "Resource not found".into(),
        }
    }
    fn internal() -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Database or internal operation failed".into(),
        }
    }
}
impl From<sea_orm::DbErr> for ApiError {
    fn from(_: sea_orm::DbErr) -> Self {
        Self::internal()
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(serde_json::json!({"error":self.message}))).into_response()
    }
}

/// Offline API for local IPC only. This function never opens a listening socket.
pub async fn local(database_path: &Path) -> anyhow::Result<Router> {
    local_with_worker(database_path, &std::env::current_exe()?).await
}

/// Embedder entrypoint with an explicitly trusted worker executable.
pub async fn local_with_worker(database_path: &Path, worker: &Path) -> anyhow::Result<Router> {
    build(
        Config {
            database_url: storage::sqlite_url(database_path)?,
            setup_token: String::new(),
            allow_registration: false,
            allow_private_network: true,
        },
        true,
        worker.to_owned(),
    )
    .await
}
pub fn sqlite_database_url(path: &Path) -> anyhow::Result<String> {
    storage::sqlite_url(path)
}
pub async fn hosted(config: Config) -> anyhow::Result<Router> {
    hosted_with_worker(config, &std::env::current_exe()?).await
}

/// Embedder entrypoint with an explicitly trusted worker executable.
pub async fn hosted_with_worker(config: Config, worker: &Path) -> anyhow::Result<Router> {
    build(config, false, worker.to_owned()).await
}
pub async fn setup_required(database_url: &str) -> anyhow::Result<bool> {
    use sea_orm::EntityTrait;
    Ok(entities::setting::Entity::find_by_id("registration")
        .one(&storage::connect(database_url).await?)
        .await?
        .is_some_and(|s| s.revision == 0))
}
async fn build(config: Config, local: bool, script_worker: PathBuf) -> anyhow::Result<Router> {
    anyhow::ensure!(
        script_worker.is_absolute(),
        "Script worker executable must be an absolute path"
    );
    let db = storage::connect(&config.database_url).await?;
    let state = AppState {
        db,
        config,
        local,
        sync_lock: Arc::new(tokio::sync::Mutex::new(())),
        script_slots: Arc::new(tokio::sync::Semaphore::new(4)),
        script_worker,
        protocol_sessions: moleapi_protocols::SessionManager::new(),
        protocol_admission: Arc::new(protocol_admission::AdmissionGates::default()),
    };
    let protected = Router::new()
        .route("/auth/logout", post(auth::logout))
        .route(
            "/workspaces",
            get(workspaces::list).post(workspaces::create),
        )
        .route(
            "/workspaces/{id}",
            get(workspaces::get)
                .put(workspaces::update)
                .delete(workspaces::delete),
        )
        .route("/workspaces/{id}/versions", get(workspaces::versions))
        .route(
            "/workspaces/{id}/history",
            get(history::history).delete(history::clear_history),
        )
        .route("/workspaces/{id}/run", post(runner::run))
        .route("/workspaces/{id}/export", post(formats::export))
        .route("/execute", post(execution::execute))
        .route("/soap/import", post(soap::import))
        .route("/soap/import-url", post(soap::import_url))
        .route("/soap/schema", post(soap::schema))
        .route("/soap/template", post(soap::template))
        .route("/graphql/introspect", post(graphql::introspect))
        .route("/graphql/schema", post(graphql::schema))
        .route("/grpc/schema", post(grpc::schema))
        .route("/grpc/import", post(grpc::import))
        .route("/grpc/reflect", post(grpc::reflect))
        .route("/sessions", post(protocols::create))
        .route(
            "/sessions/{id}",
            get(protocols::get).delete(protocols::delete),
        )
        .route("/sessions/{id}/events", get(protocols::events))
        .route("/sessions/{id}/send", post(protocols::send))
        .route("/sessions/{id}/close", post(protocols::close))
        .route("/import", post(formats::import))
        .route(
            "/mock/{workspace_id}/{request_id}/{example_id}",
            get(mock::mock),
        );
    let protected = if local {
        protected
            .route("/workspaces/{id}/sync", post(sync::sync))
            .route("/sync/status", get(sync::status))
            .route(
                "/sync/connect",
                post(sync::connect).delete(sync::disconnect),
            )
    } else {
        protected
    };
    let protected =
        protected.route_layer(middleware::from_fn_with_state(state.clone(), auth::guard));
    let api = protected
        .route(
            "/health",
            get(|| async {
                Json(serde_json::json!({"status":"ok","version":env!("CARGO_PKG_VERSION")}))
            }),
        )
        .route("/auth/status", get(auth::status))
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login))
        .fallback(|| async { ApiError::not_found() })
        .layer(middleware::from_fn(json_errors));
    let router = Router::new()
        .nest("/api", api)
        .layer(DefaultBodyLimit::max(25 * 1024 * 1024))
        .with_state(state);
    #[cfg(feature = "web")]
    let router = if !local {
        router.fallback(static_asset)
    } else {
        router.fallback(|| async { ApiError::not_found() })
    };
    #[cfg(not(feature = "web"))]
    let router = router.fallback(|| async { ApiError::not_found() });
    Ok(router)
}
#[cfg(feature = "web")]
#[derive(rust_embed::RustEmbed)]
#[folder = "$OUT_DIR/web"]
struct Assets;
#[cfg(feature = "web")]
async fn static_asset(uri: axum::http::Uri, method: axum::http::Method) -> Response {
    let _bundle_digest = env!("MOLEAPI_WEB_BUNDLE_DIGEST");
    if method != axum::http::Method::GET && method != axum::http::Method::HEAD
        || uri.path() == "/api"
        || uri.path().starts_with("/api/")
    {
        return ApiError::not_found().into_response();
    }
    let path = uri.path().trim_start_matches('/');
    let (file, mime) = if let Some(file) = Assets::get(path) {
        (
            file,
            mime_guess::from_path(path)
                .first_or_octet_stream()
                .to_string(),
        )
    } else if let Some(file) = Assets::get("index.html") {
        (file, "text/html; charset=utf-8".into())
    } else {
        return ApiError::not_found().into_response();
    };
    ([("content-type", mime)], file.data.into_owned()).into_response()
}

async fn json_errors(request: axum::extract::Request, next: middleware::Next) -> Response {
    let response = next.run(request).await;
    if response.extensions().get::<mock::SavedExample>().is_some() {
        return response;
    }
    let status = response.status();
    if (status.is_client_error() || status.is_server_error())
        && !response
            .headers()
            .get("content-type")
            .and_then(|h| h.to_str().ok())
            .is_some_and(|h| h.starts_with("application/json"))
    {
        let status = if status == StatusCode::UNPROCESSABLE_ENTITY {
            StatusCode::BAD_REQUEST
        } else {
            status
        };
        return (
            status,
            Json(
                serde_json::json!({"error":status.canonical_reason().unwrap_or("Request failed")}),
            ),
        )
            .into_response();
    }
    response
}
