use crate::{
    ApiError, AppState, auth::Identity, execution, mock_logs, mock_public, mock_rate,
    workspaces::owned,
};
use axum::{
    Extension, Json,
    body::to_bytes,
    extract::{OriginalUri, Path, Request, State},
};
use axum::{
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode},
    response::IntoResponse,
};
use rand::Rng;
use serde::Deserialize;
#[derive(Clone, serde::Serialize)]
pub(crate) struct RoutedSelection {
    pub request_id: String,
    pub example_id: String,
    pub outcome: &'static str,
}
#[derive(Clone)]
pub(crate) struct SavedExample;
#[derive(Clone)]
struct SimulatedFailure;
#[derive(Clone)]
struct SimulatedRateLimit;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidateLinkedSchema {
    request_id: String,
    specification_id: String,
    pointer: String,
    pub(crate) expected_revision: i64,
}

pub async fn validate_linked_schema(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
    Json(input): Json<ValidateLinkedSchema>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if input.request_id.len() > 128
        || input.specification_id.len() > 128
        || input.pointer.len() > 4096
    {
        return Err(ApiError::bad(
            "Linked Mock selection exceeds its size limit",
        ));
    }
    let workspace = owned(&s, &owner.0, &id).await?;
    if workspace.revision != input.expected_revision {
        return Err(ApiError::conflict(
            "Workspace changed before linked Mock validation",
        ));
    }
    let request = workspace
        .data
        .collections
        .iter()
        .flat_map(|collection| &collection.requests)
        .find(|request| request.id == input.request_id)
        .ok_or_else(ApiError::not_found)?;
    if request.protocol != moleapi_core::Protocol::Http
        || request.specification_id.as_deref() != Some(input.specification_id.as_str())
    {
        return Err(ApiError::bad(
            "Linked Mock source does not match HTTP request",
        ));
    }
    let spec = workspace
        .data
        .specifications
        .iter()
        .find(|spec| spec.id == input.specification_id)
        .ok_or_else(ApiError::not_found)?
        .clone();
    let method = request.method.clone();
    let permit = s
        .mock_schema_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "Mock schema generation capacity reached".into(),
        })?;
    let selected = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        moleapi_core::OpenapiMockIndex::parse(&spec)?.response_schema(&input.pointer)
    })
    .await
    .map_err(|_| ApiError::internal())?
    .map_err(|error| ApiError::bad(error.to_string()))?;
    if selected.method != method {
        return Err(ApiError::bad(
            "Linked Mock operation method differs from saved request",
        ));
    }
    let current = owned(&s, &owner.0, &id).await?;
    if current.revision != workspace.revision {
        return Err(ApiError::conflict(
            "Workspace changed during linked Mock validation",
        ));
    }
    Ok(Json(
        serde_json::json!({"method":selected.method,"status":selected.status,"media_type":selected.media_type}),
    ))
}

pub async fn mock(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path((id, request_id, example_id)): Path<(String, String, String)>,
    OriginalUri(uri): OriginalUri,
    incoming_headers: HeaderMap,
) -> Result<axum::response::Response, ApiError> {
    serve_direct(
        &s,
        &owner.0,
        &id,
        &request_id,
        &example_id,
        &uri,
        &incoming_headers,
        None,
    )
    .await
}

