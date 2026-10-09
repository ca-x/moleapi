mod a2a;
mod auth;
mod cookies;
mod entities;
mod execution;
mod formats;
mod generation;
mod generation_projects;
mod graphql;
mod grpc;
mod history;
mod mock;
mod oauth1;
mod oauth2;
mod privacy;
mod protocol_admission;
mod protocols;
mod runner;
mod soap;
mod storage;
mod sync;
mod webhooks;
mod workspaces;

use axum::{
    Json, Router,
    extract::DefaultBodyLimit,
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
};
pub fn dispatch_script_worker() -> anyhow::Result<bool> {
    if moleapi_generation::project::dispatch_project_worker(
        moleapi_data::limit_headless_worker_heap,
    )? || moleapi_script_runtime::dispatch_worker()?
    {
        Ok(true)
    } else {
        moleapi_data::dispatch_file_worker()
    }
}
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
    generation_slots: Arc<tokio::sync::Semaphore>,
    project_slots: Arc<tokio::sync::Semaphore>,
    project_jobs: Arc<generation_projects::Jobs>,
    project_runtime: Arc<moleapi_generation::project::ProjectRuntime>,
    cookies: Arc<cookies::Jars>,
    oauth1_slots: Arc<tokio::sync::Semaphore>,
    oauth1_flows: Arc<oauth1::flows::Hub>,
    oauth2_slots: Arc<tokio::sync::Semaphore>,
    oauth2_flows: Arc<oauth2::flows::Hub>,
    script_worker: PathBuf,
    protocol_sessions: Arc<moleapi_protocols::SessionManager>,
    protocol_admission: Arc<protocol_admission::AdmissionGates>,
    a2a_sources: Arc<a2a::Sources>,
    webhooks: Arc<webhooks::Hub>,
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
        generation_slots: Arc::new(tokio::sync::Semaphore::new(4)),
        project_slots: Arc::new(tokio::sync::Semaphore::new(2)),
        project_jobs: Arc::default(),
        project_runtime: Arc::new(
            moleapi_generation::project::ProjectRuntime::from_environment(script_worker.clone())?,
        ),
        cookies: Arc::default(),
        oauth1_slots: Arc::new(tokio::sync::Semaphore::new(8)),
        oauth1_flows: Arc::default(),
        oauth2_slots: Arc::new(tokio::sync::Semaphore::new(8)),
        oauth2_flows: Arc::default(),
        script_worker,
        protocol_sessions: moleapi_protocols::SessionManager::new(),
        protocol_admission: Arc::new(protocol_admission::AdmissionGates::default()),
        a2a_sources: Arc::new(a2a::Sources::default()),
        webhooks: Arc::new(webhooks::Hub::default()),
    };
    let protected = Router::new()
        .route("/oauth1/flows", post(oauth1::flows::begin))
        .route("/oauth1/flows/{id}", get(oauth1::flows::status))
        .route("/oauth1/flows/{id}/cancel", post(oauth1::flows::cancel))
        .route("/oauth1/flows/{id}/complete", post(oauth1::flows::complete))
        .route("/oauth1/tokens/import", post(oauth1::import))
        .route("/workspaces/{id}/oauth1/tokens", get(oauth1::list))
        .route(
            "/workspaces/{workspace}/oauth1/tokens/{token}/secret",
            get(oauth1::reveal),
        )
        .route(
            "/workspaces/{workspace}/oauth1/tokens/{token}",
            axum::routing::delete(oauth1::remove).patch(oauth1::rename),
        )
        .route(
            "/workspaces/{id}/cookies",
            get(cookies::list)
                .patch(cookies::configure)
                .post(cookies::insert)
                .delete(cookies::remove),
        )
        .route(
            "/workspaces/{id}/webhooks",
            get(webhooks::list).post(webhooks::create),
        )
        .route(
            "/webhooks/{id}",
            get(webhooks::get)
                .put(webhooks::update)
                .delete(webhooks::delete),
        )
        .route(
            "/webhooks/{id}/captures",
            get(webhooks::captures).delete(webhooks::clear),
        )
        .route(
            "/webhooks/{id}/captures/{capture_id}",
            get(webhooks::capture_get),
        )
        .route("/webhooks/{id}/export", post(webhooks::export))
        .route("/webhooks/{id}/replay", post(webhooks::replay))
        .route("/webhooks/replay/cancel", post(webhooks::replay_cancel))
        .route("/webhooks/listener", get(webhooks::listener_status))
        .route("/webhooks/listener/start", post(webhooks::listener_start))
        .route("/webhooks/listener/stop", post(webhooks::listener_stop))
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
        .route("/oauth2/flows", post(oauth2::flows::begin))
        .route("/oauth2/flows/{id}", get(oauth2::flows::status))
        .route("/oauth2/flows/{id}/cancel", post(oauth2::flows::cancel))
        .route("/oauth2/flows/{id}/complete", post(oauth2::flows::complete))
        .route("/oauth2/tokens/acquire", post(oauth2::acquire))
        .route("/oauth2/tokens/import", post(oauth2::token_actions::import))
        .route(
            "/workspaces/{workspace}/oauth2/tokens/{token}/refresh",
            post(oauth2::refresh),
        )
        .route(
            "/workspaces/{workspace}/oauth2/tokens/{token}/secret",
            get(oauth2::reveal),
        )
        .route(
            "/workspaces/{workspace}/oauth2/tokens/{token}/revoke",
            post(oauth2::token_actions::revoke),
        )
        .route(
            "/workspaces/{workspace}/oauth2/tokens/{token}/introspect",
            post(oauth2::token_actions::introspect),
        )
        .route("/workspaces/{id}/oauth2/tokens", get(oauth2::list))
        .route(
            "/workspaces/{workspace}/oauth2/tokens/{token}",
            axum::routing::delete(oauth2::remove).patch(oauth2::rename),
        )
        .route("/execute", post(execution::execute))
        .route(
            "/generation/projects/catalog",
            get(generation_projects::catalog),
        )
        .route("/generation/projects", post(generation_projects::generate))
        .route(
            "/generation/projects/regenerate",
            post(generation_projects::regenerate),
        )
        .route(
            "/generation/projects/cancel",
            post(generation_projects::cancel),
        )
        .route("/generation/snippets/catalog", get(generation::catalog))
        .route("/generation/snippets", post(generation::generate))
        .route("/soap/import", post(soap::import))
        .route("/soap/import-url", post(soap::import_url))
        .route("/soap/schema", post(soap::schema))
        .route("/soap/template", post(soap::template))
        .route("/a2a/cards/discover/cancel", post(a2a::cancel_discovery))
        .route("/a2a/cards/import", post(a2a::import))
        .route("/a2a/cards/discover", post(a2a::discover))
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
    let receiver = Router::new()
        .route("/hooks/{token}", axum::routing::any(webhooks::ingest))
        .with_state(webhooks::ReceiverState {
            db: state.db.clone(),
            gates: state.webhooks.gates.clone(),
            intake: state.webhooks.intake.clone(),
            stop: tokio_util::sync::CancellationToken::new(),
        });
    let router = Router::new()
        .merge(oauth2::callbacks::router())
        .merge(oauth1::callbacks::router())
        .merge(receiver)
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
