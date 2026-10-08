//! Owner-bound card candidates; advertised metadata never triggers network access.
use crate::{ApiError, AppState, auth::Identity, execution, privacy::Redactor, workspaces::owned};
use axum::{Extension, Json, extract::State};
use moleapi_core::{RequestSpec, Specification, VariableUpdate};
use serde::{Deserialize, Serialize};
#[derive(Default)]
pub(crate) struct Sources {
    active: std::sync::Mutex<
        std::collections::HashMap<(String, String), tokio_util::sync::CancellationToken>,
    >,
}
struct Lease {
    sources: std::sync::Arc<Sources>,
    key: (String, String),
    token: tokio_util::sync::CancellationToken,
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.token.cancel();
        self.sources.active.lock().unwrap().remove(&self.key);
    }
}
impl Sources {
    fn start(self: &std::sync::Arc<Self>, owner: &str, id: &str) -> Result<Lease, ApiError> {
        if id.is_empty() || id.len() > 128 {
            return Err(ApiError::bad("Invalid discovery id"));
        }
        let key = (owner.to_owned(), id.to_owned());
        let mut active = self.active.lock().unwrap();
        if active.len() >= 32 || active.keys().filter(|key| key.0 == owner).count() >= 4 {
            return Err(ApiError {
                status: axum::http::StatusCode::TOO_MANY_REQUESTS,
                message: "A2A discovery capacity reached".into(),
            });
        }
        if active.contains_key(&key) {
            return Err(ApiError::bad("Discovery id already active"));
        }
        let token = tokio_util::sync::CancellationToken::new();
        active.insert(key.clone(), token.clone());
        Ok(Lease {
            sources: self.clone(),
            key,
            token,
        })
    }
}
#[derive(Deserialize)]
pub struct CancelDiscovery {
    workspace_id: String,
    discovery_id: String,
}
pub async fn cancel_discovery(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Json(c): Json<CancelDiscovery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    owned(&s, &owner.0, &c.workspace_id).await?;
    if let Some(token) = s
        .a2a_sources
        .active
        .lock()
        .unwrap()
        .get(&(owner.0, c.discovery_id))
    {
        token.cancel();
    }
    Ok(Json(serde_json::json!({"stopped":true})))
}
#[derive(Deserialize)]
pub struct Import {
    workspace_id: String,
    source: String,
    dialect: String,
    #[serde(default)]
    environment_id: Option<String>,
    #[serde(default)]
    locals: Vec<VariableUpdate>,
}
#[derive(Deserialize)]
pub struct Discover {
    #[serde(default)]
    discovery_id: Option<String>,
    workspace_id: String,
    request: RequestSpec,
    #[serde(default)]
    environment_id: Option<String>,
    #[serde(default)]
    locals: Vec<VariableUpdate>,
}
#[derive(Serialize)]
pub struct Candidate {
    specification: Specification,
    #[serde(flatten)]
    description: moleapi_protocols::a2a::CardDescription,
}
fn candidate(
    source: String,
    dialect: &str,
    values: &std::collections::BTreeSet<String>,
) -> Result<Candidate, ApiError> {
    if source.len() > moleapi_protocols::MAX_MESSAGE {
        return Err(ApiError::bad("Agent Card exceeds 1 MiB"));
    }
    let raw: serde_json::Value =
        serde_json::from_str(&source).map_err(|e| ApiError::bad(e.to_string()))?;
    moleapi_core::validate_mcp_json(&raw).map_err(|e| ApiError::bad(e.to_string()))?;
    let redactor = Redactor::new(values)?;
    let mut parsed = raw.clone();
    redactor.scrub(&mut parsed);
    let mut stack = vec![&raw];
    let mut private_structure = false;
    while let Some(value) = stack.pop() {
        match value {
            serde_json::Value::Object(items) => {
                for (key, value) in items {
                    let mut text = serde_json::json!(key);
                    redactor.scrub(&mut text);
                    private_structure |= text.as_str() != Some(key);
                    stack.push(value);
                }
            }
            serde_json::Value::Array(items) => stack.extend(items),
            _ => {
                if let Some(text) = value.as_str()
                    && let Ok(url) = url::Url::parse(text)
                    && matches!(url.scheme(), "http" | "https")
                    && (!url.username().is_empty()
                        || url.password().is_some()
                        || url.query().is_some())
                {
                    return Err(ApiError::bad(
                        "Agent Card URL contains userinfo or query metadata; candidate withheld",
                    ));
                }
                let mut text = serde_json::json!(value.to_string());
                let original = text.clone();
                redactor.scrub(&mut text);
                private_structure |= text != original;
            }
        }
    }

    let mut original = serde_json::json!(source);
    redactor.scrub(&mut original);
    if private_structure || parsed != raw || original.as_str() != Some(&source) {
        return Err(ApiError::bad(
            "Agent Card contains private scoped values; candidate withheld",
        ));
    }
    let description = moleapi_protocols::a2a::describe_card(&source, dialect)
        .map_err(|e| ApiError::bad(e.to_string()))?;
    // Card URL userinfo/query credentials must not become persistent source metadata.
    for interface in &description.interfaces {
        let url = url::Url::parse(&interface.url)
            .map_err(|_| ApiError::bad("Invalid Agent Card interface URL"))?;
        if !url.username().is_empty() || url.password().is_some() || url.query().is_some() {
            return Err(ApiError::bad(
                "Agent Card interface contains userinfo or query metadata; candidate withheld",
            ));
        }
    }
    let specification = Specification {
        id: uuid::Uuid::new_v4().to_string(),
        name: description.card["name"]
            .as_str()
            .unwrap_or("Agent Card")
            .chars()
            .take(256)
            .collect(),
        kind: "a2a-agent-card".into(),
        dialect: format!("a2a-{dialect}"),
        source,
    };
    Ok(Candidate {
        specification,
        description,
    })
}
pub async fn import(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Json(c): Json<Import>,
) -> Result<Json<Candidate>, ApiError> {
    let w = owned(&s, &owner.0, &c.workspace_id).await?;
    let e = execution::environment(&w, c.environment_id.as_deref())?;
    let scopes = execution::variables(&s, &w, None, e, &[], &[], &c.locals)?;
    Ok(Json(candidate(
        c.source,
        &c.dialect,
        &scopes.private_values,
    )?))
}
pub async fn discover(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    headers: axum::http::HeaderMap,
    Json(c): Json<Discover>,
) -> Result<Json<Candidate>, ApiError> {
    let discovery_id = c
        .discovery_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let lease = s.a2a_sources.start(&owner.0, &discovery_id)?;
    let gate = s.protocol_admission.owner(&owner.0)?;
    let generation = *gate.lock().await;
    let w = owned(&s, &owner.0, &c.workspace_id).await?;
    let collection = w
        .data
        .collections
        .iter()
        .find(|col| col.requests.iter().any(|r| r.id == c.request.id))
        .ok_or_else(ApiError::not_found)?;
    let e = execution::environment(&w, c.environment_id.as_deref())?;
    let mut scopes = execution::variables(&s, &w, Some(collection), e, &[], &[], &c.locals)?;
    let moleapi_core::Protocol::A2a { config } = &c.request.protocol else {
        return Err(ApiError::bad("Select an A2A request"));
    };
    let dialect = config.dialect.clone();
    let (mut request, _, _, _) =
        execution::prepare_live(&s, &owner.0, &w, &c.request, collection, &mut scopes).await?;
    request.protocol = moleapi_core::Protocol::Http;
    request.method = "GET".into();
    request.body_kind = "none".into();
    request.body.clear();
    request.follow_redirects = false;
    let mut url = moleapi_core::protocol_url(&request.url, false).map_err(|_| {
        ApiError::bad(
            "Agent Card destination or transport failed; check network policy, TLS and endpoint",
        )
    })?;
    if !url.path().ends_with(".json") {
        url.set_path("/.well-known/agent-card.json");
    }
    for pair in request.query.iter().filter(|pair| pair.enabled) {
        url.query_pairs_mut().append_pair(&pair.key, &pair.value);
    }
    request.url = url.into();
    let client = moleapi_core::checked_request_client(
        &url::Url::parse(&request.url).map_err(|e| ApiError::bad(e.to_string()))?,
        moleapi_core::NetworkPolicy {
            allow_private_network: s.local || s.config.allow_private_network,
        },
        request.verify_tls,
        request.network.as_deref(),
    )
    .await
    .map_err(|_| {
        ApiError::bad(
            "Agent Card destination or transport failed; check network policy, TLS and endpoint",
        )
    })?;
    let fetch = async {
        let mut response = client
            .get(&request.url)
            .headers(
                moleapi_core::request_headers(&request)
                    .map_err(|e| ApiError::bad(e.to_string()))?,
            )
            .send()
            .await
            .map_err(|_| ApiError::bad("Agent Card destination or transport failed; check network policy, TLS and endpoint"))?;
        if !response.status().is_success() {
            return Err(ApiError::bad(format!(
                "Agent Card endpoint returned HTTP {}",
                response.status().as_u16()
            )));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| ApiError::bad(e.to_string()))?
        {
            if bytes.len().saturating_add(chunk.len()) > moleapi_protocols::MAX_MESSAGE {
                return Err(ApiError::bad("Agent Card exceeds 1 MiB"));
            }
            bytes.extend_from_slice(&chunk);
        }
        String::from_utf8(bytes).map_err(|_| ApiError::bad("Agent Card is not UTF-8"))
    };
    let revoked = async {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if *gate.lock().await != generation {
                return ApiError::unauthorized();
            }
            match owned(&s, &owner.0, &c.workspace_id).await {
                Ok(workspace)
                    if workspace.data.collections.iter().any(|collection| {
                        collection
                            .requests
                            .iter()
                            .any(|request| request.id == c.request.id)
                    }) => {}
                _ => return ApiError::not_found(),
            }
        }
    };
    let source = tokio::select! {biased;_=lease.token.cancelled()=>return Err(ApiError::bad("Agent Card discovery stopped locally")),error=revoked=>return Err(error),result=tokio::time::timeout(std::time::Duration::from_millis(request.timeout_ms.clamp(100,300_000)),fetch)=>result.map_err(|_|ApiError::bad("Agent Card discovery timed out"))??};
    if lease.token.is_cancelled() {
        return Err(ApiError::bad("Agent Card discovery stopped locally"));
    }
    let admitted = gate.lock().await;
    if *admitted != generation {
        return Err(ApiError::unauthorized());
    };
    crate::auth::still_authenticated(&s, &owner.0, &headers).await?;
    let current = owned(&s, &owner.0, &c.workspace_id).await?;
    if !current
        .data
        .collections
        .iter()
        .any(|col| col.requests.iter().any(|r| r.id == c.request.id))
    {
        return Err(ApiError::not_found());
    };
    Ok(Json(candidate(source, &dialect, &scopes.private_values)?))
}
