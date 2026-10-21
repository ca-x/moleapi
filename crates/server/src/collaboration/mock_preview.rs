use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{HeaderName, HeaderValue, Method, Uri},
    response::IntoResponse,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Serialize;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncomingPair {
    key: String,
    value: String,
    #[serde(default = "enabled")]
    enabled: bool,
}
fn enabled() -> bool {
    true
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutePreview {
    expected_revision: i64,
    method: String,
    path: String,
    #[serde(default)]
    query: Vec<IncomingPair>,
    #[serde(default)]
    headers: Vec<IncomingPair>,
    #[serde(default)]
    body: String,
}
#[derive(Serialize)]
pub struct PreviewResponse {
    #[serde(flatten)]
    response: moleapi_core::Response,
    matched: Option<crate::mock::RoutedSelection>,
    outcome: String,
}
fn transport_header(name: &str) -> bool {
    matches!(
        name,
        "authorization"
            | "cookie"
            | "proxy-authorization"
            | "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "content-length"
    )
}
fn incoming(input: RoutePreview) -> Result<(String, Uri, Request), ApiError> {
    if !matches!(
        input.method.as_str(),
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS"
    ) {
        return Err(ApiError::bad("Unsupported Mock test method"));
    }
    if input.path.len() > 4096
        || !input.path.starts_with('/')
        || input.path.starts_with("//")
        || input.path.contains(['?', '#', '\\'])
        || input.path.chars().any(char::is_control)
    {
        return Err(ApiError::bad(
            "Mock test path must be an absolute path without query or fragment",
        ));
    }
    if input.query.len() > 100 || input.headers.len() > 100 || input.body.len() > 1024 * 1024 {
        return Err(ApiError::bad("Mock test input exceeds its size limit"));
    }
    // Encode path characters without URL dot-segment normalization. The raw
    // wildcard route preserves these segments before Axum decodes its capture.
    const PATH: &percent_encoding::AsciiSet = &percent_encoding::CONTROLS
        .add(b' ')
        .add(b'"')
        .add(b'<')
        .add(b'>')
        .add(b'`')
        .add(b'{')
        .add(b'}');
    let encoded_path = percent_encoding::utf8_percent_encode(&input.path, PATH).to_string();
    let mut url = url::Url::parse("http://mock.invalid").map_err(|_| ApiError::internal())?;
    for pair in input.query.iter().filter(|p| p.enabled) {
        if moleapi_core::sensitive_query_key(&pair.key) {
            return Err(ApiError::bad(
                "Credential-like query parameters are unavailable for Mock testing",
            ));
        }
        if pair.key.len() > 8192 || pair.value.len() > 8192 {
            return Err(ApiError::bad("Mock path or query exceeds byte limit"));
        }
        url.query_pairs_mut().append_pair(&pair.key, &pair.value);
        if url.query().is_some_and(|q| q.len() > 8192) {
            return Err(ApiError::bad("Mock path or query exceeds byte limit"));
        }
    }
    if encoded_path.len() > 4096 || url.query().is_some_and(|q| q.len() > 8192) {
        return Err(ApiError::bad("Mock path or query exceeds byte limit"));
    }
    let path = percent_encoding::percent_decode_str(encoded_path.trim_start_matches('/'))
        .decode_utf8()
        .map_err(|_| ApiError::bad("Mock path must be UTF-8"))?
        .into_owned();
    let path_query = format!(
        "{}{}",
        encoded_path,
        url.query().map_or(String::new(), |q| format!("?{q}"))
    );
    let uri: Uri = path_query
        .parse()
        .map_err(|_| ApiError::bad("Invalid Mock test URI"))?;
    let mut builder = Request::builder()
        .method(input.method.as_str())
        .uri(uri.clone());
    let mut bytes = 0usize;
    for pair in input.headers.iter().filter(|p| p.enabled) {
        let name = HeaderName::from_bytes(pair.key.as_bytes())
            .map_err(|_| ApiError::bad("Invalid Mock test header"))?;
        if transport_header(name.as_str()) || moleapi_core::sensitive_query_key(name.as_str()) {
            return Err(ApiError::bad(
                "Credential or hop-by-hop headers are unavailable for Mock testing",
            ));
        }
        bytes = bytes.saturating_add(pair.key.len() + pair.value.len());
        if bytes > 64 * 1024 {
            return Err(ApiError::bad("Mock headers exceed 64 KiB"));
        }
        let value = HeaderValue::from_str(&pair.value)
            .map_err(|_| ApiError::bad("Invalid Mock test header"))?;
        builder = builder.header(name, value);
    }
    let incoming = builder
        .body(Body::from(input.body))
        .map_err(|_| ApiError::bad("Invalid Mock test request"))?;
    Ok((path, uri, incoming))
}
pub async fn shared_mock_route_preview(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    headers: HeaderMap,
    Path((owner, workspace)): Path<(String, String)>,
    Json(input): Json<RoutePreview>,
) -> Result<Json<PreviewResponse>, ApiError> {
    if state.local {
        return Err(ApiError::not_found());
    }
    let tx = state.db.begin().await?;
    let canonical = lock_workspace(&tx, &owner, &workspace).await?;
    let (_, admission) = access(&tx, &actor.0, &owner, &workspace).await?;
    if admission.role == Role::Viewer {
        return Err(ApiError::forbidden("Viewer cannot test Mock routes"));
    }
    if canonical.revision != input.expected_revision {
        return Err(ApiError::conflict(
            "Workspace changed before Mock route testing",
        ));
    }
    let (path, logical_uri, mut incoming) = incoming(input)?;
    let mut namespace = url::Url::parse("http://mock.invalid").map_err(|_| ApiError::internal())?;
    namespace
        .path_segments_mut()
        .map_err(|_| ApiError::internal())?
        .clear()
        .extend(["api", "owners", &owner, "workspaces", &workspace, "mocks"]);
    let uri: Uri = format!("{}{logical_uri}", namespace.path())
        .parse()
        .map_err(|_| ApiError::bad("Invalid Mock test URI"))?;
    if uri.path().len() > 4096 {
        return Err(ApiError::bad("Mock path or query exceeds byte limit"));
    }
    *incoming.uri_mut() = uri.clone();
    record_event_result(
        &tx,
        &owner,
        &workspace,
        &actor.0,
        "mock.route.preview",
        None,
        "started",
    )
    .await?;
    tx.commit().await?;
    finish_specification_preview(
        &state.db,
        &actor.0,
        &owner,
        &workspace,
        (admission, canonical.revision),
        crate::auth::still_authenticated(&state, &actor.0, &headers),
    )
    .await?;
    let operation = async {
        let start = std::time::Instant::now();
        let head = incoming.method() == Method::HEAD;
        let result = crate::mock::dispatch_at_revision(
            &state,
            &owner,
            &workspace,
            &path,
            &uri,
            incoming,
            canonical.revision,
        )
        .await;
        let response = match result {
            Ok(response) => response,
            Err(error) => error.into_response(),
        };
        let status = response.status();
        let matched = response
            .extensions()
            .get::<crate::mock::RoutedSelection>()
            .cloned();
        let outcome = matched.as_ref().map_or_else(
            || {
                if status == axum::http::StatusCode::NOT_FOUND {
                    "unmatched".to_owned()
                } else {
                    "failed".to_owned()
                }
            },
            |s| s.outcome.to_owned(),
        );
        let response_headers = response
            .headers()
            .iter()
            .enumerate()
            .map(|(i, (name, value))| moleapi_core::Pair {
                id: format!("mock-response-{i}"),
                key: name.to_string(),
                value: String::from_utf8_lossy(value.as_bytes()).into_owned(),
                enabled: true,
                secret: None,
                local_value: None,
            })
            .collect();
        let body = if head {
            axum::body::Bytes::new()
        } else {
            to_bytes(response.into_body(), moleapi_core::MAX_BODY)
                .await
                .map_err(|_| ApiError::bad("Mock response exceeds byte limit"))?
        };
        let size_bytes = body.len();
        let (body, body_base64) = match std::str::from_utf8(&body) {
            Ok(text) => (text.to_owned(), None),
            Err(_) => (String::new(), Some(STANDARD.encode(&body))),
        };
        Ok(Json(PreviewResponse {
            response: moleapi_core::Response {
                skipped: false,
                private_auth_values: vec![],
                soap_fault: None,
                request_updates: vec![],
                logs: vec![],
                variable_updates: vec![],
                status: status.as_u16(),
                status_text: status.canonical_reason().unwrap_or("Mock response").into(),
                headers: response_headers,
                body,
                body_base64,
                elapsed_ms: start.elapsed().as_millis() as u64,
                size_bytes,
                truncated: false,
                url: format!("http://mock.invalid{uri}"),
                tests: vec![],
            },
            matched,
            outcome,
        }))
    };
    let watch = watch_authenticated_revision(
        &state,
        &actor.0,
        &owner,
        &workspace,
        admission,
        canonical.revision,
        &headers,
    );
    let check = finish_specification_preview(
        &state.db,
        &actor.0,
        &owner,
        &workspace,
        (admission, canonical.revision),
        crate::auth::still_authenticated(&state, &actor.0, &headers),
    );
    with_access_watch(operation, watch, check).await
}
