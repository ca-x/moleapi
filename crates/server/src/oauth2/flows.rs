//! Owner-bound, expiring, single-use browser/device grant state.
use super::{GrantInput, configuration, policy, vault};
use crate::{ApiError, AppState, auth::Identity, workspaces::owned};
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::HeaderMap,
};
use moleapi_core::{OAuth2Auth, OAuth2Grant};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
#[derive(Default)]
pub(crate) struct Hub {
    flows: Mutex<HashMap<String, Arc<Flow>>>,
}
pub(super) struct Flow {
    owner: String,
    workspace: String,
    label: String,
    config: OAuth2Auth,
    headers: HeaderMap,
    verify_tls: bool,
    gate: Arc<tokio::sync::Mutex<u64>>,
    epoch: u64,
    pub(super) state: Option<String>,
    verifier: Mutex<Option<oauth2::PkceCodeVerifier>>,
    consumed: Mutex<bool>,
    completion: tokio::sync::Mutex<()>,
    cancel: tokio_util::sync::CancellationToken,
    status: Mutex<Status>,
}
#[derive(Clone, Serialize)]
pub(crate) struct Status {
    pub id: String,
    pub stage: String,
    pub authorization_url: Option<String>,
    pub verification_uri: Option<String>,
    pub user_code: Option<String>,
    pub expires_at: i64,
    pub token_id: Option<String>,
    pub error: Option<String>,
}
impl Hub {
    fn insert(&self, flow: Arc<Flow>) -> Result<(), ApiError> {
        let mut flows = self.flows.lock().unwrap();
        let now = chrono::Utc::now().timestamp();
        flows.retain(|_, f| {
            if f.status.lock().unwrap().expires_at < now {
                f.cancel.cancel();
                false
            } else {
                true
            }
        });
        if flows.len() >= 64 || flows.values().filter(|f| f.owner == flow.owner).count() >= 8 {
            return Err(ApiError {
                status: axum::http::StatusCode::TOO_MANY_REQUESTS,
                message: "OAuth2 flow capacity reached".into(),
            });
        }
        let id = flow.status.lock().unwrap().id.clone();
        flows.insert(id, flow);
        Ok(())
    }
    pub(super) fn get(&self, owner: &str, id: &str) -> Result<Arc<Flow>, ApiError> {
        let flow = self
            .flows
            .lock()
            .unwrap()
            .get(id)
            .filter(|f| f.owner == owner)
            .cloned()
            .ok_or_else(ApiError::not_found)?;
        if flow.status.lock().unwrap().expires_at <= chrono::Utc::now().timestamp() {
            flow.cancel.cancel();
            return Err(ApiError::bad("OAuth2 flow expired"));
        }
        Ok(flow)
    }
    pub(super) fn callback(&self, state: &str) -> Result<Arc<Flow>, ApiError> {
        if state.is_empty() || state.len() > 256 {
            return Err(ApiError::not_found());
        }
        let flow = self
            .flows
            .lock()
            .unwrap()
            .values()
            .find(|flow| flow.state.as_deref() == Some(state))
            .cloned()
            .ok_or_else(ApiError::not_found)?;
        if flow.status.lock().unwrap().expires_at <= chrono::Utc::now().timestamp() {
            return Err(ApiError::not_found());
        }
        Ok(flow)
    }
    pub fn cancel_workspace(&self, owner: &str, workspace: &str) {
        for flow in self
            .flows
            .lock()
            .unwrap()
            .values()
            .filter(|flow| flow.owner == owner && flow.workspace == workspace)
        {
            flow.cancel.cancel();
        }
    }
    pub fn cancel_owner(&self, owner: &str) {
        for flow in self
            .flows
            .lock()
            .unwrap()
            .values()
            .filter(|f| f.owner == owner)
        {
            flow.cancel.cancel();
        }
    }
}
#[derive(Deserialize)]
pub(crate) struct Begin {
    #[serde(flatten)]
    input: GrantInput,
    #[serde(default)]
    callback_mode: super::callbacks::Mode,
}
pub(crate) async fn begin(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    headers: HeaderMap,
    Json(input): Json<Begin>,
) -> Result<Json<Status>, ApiError> {
    let callback_mode = input.callback_mode;
    let input = input.input;
    if input.label.len() > 128 || input.label.chars().any(char::is_control) {
        return Err(ApiError::bad("Invalid OAuth2 token label"));
    }
    let config = configuration(&s, &owner.0, &input).await?;
    let listener = super::callbacks::listener(&s, &config, callback_mode).await?;
    let gate = s.protocol_admission.owner(&owner.0)?;
    let epoch = *gate.lock().await;
    let id = uuid::Uuid::new_v4().to_string();
    let mut status = Status {
        id: id.clone(),
        stage: "pending".into(),
        authorization_url: None,
        verification_uri: None,
        user_code: None,
        expires_at: chrono::Utc::now().timestamp() + 600,
        token_id: None,
        error: None,
    };
    let (state, verifier, device) = match config.grant {
        OAuth2Grant::AuthorizationCode | OAuth2Grant::Implicit => {
            let authorization = moleapi_core::oauth2_authorize(&config)
                .map_err(|e| ApiError::bad(e.to_string()))?;
            status.authorization_url = Some(authorization.url);
            (
                Some(authorization.state.secret().clone()),
                authorization.verifier,
                None,
            )
        }
        OAuth2Grant::DeviceCode => {
            let _permit = s
                .oauth2_slots
                .clone()
                .try_acquire_owned()
                .map_err(|_| ApiError::bad("OAuth2 grant capacity reached"))?;
            let response = moleapi_core::oauth2_device_start(&config, policy(&s), input.verify_tls)
                .await
                .map_err(|e| ApiError::bad(e.to_string()))?;
            status.verification_uri = Some(
                response
                    .verification_uri_complete()
                    .map(|u| u.secret().clone())
                    .unwrap_or_else(|| response.verification_uri().as_str().into()),
            );
            status.user_code = Some(response.user_code().secret().clone());
            status.expires_at = status.expires_at.min(
                chrono::Utc::now().timestamp()
                    + i64::try_from(response.expires_in().as_secs().min(600)).unwrap(),
            );
            (None, None, Some(response))
        }
        _ => {
            return Err(ApiError::bad(
                "This OAuth2 grant uses direct token acquisition",
            ));
        }
    };
    let flow = Arc::new(Flow {
        owner: owner.0,
        workspace: input.workspace_id,
        label: if input.label.is_empty() {
            "OAuth2 token".into()
        } else {
            input.label
        },
        config,
        headers,
        verify_tls: input.verify_tls,
        gate,
        epoch,
        state,
        verifier: Mutex::new(verifier),
        consumed: Mutex::new(false),
        completion: tokio::sync::Mutex::new(()),
        cancel: tokio_util::sync::CancellationToken::new(),
        status: Mutex::new(status.clone()),
    });
    let admission = flow.gate.lock().await;
    if *admission != flow.epoch {
        return Err(ApiError::bad("OAuth2 account context changed"));
    }
    crate::auth::still_authenticated(&s, &flow.owner, &flow.headers).await?;
    owned(&s, &flow.owner, &flow.workspace).await?;
    s.oauth2_flows.insert(flow.clone())?;
    if let Err(error) = owned(&s, &flow.owner, &flow.workspace).await {
        flow.cancel.cancel();
        return Err(error);
    }
    drop(admission);
    if let Some(listener) = listener {
        let router = super::callbacks::router().with_state(s.clone());
        let cancellation = flow.cancel.clone();
        tokio::spawn(async move {
            let shutdown = async move {
                tokio::select! { _ = cancellation.cancelled() => {}, _ = tokio::time::sleep(Duration::from_secs(600)) => {} }
            };
            let _ = axum::serve(listener, router)
                .with_graceful_shutdown(shutdown)
                .await;
        });
    }
    if let Some(device) = device {
        let state = s.clone();
        tokio::spawn(async move {
            let Ok(_permit) = state.oauth2_slots.clone().try_acquire_owned() else {
                finish(
                    &state,
                    &flow,
                    Err(anyhow::anyhow!("OAuth2 grant capacity reached")),
                )
                .await;
                return;
            };
            let remaining = flow
                .status
                .lock()
                .unwrap()
                .expires_at
                .saturating_sub(chrono::Utc::now().timestamp())
                .max(1) as u64;
            let result = tokio::select! {biased;_=flow.cancel.cancelled()=>Err(anyhow::anyhow!("OAuth2 flow cancelled")),response=moleapi_core::oauth2_device_poll(&flow.config,&device,policy(&state),flow.verify_tls,Duration::from_secs(remaining))=>response};
            finish(&state, &flow, result).await;
        });
    }
    Ok(Json(status))
}
async fn finish(
    s: &AppState,
    flow: &Arc<Flow>,
    response: anyhow::Result<oauth2::basic::BasicTokenResponse>,
) {
    let _completion = flow.completion.lock().await;
    let outcome = async {
        let response = response.map_err(|e| ApiError::bad(e.to_string()))?;
        let admission = flow.gate.lock().await;
        if *admission != flow.epoch
            || flow.cancel.is_cancelled()
            || flow.status.lock().unwrap().expires_at <= chrono::Utc::now().timestamp()
        {
            return Err(ApiError::bad(
                "OAuth2 flow expired, cancelled or changed account",
            ));
        }
        crate::auth::still_authenticated(s, &flow.owner, &flow.headers).await?;
        owned(s, &flow.owner, &flow.workspace).await?;
        vault::create(
            s,
            &flow.owner,
            &flow.workspace,
            flow.label.clone(),
            &flow.config,
            response,
        )
        .await
    }
    .await;
    let mut status = flow.status.lock().unwrap();
    match outcome {
        Ok(token) => {
            status.stage = "completed".into();
            status.token_id = Some(token.id);
        }
        Err(error) => {
            status.stage = if flow.cancel.is_cancelled() {
                "cancelled"
            } else {
                "failed"
            }
            .into();
            status.error = Some(error.message);
        }
    }
    flow.cancel.cancel();
}
pub(crate) async fn status(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
) -> Result<Json<Status>, ApiError> {
    let flow = s.oauth2_flows.get(&owner.0, &id)?;
    owned(&s, &owner.0, &flow.workspace).await?;
    let status = flow.status.lock().unwrap().clone();
    Ok(Json(status))
}
pub(crate) async fn cancel(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
) -> Result<Json<Status>, ApiError> {
    let flow = s.oauth2_flows.get(&owner.0, &id)?;
    owned(&s, &owner.0, &flow.workspace).await?;
    let _completion = flow.completion.lock().await;
    flow.cancel.cancel();
    let mut status = flow.status.lock().unwrap();
    if status.stage != "completed" {
        status.stage = "cancelled".into();
    }
    Ok(Json(status.clone()))
}
#[derive(Deserialize)]
pub(crate) struct Complete {
    pub(super) state: String,
    pub(super) code: Option<String>,
    pub(super) implicit: Option<serde_json::Value>,
    pub(super) error: Option<String>,
}
pub(crate) async fn complete(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
    Json(input): Json<Complete>,
) -> Result<Json<Status>, ApiError> {
    let flow = s.oauth2_flows.get(&owner.0, &id)?;
    owned(&s, &owner.0, &flow.workspace).await?;
    complete_flow(&s, &flow, input).await
}
pub(super) async fn complete_flow(
    s: &AppState,
    flow: &Arc<Flow>,
    input: Complete,
) -> Result<Json<Status>, ApiError> {
    if input.state.len() > 256
        || flow.state.as_deref() != Some(input.state.as_str())
        || flow.cancel.is_cancelled()
    {
        return Err(ApiError::bad(
            "OAuth2 callback state mismatch or cancellation",
        ));
    }
    if input.code.as_ref().is_some_and(|code| {
        code.is_empty() || code.len() > 8192 || code.chars().any(char::is_control)
    }) {
        return Err(ApiError::bad("Invalid OAuth2 authorization code"));
    }
    let _permit = s
        .oauth2_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::bad("OAuth2 grant capacity reached"))?;
    {
        let mut consumed = flow.consumed.lock().unwrap();
        if *consumed {
            return Err(ApiError::bad("OAuth2 callback was already consumed"));
        }
        *consumed = true;
    }
    flow.status.lock().unwrap().stage = "running".into();
    let result = if input.error.is_some() {
        Err(anyhow::anyhow!(
            "OAuth2 authorization was rejected by the provider"
        ))
    } else {
        match flow.config.grant {
            OAuth2Grant::AuthorizationCode => {
                let code = input.code.unwrap_or_default();
                let verifier = flow.verifier.lock().unwrap().take();
                tokio::select! {biased;_=flow.cancel.cancelled()=>Err(anyhow::anyhow!("OAuth2 flow cancelled")),result=moleapi_core::oauth2_exchange_code(&flow.config,&code,verifier,policy(s),flow.verify_tls)=>result}
            }
            OAuth2Grant::Implicit => input
                .implicit
                .ok_or_else(|| anyhow::anyhow!("OAuth2 implicit response missing"))
                .and_then(|value| {
                    serde_json::from_value(value)
                        .map_err(|_| anyhow::anyhow!("Invalid OAuth2 implicit token response"))
                }),
            _ => {
                return Err(ApiError::bad(
                    "OAuth2 flow does not accept callback completion",
                ));
            }
        }
    };
    finish(s, flow, result).await;
    let status = flow.status.lock().unwrap().clone();
    Ok(Json(status))
}
