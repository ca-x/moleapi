use super::{
    manager::Bridge,
    receiver::{ReceiverState, ingest},
};
use crate::{ApiError, AppState};
use axum::{Json, extract::State};
use serde::Deserialize;
use tokio_util::sync::CancellationToken;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Start {
    #[serde(default = "loopback")]
    pub host: String,
    #[serde(default)]
    pub port: u16,
}
fn loopback() -> String {
    "127.0.0.1".into()
}
pub async fn status(State(s): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    if !s.local {
        return Err(ApiError::bad(
            "Local listener control is available only in the independent native client",
        ));
    }
    let bridge = s.webhooks.bridge.lock().await;
    Ok(Json(match bridge.as_ref() {
        Some(b) => serde_json::json!({"active":true,"origin":b.origin,"bind":b.bind}),
        None => serde_json::json!({"active":false,"origin":null,"bind":null}),
    }))
}
pub async fn start(
    State(s): State<AppState>,
    Json(input): Json<Start>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if !s.local {
        return Err(ApiError::bad(
            "Hosted receivers use the existing server callback route",
        ));
    }
    if !matches!(input.host.as_str(), "127.0.0.1" | "0.0.0.0") {
        return Err(ApiError::bad("Unsupported local Webhook bind address"));
    }
    let mut bridge = s.webhooks.bridge.lock().await;
    if bridge.is_some() {
        return Err(ApiError::conflict(
            "A native Webhook listener is already running",
        ));
    }
    let listener = tokio::net::TcpListener::bind((input.host.as_str(), input.port))
        .await
        .map_err(|_| ApiError::bad("Cannot bind native Webhook listener"))?;
    let address = listener.local_addr().map_err(|_| ApiError::internal())?;
    let origin = format!("http://127.0.0.1:{}", address.port());
    let bind = address.to_string();
    let stop = CancellationToken::new();
    let receiver = ReceiverState {
        db: s.db.clone(),
        gates: s.webhooks.gates.clone(),
        intake: s.webhooks.intake.clone(),
        stop: stop.clone(),
    };
    let router = axum::Router::new()
        .route("/hooks/{token}", axum::routing::any(ingest))
        .with_state(receiver);
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
pub async fn stop(State(s): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    if !s.local {
        return Err(ApiError::bad(
            "Hosted server listener cannot be stopped through the Webhook API",
        ));
    }
    if let Some(mut bridge) = s.webhooks.bridge.lock().await.take() {
        bridge.stop.cancel();
        if tokio::time::timeout(std::time::Duration::from_secs(2), &mut bridge.task)
            .await
            .is_err()
        {
            bridge.task.abort();
            let _ = bridge.task.await;
        }
    }
    Ok(Json(
        serde_json::json!({"active":false,"origin":null,"bind":null}),
    ))
}
