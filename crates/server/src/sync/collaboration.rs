use super::*;
use serde_json::Value;

#[derive(Default, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Mode {
    #[default]
    Json,
    MockPreview,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Forward {
    session_id: String,
    method: String,
    path: String,
    body: Option<Value>,
    #[serde(default)]
    mode: Mode,
}
#[derive(Serialize)]
pub struct Reply {
    status: u16,
    body: Value,
    mock_preview: bool,
}
struct Target {
    method: Method,
    parts: Vec<String>,
    query: Vec<(String, String)>,
}
fn member_route(method: &Method, parts: &[&str], mode: &Mode) -> Option<&'static [&'static str]> {
    if *mode == Mode::MockPreview {
        return match parts {
            ["owners", _, "workspaces", _, "mock", _, _] if *method == Method::GET => Some(&[]),
            _ => None,
        };
    }
    match parts {
        ["workspaces", "shared"] if *method == Method::GET => Some(&["cursor"]),
        ["workspace-invitations", "accept"] if *method == Method::POST => Some(&[]),
        ["dubbo", "capability"] if *method == Method::GET => Some(&[]),
        ["owners", _, "workspaces", _, tail @ ..] => match tail {
            [] if matches!(*method, Method::GET | Method::PUT) => Some(&[]),
            ["invitations"] if matches!(*method, Method::GET | Method::POST) => Some(&[]),
            ["invitations", _] if *method == Method::DELETE => Some(&[]),
            ["members" | "audit" | "mock-calls" | "history"] if *method == Method::GET => Some(&[]),
            ["members", _] if matches!(*method, Method::PATCH | Method::DELETE) => Some(&[]),
            ["reports"] if *method == Method::GET => Some(&["cursor", "limit"]),
            ["reports", _] if *method == Method::GET => Some(&[]),
            ["reports", _, "export"] if *method == Method::GET => Some(&["format"]),
            ["reports", _, "steps", position, "response"]
                if *method == Method::GET
                    && !position.is_empty()
                    && position.bytes().all(|b| b.is_ascii_digit()) =>
            {
                Some(&[])
            }
            ["sessions", _] if matches!(*method, Method::GET | Method::DELETE) => Some(&[]),
            ["sessions", _, "events"] if *method == Method::GET => Some(&["after"]),
            ["sessions", _, "send" | "close"] if *method == Method::POST => Some(&[]),
            ["run" | "execute-draft" | "sessions"] if *method == Method::POST => Some(&[]),
            ["run", "cancel"]
            | ["requests", _, "execute"]
            | ["mock", "route-preview"]
            | ["dubbo", "cancel"]
            | ["dubbo", "metadata", "preview" | "import"]
            | ["a2a", "cards", "import" | "read" | "discover"]
            | ["a2a", "cards", "discover", "cancel"]
            | ["mcp", "config", "export"]
            | ["soap", "import" | "import-url" | "schema"]
            | ["grpc", "import" | "schema" | "reflect"]
            | ["graphql", "introspect" | "schema"]
            | ["specifications", "preview"]
            | ["dataset", "preview"]
            | ["mock-schema", "validate"]
                if *method == Method::POST =>
            {
                Some(&[])
            }
            _ => None,
        },
        _ => None,
    }
}
fn target(input: &Forward) -> Result<Target, ApiError> {
    let denied = || ApiError::not_found();
    if input.path.contains('#')
        || input.path.len() > 12 * 1024
        || input.session_id.is_empty()
        || input.session_id.len() > 128
    {
        return Err(denied());
    }
    let method = Method::from_bytes(input.method.as_bytes()).map_err(|_| denied())?;
    let uri: Uri = input.path.parse().map_err(|_| denied())?;
    if uri.scheme().is_some()
        || uri.authority().is_some()
        || !uri.path().starts_with("/api/")
        || uri.path().len() > 4096
    {
        return Err(denied());
    }
    let mut parts = Vec::new();
    for encoded in uri.path().trim_start_matches("/api/").split('/') {
        let bytes = encoded.as_bytes();
        if bytes.iter().enumerate().any(|(i, b)| {
            *b == b'%'
                && !bytes
                    .get(i + 1..i + 3)
                    .is_some_and(|escape| escape.iter().all(u8::is_ascii_hexdigit))
        }) {
            return Err(denied());
        }
        let part = percent_encoding::percent_decode_str(encoded)
            .decode_utf8()
            .map_err(|_| denied())?
            .into_owned();
        if part.is_empty()
            || part.len() > 256
            || matches!(part.as_str(), "." | "..")
            || part
                .chars()
                .any(|c| c.is_control() || matches!(c, '/' | '\\'))
        {
            return Err(denied());
        }
        parts.push(part);
    }
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    let keys = member_route(&method, &refs, &input.mode).ok_or_else(denied)?;
    let raw_query = uri.query().unwrap_or("");
    if raw_query.len() > 8192 {
        return Err(denied());
    }
    let mut query = Vec::new();
    for (key, value) in url::form_urlencoded::parse(raw_query.as_bytes()) {
        if !keys.contains(&key.as_ref()) || query.iter().any(|(existing, _)| existing == &key) {
            return Err(denied());
        }
        query.push((key.into_owned(), value.into_owned()));
    }
    if matches!(method, Method::GET | Method::DELETE) && input.body.is_some() {
        return Err(ApiError::bad(
            "Hosted member GET/DELETE requests cannot carry a body",
        ));
    }
    if input.body.as_ref().is_some_and(|value| {
        serde_json::to_vec(value).map_or(true, |bytes| bytes.len() > 25 * 1024 * 1024)
    }) {
        return Err(ApiError::bad("Hosted member request exceeds byte limit"));
    }
    Ok(Target {
        method,
        parts,
        query,
    })
}
async fn ensure_session(s: &AppState, session: &str) -> Result<(), ApiError> {
    if connection(s)
        .await?
        .is_some_and(|c| !c.session_id.is_empty() && c.session_id == session)
    {
        Ok(())
    } else {
        Err(ApiError::conflict(
            "Hosted connection changed; reload before continuing",
        ))
    }
}
fn contains_login(value: &Value, token: &str) -> bool {
    contains_login_inner(value, token, true)
}
fn contains_login_inner(value: &Value, token: &str, allow_json_text: bool) -> bool {
    match value {
        Value::String(text) => {
            text.contains(token)
                || (allow_json_text
                    && matches!(
                        text.trim_start().as_bytes().first(),
                        Some(b'{' | b'[' | b'"')
                    )
                    && serde_json::from_str::<Value>(text)
                        .is_ok_and(|value| contains_login_inner(&value, token, false)))
        }
        Value::Array(values) => values
            .iter()
            .any(|value| contains_login_inner(value, token, allow_json_text)),
        Value::Object(values) => values.iter().any(|(key, value)| {
            key.contains(token) || contains_login_inner(value, token, allow_json_text)
        }),
        _ => false,
    }
}
async fn send(
    snapshot: &Connection,
    input: Forward,
    target: Target,
) -> Result<Json<Reply>, ApiError> {
    let (client, mut url) = client_with_timeout(&snapshot.server_url, None).await?;
    url.path_segments_mut()
        .map_err(|_| ApiError::internal())?
        .clear()
        .push("api")
        .extend(target.parts);
    if !target.query.is_empty() {
        url.query_pairs_mut().extend_pairs(target.query);
    }
    let mut request = client
        .request(target.method, url)
        .bearer_auth(&snapshot.token);
    if let Some(body) = input.body {
        request = request.json(&body);
    }
    let response = request.send().await.map_err(|_| {
        ApiError::bad("Hosted member request failed; refresh before retrying a write")
    })?;
    let status = response.status();
    let preview = input.mode == Mode::MockPreview
        && response
            .headers()
            .get("x-moleapi-mock-preview")
            .is_some_and(|h| h == "saved-example");
    let limit = if input.mode == Mode::MockPreview {
        moleapi_core::MAX_BODY
    } else {
        25 * 1024 * 1024
    };
    if response.content_length().is_some_and(|n| n > limit as u64) {
        return Err(ApiError::bad("Hosted member response exceeds byte limit"));
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(part) = stream.next().await {
        let part = part.map_err(|_| {
            ApiError::bad("Reading hosted member response failed; refresh before retrying a write")
        })?;
        if bytes.len().saturating_add(part.len()) > limit {
            return Err(ApiError::bad("Hosted member response exceeds byte limit"));
        }
        bytes.extend_from_slice(&part);
    }
    let text = String::from_utf8_lossy(&bytes);
    let reflected = || ApiError::bad("Hosted member response contains connection credentials");
    if !snapshot.token.is_empty() && text.contains(&snapshot.token) {
        return Err(reflected());
    }
    let body = if input.mode == Mode::MockPreview {
        if !snapshot.token.is_empty()
            && serde_json::from_slice::<Value>(&bytes)
                .is_ok_and(|value| contains_login(&value, &snapshot.token))
        {
            return Err(reflected());
        }
        Value::String(text.into_owned())
    } else {
        match serde_json::from_slice(&bytes) {
            Ok(value) => value,
            Err(_) if !status.is_success() => {
                serde_json::json!({"error":format!("Hosted service rejected member request ({})",status.as_u16())})
            }
            Err(_) => return Err(ApiError::bad("Invalid hosted member JSON response")),
        }
    };
    if !snapshot.token.is_empty() && contains_login(&body, &snapshot.token) {
        return Err(reflected());
    }
    Ok(Json(Reply {
        status: status.as_u16(),
        body,
        mock_preview: preview,
    }))
}
pub async fn request(
    State(s): State<AppState>,
    Json(input): Json<Forward>,
) -> Result<Json<Reply>, ApiError> {
    native(&s)?;
    let target = target(&input)?;
    let session = input.session_id.clone();
    let snapshot = {
        let _guard = s.sync_lock.lock().await;
        ensure_session(&s, &session).await?;
        connection(&s).await?.ok_or_else(ApiError::not_found)?
    };
    let operation = send(&snapshot, input, target);
    let watch = async {
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;
            ensure_session(&s, &session).await?;
        }
        #[allow(unreachable_code)]
        Ok::<(), ApiError>(())
    };
    let result = tokio::select! {biased; revoked=watch=>{revoked?;return Err(ApiError::internal());}, result=operation=>result};
    let _guard = s.sync_lock.lock().await;
    ensure_session(&s, &session).await?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_registered_member_control_has_a_native_transport_policy() {
        let methods = [
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
        ];
        for registration in include_str!("../lib.rs").split(".route(").skip(1) {
            let Some(path) = registration
                .trim_start()
                .strip_prefix('"')
                .and_then(|s| s.split_once('"').map(|p| p.0))
            else {
                continue;
            };
            if !path.starts_with("/owners/") || path.ends_with("/mocks/{*path}") {
                continue;
            }
            let concrete = path.replace("{position}", "0");
            let parts = concrete
                .trim_start_matches('/')
                .split('/')
                .collect::<Vec<_>>();
            assert!(
                methods
                    .iter()
                    .any(|m| member_route(m, &parts, &Mode::Json).is_some()
                        || member_route(m, &parts, &Mode::MockPreview).is_some()),
                "Missing native member route policy: {path}"
            );
        }
    }
}
