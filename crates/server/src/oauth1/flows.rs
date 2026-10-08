//! Expiring, owner-fenced and single-use OAuth1 authorization state.
use super::{
    callbacks,
    configuration::{self, Input, Profile},
    policy, vault,
};
use crate::{ApiError, AppState, auth::Identity, workspaces::owned};
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::HeaderMap,
};
use moleapi_core::{OAuth1Auth, OAuth1Credentials, OAuth1Grant};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio_util::sync::CancellationToken;
#[derive(Default)]
pub(crate) struct Hub {
    flows: Mutex<HashMap<String, Arc<Flow>>>,
}
pub(super) struct Flow {
    owner: String,
    workspace: String,
    label: String,
    profile: Profile,
    grant: OAuth1Grant,
    config: Mutex<Option<OAuth1Auth>>,
    temporary: Mutex<Option<OAuth1Credentials>>,
    session: HeaderMap,
    verify_tls: bool,
    state: String,
    gate: Arc<tokio::sync::Mutex<u64>>,
    epoch: u64,
    completion: tokio::sync::Mutex<()>,
    consumed: Mutex<bool>,
    pub(super) stop: CancellationToken,
    status: Mutex<Status>,
}
#[derive(Clone, Serialize)]
pub(crate) struct Status {
    pub id: String,
    pub stage: String,
    pub authorization_url: Option<String>,
    pub expires_at: i64,
    pub token_id: Option<String>,
    pub error: Option<String>,
}
impl Flow {
    fn expired(&self) -> bool {
        self.status.lock().unwrap().expires_at <= chrono::Utc::now().timestamp()
    }
    fn clear_secrets(&self) {
        self.config.lock().unwrap().take();
        self.temporary.lock().unwrap().take();
    }
    fn cancelled(&self) {
        self.stop.cancel();
        self.clear_secrets();
        let mut status = self.status.lock().unwrap();
        if status.stage != "completed" {
            status.stage = "cancelled".into();
        }
    }
}
impl Hub {
    fn insert(&self, flow: Arc<Flow>) -> Result<(), ApiError> {
        let mut flows = self.flows.lock().unwrap();
        let now = chrono::Utc::now().timestamp();
        flows.retain(|_, f| {
            let expired = f.status.lock().unwrap().expires_at <= now;
            if expired {
                f.cancelled();
                false
            } else {
                true
            }
        });
        if flows.len() >= 64 || flows.values().filter(|f| f.owner == flow.owner).count() >= 8 {
            return Err(ApiError::bad("OAuth1 flow capacity reached"));
        }
        let id = flow.status.lock().unwrap().id.clone();
        flows.insert(id, flow);
        Ok(())
    }
    fn get(&self, owner: &str, id: &str) -> Result<Arc<Flow>, ApiError> {
        let flow = self
            .flows
            .lock()
            .unwrap()
            .get(id)
            .filter(|f| f.owner == owner)
            .cloned()
            .ok_or_else(ApiError::not_found)?;
        if flow.expired() {
            flow.cancelled();
            return Err(ApiError::bad("OAuth1 flow expired"));
        }
        Ok(flow)
    }
    pub(super) fn callback(&self, state: &str) -> Result<Arc<Flow>, ApiError> {
        if state.len() != 43 {
            return Err(ApiError::not_found());
        }
        let flow = self
            .flows
            .lock()
            .unwrap()
            .values()
            .find(|f| f.state == state)
            .cloned()
            .ok_or_else(ApiError::not_found)?;
        if flow.stop.is_cancelled()
            || flow.status.lock().unwrap().expires_at <= chrono::Utc::now().timestamp()
        {
            return Err(ApiError::not_found());
        }
        Ok(flow)
    }
    pub fn cancel_owner(&self, owner: &str) {
        for flow in self
            .flows
            .lock()
            .unwrap()
            .values()
            .filter(|f| f.owner == owner)
        {
            flow.cancelled();
        }
    }
    pub fn cancel_workspace(&self, owner: &str, workspace: &str) {
        for flow in self
            .flows
            .lock()
            .unwrap()
            .values()
            .filter(|f| f.owner == owner && f.workspace == workspace)
        {
            flow.cancelled();
        }
    }
}
#[derive(Deserialize)]
pub(crate) struct Begin {
    #[serde(flatten)]
    input: Input,
    #[serde(default)]
    callback_mode: callbacks::Mode,
}
pub(crate) async fn begin(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    headers: HeaderMap,
    Json(input): Json<Begin>,
) -> Result<Json<Status>, ApiError> {
    let gate = s.protocol_admission.owner(&owner.0)?;
    let admission = gate.lock().await;
    crate::auth::still_authenticated(&s, &owner.0, &headers).await?;
    let _workspace = s
        .webhooks
        .gates
        .lock(&owner.0, &input.input.workspace_id)
        .await;
    owned(&s, &owner.0, &input.input.workspace_id).await?;
    let resolved = configuration::resolve(&s, &owner.0, &input.input, true).await?;
    let permit = s
        .oauth1_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::bad("OAuth1 exchange capacity reached"))?;
    use base64::Engine;
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    let state = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
    let (callback, listener) = callbacks::prepare(
        &s,
        &resolved.grant.callback_url,
        input.callback_mode,
        &state,
    )
    .await?;
    let status = Status {
        id: uuid::Uuid::new_v4().to_string(),
        stage: "requesting".into(),
        authorization_url: None,
        expires_at: chrono::Utc::now().timestamp() + 600,
        token_id: None,
        error: None,
    };
    let flow = Arc::new(Flow {
        owner: owner.0,
        workspace: input.input.workspace_id,
        label: resolved.label,
        profile: resolved.profile,
        grant: resolved.grant,
        config: Mutex::new(Some(resolved.auth)),
        temporary: Mutex::new(None),
        session: headers,
        verify_tls: input.input.verify_tls,
        state,
        gate: gate.clone(),
        epoch: *admission,
        completion: tokio::sync::Mutex::new(()),
        consumed: Mutex::new(false),
        stop: CancellationToken::new(),
        status: Mutex::new(status.clone()),
    });
    s.oauth1_flows.insert(flow.clone())?;
    if let Some(listener) = listener {
        let router = callbacks::router().with_state(s.clone());
        let stop = flow.stop.clone();
        tokio::spawn(async move {
            let _ = axum::serve(listener, router)
                .with_graceful_shutdown(stop.cancelled_owned())
                .await;
        });
    }
    let cleanup = flow.clone();
    let hub = Arc::downgrade(&s.oauth1_flows);
    let id = status.id.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(600)).await;
        let _completion = cleanup.completion.lock().await;
        cleanup.cancelled();
        if let Some(hub) = hub.upgrade() {
            hub.flows.lock().unwrap().remove(&id);
        }
    });
    let state = s.clone();
    let init = flow.clone();
    tokio::spawn(async move {
        let _permit = permit;
        let config = init.config.lock().unwrap().clone();
        let result = match config {
            Some(c) => {
                tokio::select! {biased;_=init.stop.cancelled()=>Err(anyhow::anyhow!("OAuth1 flow cancelled")),value=moleapi_core::oauth1_request_token(&c,&init.grant,&callback,policy(&state),init.verify_tls)=>value}
            }
            None => Err(anyhow::anyhow!("OAuth1 flow cancelled")),
        };
        let _completion = init.completion.lock().await;
        let outcome = async {
            let temporary = result.map_err(|e| ApiError::bad(e.to_string()))?;
            let admission = init.gate.lock().await;
            if *admission != init.epoch || init.stop.is_cancelled() {
                return Err(ApiError::bad("OAuth1 account context changed"));
            }
            crate::auth::still_authenticated(&state, &init.owner, &init.session).await?;
            let _workspace = state
                .webhooks
                .gates
                .lock(&init.owner, &init.workspace)
                .await;
            owned(&state, &init.owner, &init.workspace).await?;
            if init.stop.is_cancelled() || init.expired() {
                return Err(ApiError::bad("OAuth1 flow expired or cancelled"));
            }
            let mut url = moleapi_core::valid_url(&init.grant.authorization_url)
                .map_err(|e| ApiError::bad(e.to_string()))?;
            if url.query_pairs().any(|(k, _)| k == "oauth_token") {
                return Err(ApiError::bad(
                    "OAuth1 authorization endpoint owns oauth_token",
                ));
            }
            url.query_pairs_mut()
                .append_pair("oauth_token", &temporary.token);
            *init.temporary.lock().unwrap() = Some(temporary);
            let mut status = init.status.lock().unwrap();
            status.stage = "pending".into();
            status.authorization_url = Some(url.to_string());
            Ok::<_, ApiError>(())
        }
        .await;
        if let Err(error) = outcome {
            init.clear_secrets();
            let mut status = init.status.lock().unwrap();
            if status.stage != "cancelled" {
                status.stage = "failed".into();
                status.error = Some(error.message);
            }
            init.stop.cancel();
        }
    });
    Ok(Json(status))
}
pub(crate) async fn status(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
) -> Result<Json<Status>, ApiError> {
    let flow = s.oauth1_flows.get(&owner.0, &id)?;
    owned(&s, &owner.0, &flow.workspace).await?;
    let status = flow.status.lock().unwrap().clone();
    Ok(Json(status))
}
pub(crate) async fn cancel(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
) -> Result<Json<Status>, ApiError> {
    let flow = s.oauth1_flows.get(&owner.0, &id)?;
    owned(&s, &owner.0, &flow.workspace).await?;
    let _completion = flow.completion.lock().await;
    flow.cancelled();
    let status = flow.status.lock().unwrap().clone();
    Ok(Json(status))
}
#[derive(Deserialize)]
pub(crate) struct Complete {
    pub verifier: String,
    pub token: Option<String>,
    pub state: Option<String>,
}
pub(crate) async fn complete(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<Complete>,
) -> Result<Json<Status>, ApiError> {
    let flow = s.oauth1_flows.get(&owner.0, &id)?;
    crate::auth::still_authenticated(&s, &owner.0, &headers).await?;
    owned(&s, &owner.0, &flow.workspace).await?;
    Ok(Json(complete_flow(&s, &flow, input).await?))
}
pub(super) async fn complete_flow(
    s: &AppState,
    flow: &Arc<Flow>,
    input: Complete,
) -> Result<Status, ApiError> {
    if input.verifier.is_empty()
        || input.verifier.len() > 4096
        || input.verifier.chars().any(char::is_control)
        || input
            .state
            .as_ref()
            .is_some_and(|state| state != &flow.state)
    {
        return Err(ApiError::bad("Invalid OAuth1 callback state or verifier"));
    }
    let permit = s
        .oauth1_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::bad("OAuth1 exchange capacity reached"))?;
    let (config, temporary) = {
        let _completion = flow.completion.lock().await;
        if flow.stop.is_cancelled()
            || flow.status.lock().unwrap().expires_at <= chrono::Utc::now().timestamp()
        {
            return Err(ApiError::bad("OAuth1 flow expired or cancelled"));
        }
        if *flow.consumed.lock().unwrap() {
            return Err(ApiError::bad("OAuth1 callback was already consumed"));
        }
        let token = flow
            .temporary
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| ApiError::bad("OAuth1 request token is not ready"))?;
        if input
            .token
            .as_ref()
            .is_some_and(|value| value != &token.token)
        {
            return Err(ApiError::bad("OAuth1 callback token mismatch"));
        }
        let config = flow
            .config
            .lock()
            .unwrap()
            .take()
            .ok_or_else(|| ApiError::bad("OAuth1 flow cancelled"))?;
        flow.temporary.lock().unwrap().take();
        *flow.consumed.lock().unwrap() = true;
        flow.status.lock().unwrap().stage = "running".into();
        (config, token)
    };
    let result = tokio::select! {biased;_=flow.stop.cancelled()=>Err(anyhow::anyhow!("OAuth1 flow cancelled")),value=moleapi_core::oauth1_access_token(&config,&flow.grant,&temporary,&input.verifier,policy(s),flow.verify_tls)=>value};
    drop(permit);
    let _completion = flow.completion.lock().await;
    let outcome = async {
        let credentials = result.map_err(|e| ApiError::bad(e.to_string()))?;
        let admission = flow.gate.lock().await;
        if *admission != flow.epoch
            || flow.stop.is_cancelled()
            || flow.status.lock().unwrap().expires_at <= chrono::Utc::now().timestamp()
        {
            return Err(ApiError::bad(
                "OAuth1 flow expired, cancelled or changed account",
            ));
        }
        crate::auth::still_authenticated(s, &flow.owner, &flow.session).await?;
        let _workspace = s.webhooks.gates.lock(&flow.owner, &flow.workspace).await;
        owned(s, &flow.owner, &flow.workspace).await?;
        vault::create(
            s,
            &flow.owner,
            &flow.workspace,
            flow.label.clone(),
            &flow.profile,
            credentials,
        )
        .await
    }
    .await;
    flow.clear_secrets();
    let mut status = flow.status.lock().unwrap();
    match outcome {
        Ok(token) => {
            status.stage = "completed".into();
            status.token_id = Some(token.id);
        }
        Err(error) => {
            if status.stage != "cancelled" {
                status.stage = "failed".into();
                status.error = Some(error.message);
            }
        }
    }
    flow.stop.cancel();
    Ok(status.clone())
}

pub(super) async fn reject(flow: &Arc<Flow>, token: Option<&str>) -> Result<(), ApiError> {
    let _completion = flow.completion.lock().await;
    if flow.stop.is_cancelled() || flow.expired() || *flow.consumed.lock().unwrap() {
        return Err(ApiError::bad("OAuth1 flow already finished"));
    }
    let temporary = flow
        .temporary
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| ApiError::bad("OAuth1 request token is not ready"))?;
    if token.is_some_and(|token| token != temporary.token) {
        return Err(ApiError::bad("OAuth1 callback token mismatch"));
    }
    *flow.consumed.lock().unwrap() = true;
    flow.clear_secrets();
    flow.stop.cancel();
    let mut status = flow.status.lock().unwrap();
    status.stage = "failed".into();
    status.error = Some("OAuth1 authorization was rejected by the provider".into());
    Ok(())
}
