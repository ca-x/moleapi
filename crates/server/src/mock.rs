use crate::{ApiError, AppState, auth::Identity, execution, workspaces::owned};
use axum::{
    Extension,
    body::to_bytes,
    extract::{OriginalUri, Path, Request, State},
};
use axum::{
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode},
    response::IntoResponse,
};
#[derive(Clone)]
pub(crate) struct SavedExample;

pub async fn mock(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path((id, request_id, example_id)): Path<(String, String, String)>,
    OriginalUri(uri): OriginalUri,
    incoming_headers: HeaderMap,
) -> Result<axum::response::Response, ApiError> {
    let w = owned(&s, &owner.0, &id).await?;
    let (collection, request, e) = w
        .data
        .collections
        .iter()
        .find_map(|collection| {
            collection
                .requests
                .iter()
                .find(|r| r.id == request_id)
                .and_then(|r| {
                    r.examples
                        .iter()
                        .find(|e| e.id == example_id)
                        .map(|e| (collection, r, e))
                })
        })
        .ok_or_else(ApiError::not_found)?;
    serve_selected(
        &s,
        &owner.0,
        &w,
        collection,
        request,
        e,
        &uri,
        &incoming_headers,
        &Method::GET,
        &[],
    )
    .await
}

pub async fn dispatch(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path((id, path)): Path<(String, String)>,
    OriginalUri(uri): OriginalUri,
    incoming: Request,
) -> Result<axum::response::Response, ApiError> {
    let (parts, body) = incoming.into_parts();
    check_uri_budget(&uri)?;
    let bytes = to_bytes(body, 1024 * 1024)
        .await
        .map_err(|_| ApiError::bad("Mock request body exceeds 1 MiB"))?;
    let w = owned(&s, &owner.0, &id).await?;
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
                return Err(ApiError::bad("Mock matching request budget exceeded"));
            }
            if request.protocol != moleapi_core::Protocol::Http
                || request.method != parts.method.as_str()
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
                snapshot = Some(safe_incoming_request(
                    request,
                    &uri,
                    &parts.headers,
                    &parts.method,
                    &bytes,
                )?);
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
                    return Err(ApiError::bad("Mock matching example budget exceeded"));
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
    let (collection, request, example) = selected.ok_or_else(ApiError::not_found)?;
    serve_selected(
        &s,
        &owner.0,
        &w,
        collection,
        request,
        example,
        &uri,
        &parts.headers,
        &parts.method,
        &bytes,
    )
    .await
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
) -> Result<axum::response::Response, ApiError> {
    let mut draft = moleapi_script_runtime::MockResponseDraft {
        status: e.status,
        headers: e.headers.clone(),
        body: e.body.clone(),
    };
    if !e.mock_script.trim().is_empty() {
        if w.data.script_mode != moleapi_core::ScriptMode::V2 {
            return Err(ApiError::bad("Mock scripts require compatibility v2"));
        }
        let incoming = safe_incoming_request(request, uri, incoming_headers, method, body)?;
        draft =
            execution::run_mock_phase(s, owner, w, collection, &incoming, &e.mock_script, draft)
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
    response.extensions_mut().insert(SavedExample);
    Ok(response)
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
