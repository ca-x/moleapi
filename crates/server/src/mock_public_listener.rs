use crate::{ApiError, AppState, mock_public};
use axum::{
    Json, Router,
    extract::{Request, State},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub(crate) struct Hub {
    bridge: tokio::sync::Mutex<Option<Bridge>>,
}

struct Bridge {
    origin: String,
    bind: String,
    stop: CancellationToken,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Hub {
    fn drop(&mut self) {
        if let Some(bridge) = self.bridge.get_mut().take() {
            bridge.stop.cancel();
            bridge.task.abort();
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Start {
    #[serde(default = "loopback")]
    host: String,
    #[serde(default)]
    port: u16,
    #[serde(default)]
    allow_lan: bool,
}

fn loopback() -> String {
    "127.0.0.1".into()
}

fn local_hub(state: &AppState) -> Result<std::sync::Arc<Hub>, ApiError> {
    if !state.local {
        return Err(ApiError::not_found());
    }
    state
        .mock_public_listener
        .upgrade()
        .ok_or_else(ApiError::internal)
}

fn status_json(bridge: Option<&Bridge>) -> serde_json::Value {
    match bridge {
        Some(bridge) => {
            serde_json::json!({"active":true,"origin":bridge.origin,"bind":bridge.bind})
        }
        None => serde_json::json!({"active":false,"origin":null,"bind":null}),
    }
}

async fn fence(State(stop): State<CancellationToken>, request: Request, next: Next) -> Response {
    tokio::select! {
        biased;
        _ = stop.cancelled() => ApiError::not_found().into_response(),
        response = next.run(request) => response,
    }
}

pub async fn status(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    let hub = local_hub(&state)?;
    let mut bridge = hub.bridge.lock().await;
    if bridge
        .as_ref()
        .is_some_and(|bridge| bridge.task.is_finished())
    {
        bridge.take();
    }
    Ok(Json(status_json(bridge.as_ref())))
}

pub async fn start(
    State(state): State<AppState>,
    Json(input): Json<Start>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let hub = local_hub(&state)?;
    if !matches!(input.host.as_str(), "127.0.0.1" | "0.0.0.0")
        || (input.host == "0.0.0.0" && !input.allow_lan)
    {
        return Err(ApiError::bad(
            "Unsupported or unapproved public Mock bind address",
        ));
    }
    let mut bridge = hub.bridge.lock().await;
    if bridge
        .as_ref()
        .is_some_and(|bridge| bridge.task.is_finished())
    {
        bridge.take();
    }
    if bridge.is_some() {
        return Err(ApiError::conflict(
            "A native public Mock listener is already running",
        ));
    }
    let listener = tokio::net::TcpListener::bind((input.host.as_str(), input.port))
        .await
        .map_err(|_| ApiError::bad("Cannot bind native public Mock listener"))?;
    let address = listener.local_addr().map_err(|_| ApiError::internal())?;
    let origin = format!("http://127.0.0.1:{}", address.port());
    let bind = address.to_string();
    let stop = CancellationToken::new();
    let router: Router = Router::new()
        .nest("/api", mock_public::routes())
        .layer(middleware::from_fn_with_state(stop.clone(), fence))
        .with_state(state);
    let halted = stop.clone();
    let task = tokio::spawn(async move {
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(halted.cancelled_owned())
            .await;
    });
    *bridge = Some(Bridge {
        origin: origin.clone(),
        bind: bind.clone(),
        stop,
        task,
    });
    Ok(Json(
        serde_json::json!({"active":true,"origin":origin,"bind":bind}),
    ))
}

pub async fn stop(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    let hub = local_hub(&state)?;
    let mut slot = hub.bridge.lock().await;
    if let Some(mut bridge) = slot.take() {
        bridge.stop.cancel();
        if tokio::time::timeout(std::time::Duration::from_secs(2), &mut bridge.task)
            .await
            .is_err()
        {
            bridge.task.abort();
            let _ = bridge.task.await;
        }
    }
    Ok(Json(status_json(None)))
}
