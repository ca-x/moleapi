//! Private in-memory jars: never part of workspace sync, exports or snapshots.
use crate::{ApiError, AppState, auth::Identity, workspaces::owned};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
};
use moleapi_core::{CookieJar, CookieSnapshot};
use serde::Deserialize;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
type Scope = (String, String, Option<String>);
#[derive(Default)]
pub(crate) struct Jars(Mutex<HashMap<Scope, Arc<CookieJar>>>);
impl Jars {
    pub fn get(
        &self,
        owner: &str,
        workspace: &str,
        environment: Option<&str>,
    ) -> Result<Arc<CookieJar>, ApiError> {
        let mut jars = self.0.lock().unwrap();
        let key = (
            owner.into(),
            workspace.into(),
            environment.map(str::to_owned),
        );
        if let Some(jar) = jars.get(&key) {
            return Ok(jar.clone());
        }
        if jars.len() >= 1024 {
            return Err(ApiError::bad("Cookie jar capacity reached (1024 scopes)"));
        }
        let jar = Arc::new(CookieJar::default());
        jars.insert(key, jar.clone());
        Ok(jar)
    }
    pub fn lookup(
        &self,
        owner: &str,
        workspace: &str,
        environment: Option<&str>,
    ) -> Option<Arc<CookieJar>> {
        self.0
            .lock()
            .unwrap()
            .get(&(
                owner.into(),
                workspace.into(),
                environment.map(str::to_owned),
            ))
            .cloned()
    }
    pub fn clear_scope(&self, owner: &str, workspace: Option<&str>) {
        self.0.lock().unwrap().retain(|(o, w, _), jar| {
            if o == owner && workspace.is_none_or(|id| id == w) {
                jar.configure(false, true);
                false
            } else {
                true
            }
        });
    }
}
#[derive(Default, Deserialize)]
pub(crate) struct ScopeQuery {
    environment_id: Option<String>,
    #[serde(default)]
    reveal: bool,
}
struct Access {
    jar: Arc<CookieJar>,
    _owner: tokio::sync::OwnedMutexGuard<u64>,
    _workspace: tokio::sync::OwnedMutexGuard<()>,
}
impl std::ops::Deref for Access {
    type Target = CookieJar;
    fn deref(&self) -> &CookieJar {
        &self.jar
    }
}
async fn jar(
    s: &AppState,
    owner: &str,
    workspace: &str,
    query: &ScopeQuery,
    headers: &axum::http::HeaderMap,
) -> Result<Access, ApiError> {
    let gate = s.protocol_admission.owner(owner)?;
    let admission = gate.lock_owned().await;
    crate::auth::still_authenticated(s, owner, headers).await?;
    let workspace_gate = s.webhooks.gates.lock(owner, workspace).await;
    let w = owned(s, owner, workspace).await?;
    let environment = crate::execution::environment(&w, query.environment_id.as_deref())?;
    Ok(Access {
        jar: s
            .cookies
            .get(owner, workspace, environment.map(|e| e.id.as_str()))?,
        _owner: admission,
        _workspace: workspace_gate,
    })
}
pub(crate) async fn list(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
    Query(query): Query<ScopeQuery>,
    headers: axum::http::HeaderMap,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    Ok((
        [("cache-control", "no-store")],
        Json(
            jar(&s, &owner.0, &id, &query, &headers)
                .await?
                .snapshot(query.reveal),
        ),
    ))
}
#[derive(Deserialize)]
pub(crate) struct Change {
    enabled: bool,
    #[serde(default)]
    clear: bool,
}
pub(crate) async fn configure(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
    Query(query): Query<ScopeQuery>,
    headers: axum::http::HeaderMap,
    Json(input): Json<Change>,
) -> Result<Json<CookieSnapshot>, ApiError> {
    let jar = jar(&s, &owner.0, &id, &query, &headers).await?;
    jar.configure(input.enabled, input.clear);
    Ok(Json(jar.snapshot(false)))
}
#[derive(Deserialize)]
pub(crate) struct Insert {
    url: String,
    cookie: String,
}
pub(crate) async fn insert(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
    Query(query): Query<ScopeQuery>,
    headers: axum::http::HeaderMap,
    Json(input): Json<Insert>,
) -> Result<Json<CookieSnapshot>, ApiError> {
    let jar = jar(&s, &owner.0, &id, &query, &headers).await?;
    let url = moleapi_core::valid_url(&input.url).map_err(|e| ApiError::bad(e.to_string()))?;
    jar.insert(&url, &input.cookie)
        .map_err(|e| ApiError::bad(e.to_string()))?;
    Ok(Json(jar.snapshot(false)))
}
#[derive(Deserialize)]
pub(crate) struct Remove {
    domain: String,
    path: String,
    name: String,
}
pub(crate) async fn remove(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
    Query(query): Query<ScopeQuery>,
    headers: axum::http::HeaderMap,
    Json(input): Json<Remove>,
) -> Result<Json<CookieSnapshot>, ApiError> {
    let jar = jar(&s, &owner.0, &id, &query, &headers).await?;
    jar.remove(&input.domain, &input.path, &input.name);
    Ok(Json(jar.snapshot(false)))
}