pub(crate) async fn mock_for(
    s: &AppState,
    grant: &mock_public::PublicGrant,
    example_id: &str,
    uri: &axum::http::Uri,
    headers: &HeaderMap,
) -> Result<axum::response::Response, ApiError> {
    serve_direct(
        s,
        &grant.owner,
        &grant.share.workspace_id,
        &grant.share.request_id,
        example_id,
        uri,
        headers,
        Some(grant),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn serve_direct(
    s: &AppState,
    owner: &str,
    id: &str,
    request_id: &str,
    example_id: &str,
    uri: &axum::http::Uri,
    incoming_headers: &HeaderMap,
    grant: Option<&mock_public::PublicGrant>,
) -> Result<axum::response::Response, ApiError> {
    let w = owned(s, owner, id).await?;
    if let Some(grant) = grant
        && !mock_public::request_matches(&w, &grant.share)?
    {
        return Err(ApiError::not_found());
    }
    let started = std::time::Instant::now();
    let selected = w.data.collections.iter().find_map(|collection| {
        collection
            .requests
            .iter()
            .find(|r| {
                r.id == request_id
                    && grant.is_none_or(|_| r.protocol == moleapi_core::Protocol::Http)
            })
            .and_then(|r| {
                r.examples
                    .iter()
                    .find(|e| e.id == example_id)
                    .map(|e| (collection, r, e))
            })
    });
    let selected = selected.filter(|(_, _, example)| {
        grant.is_none_or(|grant| grant.share.allow_dynamic || !is_dynamic(example))
    });
    let Some((collection, request, e)) = selected else {
        mock_logs::record(
            s,
            owner,
            &w,
            mock_logs::CallMeta {
                method: "GET",
                request_id: None,
                example_id: None,
                status: StatusCode::NOT_FOUND,
                outcome: "unmatched",
                elapsed_ms: started.elapsed().as_millis() as u64,
            },
        )
        .await?;
        return Err(ApiError::not_found());
    };
    let result = serve_selected(
        s,
        owner,
        &w,
        collection,
        request,
        e,
        uri,
        incoming_headers,
        &Method::GET,
        &[],
        None,
    )
    .await;
    if let Some(grant) = grant {
        mock_public::still_active(s, grant).await?;
    }
    let (status, outcome) = outcome_meta(&result);
    record_outcome(
        s,
        owner,
        &w,
        "GET",
        (request, e),
        started,
        (status, outcome),
    )
    .await?;
    if let Some(grant) = grant {
        mock_public::still_active(s, grant).await?;
    }
    result
}

pub async fn dispatch(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path((id, path)): Path<(String, String)>,
    OriginalUri(uri): OriginalUri,
    incoming: Request,
) -> Result<axum::response::Response, ApiError> {
    serve_routed(&s, &owner.0, &id, &path, &uri, incoming, None, None).await
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn dispatch_at_revision(
    s: &AppState,
    owner: &str,
    workspace: &str,
    path: &str,
    uri: &axum::http::Uri,
    incoming: Request,
    revision: i64,
) -> Result<axum::response::Response, ApiError> {
    serve_routed(
        s,
        owner,
        workspace,
        path,
        uri,
        incoming,
        None,
        Some(revision),
    )
    .await
}

pub(crate) async fn dispatch_for(
    s: &AppState,
    grant: &mock_public::PublicGrant,
    path: &str,
    uri: &axum::http::Uri,
    incoming: Request,
) -> Result<axum::response::Response, ApiError> {
    serve_routed(
        s,
        &grant.owner,
        &grant.share.workspace_id,
        path,
        uri,
        incoming,
        Some(grant),
        None,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn serve_routed(
    s: &AppState,
    owner: &str,
    id: &str,
    path: &str,
    uri: &axum::http::Uri,
    incoming: Request,
    grant: Option<&mock_public::PublicGrant>,
    expected_revision: Option<i64>,
) -> Result<axum::response::Response, ApiError> {
    let (parts, body) = incoming.into_parts();
    check_uri_budget(uri)?;
    let bytes = to_bytes(body, 1024 * 1024)
        .await
        .map_err(|_| ApiError::bad("Mock request body exceeds 1 MiB"))?;
    let w = owned(s, owner, id).await?;
    if expected_revision.is_some_and(|revision| revision != w.revision) {
        return Err(ApiError::conflict("Workspace changed before Mock routing"));
    }
    if let Some(grant) = grant
        && !mock_public::request_matches(&w, &grant.share)?
    {
        return Err(ApiError::not_found());
    }
    let started = std::time::Instant::now();
    let actual = format!("/{path}");
    let mut selected = None;
    let mut best_score = None;
    let mut snapshot = None;
    let mut scanned_requests = 0usize;
    let mut scanned_examples = 0usize;
    for collection in &w.data.collections {
        for request in &collection.requests {
            scanned_requests += 1;
            if scanned_requests > 10_000 {
                return reject_after_owner(
                    s,
                    owner,
                    &w,
                    parts.method.as_str(),
                    started,
                    "Mock matching request budget exceeded",
                )
                .await;
            }
            if request.protocol != moleapi_core::Protocol::Http
                || request.method != parts.method.as_str()
                || grant.is_some_and(|grant| request.id != grant.share.request_id)
            {
                continue;
            }
            let Ok(saved_url) = url::Url::parse(&request.url) else {
                continue;
            };
            if !matches!(saved_url.scheme(), "http" | "https") {
                continue;
            }
            let Some(literal_segments) = path_match(saved_url.path(), &actual) else {
                continue;
            };
            if snapshot.is_none() {
                let incoming =
                    safe_incoming_request(request, uri, &parts.headers, &parts.method, &bytes);
                snapshot = Some(match incoming {
                    Ok(incoming) => incoming,
                    Err(error) => {
                        mock_logs::record(
                            s,
                            owner,
                            &w,
                            mock_logs::CallMeta {
                                method: parts.method.as_str(),
                                request_id: None,
                                example_id: None,
                                status: error.status,
                                outcome: "rejected",
                                elapsed_ms: started.elapsed().as_millis() as u64,
                            },
                        )
                        .await?;
                        return Err(error);
                    }
                });
            }
            let incoming = snapshot.as_ref().expect("snapshot initialized");
            let mut saved_query: Vec<(String, String)> = saved_url
                .query_pairs()
                .map(|(key, value)| (key.into_owned(), value.into_owned()))
                .collect();
            saved_query.extend(
                request
                    .query
                    .iter()
                    .filter(|row| row.enabled)
                    .map(|row| (row.key.clone(), row.value.clone())),
            );
            if !pairs_match(
                saved_query
                    .iter()
                    .map(|(key, value)| (key.as_str(), value.as_str())),
                incoming
                    .query
                    .iter()
                    .map(|row| (row.key.as_str(), row.value.as_str())),
                false,
            ) {
                continue;
            }
            for example in &request.examples {
                scanned_examples += 1;
                if scanned_examples > 10_000 {
                    return reject_after_owner(
                        s,
                        owner,
                        &w,
                        parts.method.as_str(),
                        started,
                        "Mock matching example budget exceeded",
                    )
                    .await;
                }
                if grant.is_some_and(|grant| !grant.share.allow_dynamic) && is_dynamic(example) {
                    continue;
                }
                let Some(condition_count) = example_matches(example, incoming) else {
                    continue;
                };
                let score = (
                    example.mock_match.is_some(),
                    example.mock_match.as_ref().map_or(0, |rule| rule.priority),
                    literal_segments,
                    condition_count,
                );
                if best_score.is_none_or(|best| score > best) {
                    best_score = Some(score);
                    selected = Some((collection, request, example));
                }
            }
        }
    }
    let Some((collection, request, example)) = selected else {
        mock_logs::record(
            s,
            owner,
            &w,
            mock_logs::CallMeta {
                method: parts.method.as_str(),
                request_id: None,
                example_id: None,
                status: StatusCode::NOT_FOUND,
                outcome: "unmatched",
                elapsed_ms: started.elapsed().as_millis() as u64,
            },
        )
        .await?;
        return Err(ApiError::not_found());
    };
    let result = serve_selected(
        s,
        owner,
        &w,
        collection,
        request,
        example,
        uri,
        &parts.headers,
        &parts.method,
        &bytes,
        Some(&actual),
    )
    .await;
    if let Some(grant) = grant {
        mock_public::still_active(s, grant).await?;
    }
    let (status, outcome) = outcome_meta(&result);
    record_outcome(
        s,
        owner,
        &w,
        parts.method.as_str(),
        (request, example),
        started,
        (status, outcome),
    )
    .await?;
    if let Some(grant) = grant {
        mock_public::still_active(s, grant).await?;
    }
    result.map(|mut response| {
        response.extensions_mut().insert(RoutedSelection {
            request_id: request.id.clone(),
            example_id: example.id.clone(),
            outcome,
        });
        response
    })
}

async fn reject_after_owner(
    s: &AppState,
    owner: &str,
    workspace: &moleapi_core::Workspace,
    method: &str,
    started: std::time::Instant,
    message: &str,
) -> Result<axum::response::Response, ApiError> {
    let error = ApiError::bad(message);
    mock_logs::record(
        s,
        owner,
        workspace,
        mock_logs::CallMeta {
            method,
            request_id: None,
            example_id: None,
            status: error.status,
            outcome: "rejected",
            elapsed_ms: started.elapsed().as_millis() as u64,
        },
    )
    .await?;
    Err(error)
}

async fn record_outcome(
    s: &AppState,
    owner: &str,
    workspace: &moleapi_core::Workspace,
    method: &str,
    selected: (&moleapi_core::RequestSpec, &moleapi_core::Example),
    started: std::time::Instant,
    result: (StatusCode, &str),
) -> Result<(), ApiError> {
    mock_logs::record(
        s,
        owner,
        workspace,
        mock_logs::CallMeta {
            method,
            request_id: Some(&selected.0.id),
            example_id: Some(&selected.1.id),
            status: result.0,
            outcome: result.1,
            elapsed_ms: started.elapsed().as_millis() as u64,
        },
    )
    .await
}

fn outcome_meta(result: &Result<axum::response::Response, ApiError>) -> (StatusCode, &'static str) {
    match result {
        Ok(response) if response.extensions().get::<SimulatedRateLimit>().is_some() => {
            (response.status(), "rate_limited")
        }
        Ok(response) if response.extensions().get::<SimulatedFailure>().is_some() => {
            (response.status(), "simulated_failure")
        }
        Ok(response) => (response.status(), "served"),
        Err(error) => (error.status, "failed"),
    }
}

fn is_dynamic(example: &moleapi_core::Example) -> bool {
    !example.mock_script.trim().is_empty() || example.mock_data.is_some()
}

fn mock_data_parameters(
    binding: &moleapi_core::MockDataBinding,
    saved: &moleapi_core::RequestSpec,
    route_path: Option<&str>,
    incoming: &moleapi_core::RequestSpec,
) -> Result<Vec<String>, ApiError> {
    if binding.parameters.len() > 16 {
        return Err(ApiError::bad("Mock data parameter limit exceeded"));
    }
    binding
        .parameters
        .iter()
        .map(|parameter| {
            let values: Vec<String> = match parameter.source {
                moleapi_core::MockDataSource::Query => incoming
                    .query
                    .iter()
                    .filter(|row| row.enabled && row.key == parameter.name)
                    .map(|row| row.value.clone())
                    .collect(),
                moleapi_core::MockDataSource::Header => incoming
                    .headers
                    .iter()
                    .filter(|row| row.enabled && row.key.eq_ignore_ascii_case(&parameter.name))
                    .map(|row| row.value.clone())
                    .collect(),
                moleapi_core::MockDataSource::Path => {
                    let actual = route_path.ok_or_else(|| {
                        ApiError::bad("Mock data path parameter requires a routed Mock call")
                    })?;
                    let saved = url::Url::parse(&saved.url)
                        .map_err(|_| ApiError::bad("Invalid saved Mock data route"))?;
                    saved
                        .path()
                        .trim_matches('/')
                        .split('/')
                        .zip(actual.trim_matches('/').split('/'))
                        .filter_map(|(segment, value)| {
                            let name = percent_encoding::percent_decode_str(segment)
                                .decode_utf8()
                                .ok()?;
                            (name.as_ref() == format!("{{{}}}", parameter.name))
                                .then(|| {
                                    percent_encoding::percent_decode_str(value)
                                        .decode_utf8()
                                        .ok()
                                })
                                .flatten()
                                .map(|value| value.into_owned())
                        })
                        .collect()
                }
            };
            let [value] = values.as_slice() else {
                return Err(ApiError::bad("Mock data parameter is missing or ambiguous"));
            };
            if value.len() > 512 || value.chars().any(char::is_control) {
                return Err(ApiError::bad("Mock data parameter exceeds its safe limit"));
            }
            Ok(value.clone())
        })
        .collect()
}

fn mock_data_script(
    binding: &moleapi_core::MockDataBinding,
    params: &[String],
) -> Result<String, ApiError> {
    let mut script = String::from("await (async()=>{const result=await pm.datasets(");
    script.push_str(&serde_json::to_string(&binding.dataset_id).map_err(|_| ApiError::internal())?);
    script.push_str(").executeView(");
    script.push_str(&serde_json::to_string(&binding.view_id).map_err(|_| ApiError::internal())?);
    script.push(',');
    script.push_str(&serde_json::to_string(params).map_err(|_| ApiError::internal())?);
    script.push_str(");const rows=[];for await(const row of result.rows)rows.push(row);");
    match binding.shape {
        moleapi_core::MockDataShape::Rows => script.push_str("pm.mock.setBody(rows);"),
        moleapi_core::MockDataShape::First => script.push_str(
            "if(rows.length){pm.mock.setBody(rows[0]);}else{pm.mock.setStatus(404);pm.mock.setBody(null);}",
        ),
    }
    script.push_str("})()");
    Ok(script)
}

fn path_match(saved: &str, actual: &str) -> Option<usize> {
    let saved_segments: Vec<_> = saved.trim_matches('/').split('/').collect();
    let actual_segments: Vec<_> = actual.trim_matches('/').split('/').collect();
    if saved_segments.len() != actual_segments.len() {
        return None;
    }
    let mut literal = 0;
    for (saved, actual) in saved_segments.into_iter().zip(actual_segments) {
        let saved = percent_encoding::percent_decode_str(saved)
            .decode_utf8()
            .ok()?;
        let saved = saved.as_ref();
        let placeholder = saved
            .strip_prefix('{')
            .and_then(|name| name.strip_suffix('}'))
            .is_some_and(|name| {
                name.len() <= 64
                    && name
                        .bytes()
                        .next()
                        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
                    && name
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            });
        if placeholder {
            if actual.is_empty() {
                return None;
            }
        } else if saved.contains(['{', '}']) {
            return None;
        } else if saved == actual {
            literal += 1;
        } else {
            return None;
        }
    }
    Some(literal)
}

fn check_uri_budget(uri: &axum::http::Uri) -> Result<(), ApiError> {
    if uri.path().len() > 4096 || uri.query().is_some_and(|query| query.len() > 8192) {
        return Err(ApiError::bad("Mock path or query exceeds byte limit"));
    }
    Ok(())
}

fn pairs_match<'a>(
    expected: impl Iterator<Item = (&'a str, &'a str)>,
    actual: impl Iterator<Item = (&'a str, &'a str)>,
    header_names: bool,
) -> bool {
    let actual: Vec<_> = actual.collect();
    let mut used = vec![false; actual.len()];
    for (key, value) in expected {
        let Some(index) =
            actual
                .iter()
                .enumerate()
                .find_map(|(index, &(actual_key, actual_value))| {
                    (!used[index]
                        && actual_value == value
                        && if header_names {
                            actual_key.eq_ignore_ascii_case(key)
                        } else {
                            actual_key == key
                        })
                    .then_some(index)
                })
        else {
            return false;
        };
        used[index] = true;
    }
    true
}

fn example_matches(
    example: &moleapi_core::Example,
    incoming: &moleapi_core::RequestSpec,
) -> Option<usize> {
    let Some(rule) = &example.mock_match else {
        return Some(0);
    };
    let query = rule
        .query
        .iter()
        .filter(|row| row.enabled)
        .collect::<Vec<_>>();
    let headers = rule
        .headers
        .iter()
        .filter(|row| row.enabled)
        .collect::<Vec<_>>();
    if !pairs_match(
        query
            .iter()
            .map(|row| (row.key.as_str(), row.value.as_str())),
        incoming
            .query
            .iter()
            .map(|row| (row.key.as_str(), row.value.as_str())),
        false,
    ) || !pairs_match(
        headers
            .iter()
            .map(|row| (row.key.as_str(), row.value.as_str())),
        incoming
            .headers
            .iter()
            .map(|row| (row.key.as_str(), row.value.as_str())),
        true,
    ) || rule
        .body
        .as_ref()
        .is_some_and(|body| body != &incoming.body)
    {
        return None;
    }
    Some(query.len() + headers.len() + usize::from(rule.body.is_some()))
}

#[allow(clippy::too_many_arguments)]
async fn serve_selected(
    s: &AppState,
    owner: &str,
    w: &moleapi_core::Workspace,
    collection: &moleapi_core::Collection,
    request: &moleapi_core::RequestSpec,
    e: &moleapi_core::Example,
    uri: &axum::http::Uri,
    incoming_headers: &HeaderMap,
    method: &Method,
    body: &[u8],
    route_path: Option<&str>,
) -> Result<axum::response::Response, ApiError> {
    if e.fault_rate_percent > 100 || !(500..=599).contains(&e.fault_status) {
        return Err(ApiError::bad("Invalid saved Mock fault settings"));
    }
    if let mock_rate::Admission::Limited {
        retry_after_seconds,
    } = mock_rate::admit(s, owner, w, &request.id, e).await?
    {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::RETRY_AFTER,
            HeaderValue::from_str(&retry_after_seconds.to_string())
                .map_err(|_| ApiError::internal())?,
        );
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        );
        headers.insert(
            axum::http::header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        );
        let mut response = (
            StatusCode::TOO_MANY_REQUESTS,
            headers,
            "Simulated Mock rate limit",
        )
            .into_response();
        response.extensions_mut().insert(SavedExample);
        response.extensions_mut().insert(SimulatedRateLimit);
        return Ok(response);
    }
    if e.fault_rate_percent > 0
        && (e.fault_rate_percent == 100
            || rand::thread_rng().gen_range(0u8..100) < e.fault_rate_percent)
    {
        wait_selected_delay(s, owner, w, e.delay_ms).await?;
        let mut response = (
            StatusCode::from_u16(e.fault_status)
                .map_err(|_| ApiError::bad("Invalid Mock fault status"))?,
            [
                (
                    axum::http::header::CONTENT_TYPE,
                    "text/plain; charset=utf-8",
                ),
                (axum::http::header::CACHE_CONTROL, "no-store"),
            ],
            "Simulated Mock failure",
        )
            .into_response();
        response.extensions_mut().insert(SavedExample);
        response.extensions_mut().insert(SimulatedFailure);
        return Ok(response);
    }
    let mut response_headers = e.headers.clone();
    enum SchemaSource {
        Inline(String),
        Linked(moleapi_core::Specification, String, String, u16),
    }
    let schema_source = if let Some(link) = &e.mock_schema_link {
        let spec = w
            .data
            .specifications
            .iter()
            .find(|spec| spec.id == link.specification_id)
            .ok_or_else(ApiError::not_found)?;
        Some(SchemaSource::Linked(
            spec.clone(),
            link.pointer.clone(),
            request.method.clone(),
            e.status,
        ))
    } else if !e.mock_schema.is_empty() {
        Some(SchemaSource::Inline(e.mock_schema.clone()))
    } else {
        None
    };
    let response_body = if let Some(schema_source) = schema_source {
        let infer_names = e.mock_name_inference;
        let permit = s
            .mock_schema_slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| ApiError {
                status: StatusCode::TOO_MANY_REQUESTS,
                message: "Mock schema generation capacity reached".into(),
            })?;
        let (body, media_type) =
            tokio::task::spawn_blocking(move || -> anyhow::Result<(String, String)> {
                let _permit = permit;
                let generate = |schema: &str| {
                    if infer_names {
                        moleapi_core::generate_mock_schema_with_names(schema)
                    } else {
                        moleapi_core::generate_mock_schema(schema)
                    }
                };
                match schema_source {
                    SchemaSource::Inline(schema) => {
                        Ok((generate(&schema)?, "application/json".into()))
                    }
                    SchemaSource::Linked(spec, pointer, method, status) => {
                        let selected = moleapi_core::OpenapiMockIndex::parse(&spec)?
                            .response_schema(&pointer)?;
                        anyhow::ensure!(
                            selected.method == method && selected.status == status,
                            "Linked Mock operation changed"
                        );
                        Ok((generate(&selected.schema)?, selected.media_type))
                    }
                }
            })
            .await
            .map_err(|_| ApiError::internal())?
            .map_err(|_| ApiError::bad("Invalid saved Mock schema"))?;
        if !response_headers
            .iter()
            .any(|header| header.enabled && header.key.eq_ignore_ascii_case("content-type"))
        {
            response_headers.push(moleapi_core::Pair {
                id: "mock-schema-content-type".into(),
                key: "Content-Type".into(),
                value: media_type,
                enabled: true,
                secret: None,
                local_value: None,
            });
        }
        let current = owned(s, owner, &w.id).await?;
        if current.revision != w.revision {
            return Err(ApiError::conflict(
                "Workspace changed during Mock schema generation",
            ));
        }
        body
    } else {
        e.body.clone()
    };
    if e.mock_data.is_some()
        && !response_headers
            .iter()
            .any(|header| header.enabled && header.key.eq_ignore_ascii_case("content-type"))
    {
        response_headers.push(moleapi_core::Pair {
            id: "mock-data-content-type".into(),
            key: "Content-Type".into(),
            value: "application/json".into(),
            enabled: true,
            secret: None,
            local_value: None,
        });
    }
    let mut draft = moleapi_script_runtime::MockResponseDraft {
        status: e.status,
        headers: response_headers,
        body: response_body,
    };
    if is_dynamic(e) {
        if w.data.script_mode != moleapi_core::ScriptMode::V2 {
            return Err(ApiError::bad("Dynamic Mock requires compatibility v2"));
        }
        let incoming = safe_incoming_request(request, uri, incoming_headers, method, body)?;
        let mut scripts = Vec::new();
        if let Some(binding) = &e.mock_data {
            let params = mock_data_parameters(binding, request, route_path, &incoming)?;
            scripts.push(mock_data_script(binding, &params)?);
        }
        if !e.mock_script.trim().is_empty() {
            scripts.push(e.mock_script.clone());
        }
        draft = execution::run_mock_phase(
            s,
            owner,
            w,
            collection,
            &incoming,
            execution::MockPhase {
                scripts,
                seed: draft,
                allow_empty_live: e.mock_data.is_some(),
            },
        )
        .await?;
        let current = owned(s, owner, &w.id).await?;
        if current.revision != w.revision {
            return Err(ApiError::conflict("Workspace changed during Mock script"));
        }
    }
    let mut headers = HeaderMap::new();
    let nominated: Vec<String> = draft
        .headers
        .iter()
        .filter(|h| h.enabled && h.key.eq_ignore_ascii_case("connection"))
        .flat_map(|h| h.value.split(',').map(|v| v.trim().to_ascii_lowercase()))
        .collect();
    for h in draft.headers.iter().filter(|h| h.enabled) {
        let name = h.key.to_ascii_lowercase();
        if [
            "connection",
            "keep-alive",
            "proxy-authenticate",
            "proxy-authorization",
            "te",
            "trailer",
            "transfer-encoding",
            "upgrade",
            "content-length",
        ]
        .contains(&name.as_str())
            || nominated.contains(&name)
        {
            continue;
        }
        headers.append(
            HeaderName::from_bytes(h.key.as_bytes())
                .map_err(|_| ApiError::bad("Invalid mock header"))?,
            HeaderValue::from_str(&h.value).map_err(|_| ApiError::bad("Invalid mock header"))?,
        );
    }
    let mut response = (
        StatusCode::from_u16(draft.status).map_err(|_| ApiError::bad("Invalid mock status"))?,
        headers,
        draft.body,
    )
        .into_response();
    wait_selected_delay(s, owner, w, e.delay_ms).await?;
    response.extensions_mut().insert(SavedExample);
    Ok(response)
}
async fn wait_selected_delay(
    s: &AppState,
    owner: &str,
    workspace: &moleapi_core::Workspace,
    delay_ms: u64,
) -> Result<(), ApiError> {
    if delay_ms == 0 {
        return Ok(());
    }
    if delay_ms > 30_000 {
        return Err(ApiError::bad("Mock delay exceeds 30000 ms"));
    }
    let _permit = s
        .mock_delay_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "Delayed Mock capacity reached".into(),
        })?;
    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
    let current = owned(s, owner, &workspace.id).await?;
    if current.revision != workspace.revision {
        return Err(ApiError::conflict("Workspace changed during Mock delay"));
    }
    Ok(())
}
fn safe_incoming_request(
    saved: &moleapi_core::RequestSpec,
    uri: &axum::http::Uri,
    headers: &HeaderMap,
    method: &Method,
    body: &[u8],
) -> Result<moleapi_core::RequestSpec, ApiError> {
    check_uri_budget(uri)?;
    let mut request = saved.clone();
    request.method = method.as_str().into();
    request.protocol = moleapi_core::Protocol::Http;
    request.network = None;
    request.description.clear();
    request.pre_request_script.clear();
    request.post_response_script.clear();
    request.body_kind = if body.is_empty() { "none" } else { "text" }.into();
    request.body = std::str::from_utf8(body)
        .map_err(|_| ApiError::bad("Mock request body must be UTF-8"))?
        .into();
    request.examples.clear();
    request.assertions.clear();
    request.specification_id = None;
    request.operation_id = None;
    request.auth = moleapi_core::Auth {
        asap: None,
        edgegrid: None,
        ntlm: None,
        oauth1: None,
        hawk: None,
        aws: None,
        oauth2: None,
        api_key: None,
        jwt: None,
        kind: "none".into(),
        token: String::new(),
        username: String::new(),
        password: String::new(),
    };
    let mut query = url::form_urlencoded::Serializer::new(String::new());
    request.query.clear();
    for (index, (key, value)) in
        url::form_urlencoded::parse(uri.query().unwrap_or_default().as_bytes()).enumerate()
    {
        if index >= 100 {
            return Err(ApiError::bad("Mock query exceeds 100 parameters"));
        }
        if moleapi_core::sensitive_query_key(&key) {
            continue;
        }
        query.append_pair(&key, &value);
        request.query.push(moleapi_core::Pair {
            id: format!("mock-query-{index}"),
            key: key.into_owned(),
            value: value.into_owned(),
            enabled: true,
            secret: None,
            local_value: None,
        });
    }
    let query = query.finish();
    request.url = format!(
        "http://mock.invalid/{}{}",
        uri.path().trim_start_matches('/'),
        if query.is_empty() {
            String::new()
        } else {
            format!("?{query}")
        }
    );
    request.headers.clear();
    let mut bytes = 0usize;
    let mut nominated = Vec::new();
    for value in headers.get_all("connection").iter() {
        let value = value
            .to_str()
            .map_err(|_| ApiError::bad("Invalid Mock Connection header"))?;
        for name in value.split(',') {
            nominated.push(
                HeaderName::from_bytes(name.trim().as_bytes())
                    .map_err(|_| ApiError::bad("Invalid Mock Connection header"))?,
            );
        }
    }
    for (index, (key, value)) in headers.iter().enumerate() {
        if index >= 100 {
            return Err(ApiError::bad("Mock headers exceed 100 entries"));
        }
        if moleapi_core::sensitive_query_key(key.as_str())
            || nominated.contains(key)
            || matches!(
                key.as_str(),
                "cookie"
                    | "authorization"
                    | "proxy-authorization"
                    | "connection"
                    | "keep-alive"
                    | "proxy-authenticate"
                    | "te"
                    | "trailer"
                    | "transfer-encoding"
                    | "upgrade"
            )
        {
            continue;
        }
        let Ok(value) = value.to_str() else {
            continue;
        };
        bytes = bytes.saturating_add(key.as_str().len() + value.len());
        if bytes > 64 * 1024 {
            return Err(ApiError::bad("Mock headers exceed 64 KiB"));
        }
        request.headers.push(moleapi_core::Pair {
            id: format!("mock-header-{index}"),
            key: key.as_str().into(),
            value: value.into(),
            enabled: true,
            secret: None,
            local_value: None,
        });
    }
    moleapi_core::validate_request(&request, true)
        .map_err(|_| ApiError::bad("Invalid incoming Mock request"))?;
    Ok(request)
}
