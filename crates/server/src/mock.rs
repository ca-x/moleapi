use crate::{ApiError, AppState, auth::Identity, execution, workspaces::owned};
use axum::{
    Extension,
    extract::{OriginalUri, Path, State},
};
use axum::{
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
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
    let mut draft = moleapi_script_runtime::MockResponseDraft {
        status: e.status,
        headers: e.headers.clone(),
        body: e.body.clone(),
    };
    if !e.mock_script.trim().is_empty() {
        if w.data.script_mode != moleapi_core::ScriptMode::V2 {
            return Err(ApiError::bad("Mock scripts require compatibility v2"));
        }
        let incoming = safe_incoming_request(request, &uri, &incoming_headers)?;
        draft = execution::run_mock_phase(
            &s,
            &owner.0,
            &w,
            collection,
            &incoming,
            &e.mock_script,
            draft,
        )
        .await?;
        let current = owned(&s, &owner.0, &id).await?;
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
) -> Result<moleapi_core::RequestSpec, ApiError> {
    let mut request = saved.clone();
    request.method = "GET".into();
    request.protocol = moleapi_core::Protocol::Http;
    request.network = None;
    request.description.clear();
    request.pre_request_script.clear();
    request.post_response_script.clear();
    request.body_kind = "none".into();
    request.body.clear();
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
    for (index, (key, value)) in headers.iter().enumerate() {
        if index >= 100 {
            return Err(ApiError::bad("Mock headers exceed 100 entries"));
        }
        if moleapi_core::sensitive_query_key(key.as_str())
            || matches!(
                key.as_str(),
                "cookie" | "authorization" | "proxy-authorization" | "connection"
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
