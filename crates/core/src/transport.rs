use crate::*;
use anyhow::{Context, Result, bail, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use futures_util::StreamExt;
use reqwest::{
    Method,
    header::{HeaderMap, HeaderName, HeaderValue},
};
use std::time::{Duration, Instant};
pub async fn execute(
    request: &RequestSpec,
    environment: Option<&Environment>,
    policy: NetworkPolicy,
) -> Result<Response> {
    ensure!(
        request.protocol == Protocol::Http
            || request.protocol.is_graphql()
            || request.protocol.is_soap(),
        "Live protocols require the session API"
    );
    let prepared = prepare_soap(&prepare_graphql(request)?)?;
    let r = resolve_request(&prepared, environment)?;
    validate_soap(&r, false)?;
    if r.protocol.is_graphql() {
        ensure!(
            !graphql_is_subscription(&r)?,
            "GraphQL subscriptions require the session API"
        );
    }
    tokio::time::timeout(
        Duration::from_millis(r.timeout_ms),
        execute_inner(&r, policy, None),
    )
    .await
    .context("Request timed out")?
}
/// Checked HTTP transport for explicit byte replay; no scripts or source interpolation.
pub async fn execute_bytes(
    request: &RequestSpec,
    body: Vec<u8>,
    policy: NetworkPolicy,
) -> Result<Response> {
    ensure!(
        request.protocol == Protocol::Http,
        "Raw byte execution requires HTTP"
    );
    crate::validate_request(request, false)?;
    ensure!(body.len() <= crate::MAX_BODY, "Raw HTTP body exceeds 5 MiB");
    tokio::time::timeout(
        Duration::from_millis(request.timeout_ms),
        execute_inner(request, policy, Some(body)),
    )
    .await
    .context("Request timed out")?
}
async fn execute_inner(
    r: &RequestSpec,
    policy: NetworkPolicy,
    raw_body: Option<Vec<u8>>,
) -> Result<Response> {
    let start = Instant::now();
    let mut url = valid_url(&r.url)?;
    {
        let mut query = url.query_pairs_mut();
        for p in r.query.iter().filter(|p| p.enabled) {
            query.append_pair(&p.key, &p.value);
        }
    }
    let mut method = Method::from_bytes(r.method.as_bytes())?;
    let mut headers = request_headers(r)?;
    let mut body = raw_body.or_else(|| {
        if r.body_kind == "none" {
            None
        } else {
            Some(r.body.as_bytes().to_vec())
        }
    });
    if r.body_kind == "json" && !headers.contains_key("content-type") {
        headers.insert("content-type", HeaderValue::from_static("application/json"));
    }
    if r.body_kind == "form" && !headers.contains_key("content-type") {
        headers.insert(
            "content-type",
            HeaderValue::from_static("application/x-www-form-urlencoded"),
        );
    }
    for redirect in 0..=10 {
        let client = checked_client(&url, policy, r.verify_tls).await?;
        let mut builder = client
            .request(method.clone(), url.clone())
            .headers(headers.clone());
        if let Some(b) = &body {
            builder = builder.body(b.clone());
        }
        let response = builder.send().await.context("HTTP request failed")?;
        let status = response.status();
        if r.follow_redirects
            && matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308)
            && let Some(location) = response.headers().get("location")
        {
            ensure!(redirect < 10, "Too many redirects");
            let next = url.join(location.to_str().context("Invalid redirect location")?)?;
            valid_url(next.as_str())?;
            if r.protocol.is_soap() {
                ensure!(
                    matches!(status.as_u16(), 307 | 308),
                    "SOAP redirects must preserve POST (307/308)"
                );
                ensure!(
                    url.origin() == next.origin(),
                    "Cross-origin SOAP redirects are blocked to protect envelope credentials"
                );
            }
            ensure!(
                url.scheme() != "https" || next.scheme() == "https",
                "HTTPS downgrade redirect blocked"
            );
            if url.origin() != next.origin() {
                headers.remove("authorization");
                headers.remove("cookie");
            }
            if status.as_u16() == 303 && method != Method::HEAD
                || matches!(status.as_u16(), 301 | 302) && method == Method::POST
            {
                method = Method::GET;
                body = None;
                headers.remove("content-type");
            }
            url = next;
            continue;
        }
        let response_headers = response
            .headers()
            .iter()
            .map(|(name, value)| Pair {
                id: uuid::Uuid::new_v4().to_string(),
                key: name.to_string(),
                value: if name == "set-cookie" {
                    "[REDACTED]".into()
                } else {
                    value.to_str().unwrap_or("[binary]").into()
                },
                enabled: true,
                secret: None,
                local_value: None,
            })
            .collect();
        let mut bytes = Vec::new();
        let mut truncated = false;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Reading response failed")?;
            let remaining = MAX_BODY - bytes.len();
            if chunk.len() > remaining {
                bytes.extend_from_slice(&chunk[..remaining]);
                truncated = true;
                break;
            }
            bytes.extend_from_slice(&chunk);
        }
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let mut result = Response {
            soap_fault: None,
            request_updates: vec![],
            logs: vec![],
            variable_updates: vec![],
            status: status.as_u16(),
            status_text: status.canonical_reason().unwrap_or("").into(),
            headers: response_headers,
            body: text,
            body_base64: std::str::from_utf8(&bytes)
                .is_err()
                .then(|| STANDARD.encode(&bytes)),
            elapsed_ms: start.elapsed().as_millis() as u64,
            size_bytes: bytes.len(),
            truncated,
            url: url.to_string(),
            tests: vec![],
        };
        if r.protocol.is_soap() && !result.truncated {
            result.soap_fault = soap_fault(&result.body);
        }
        result.tests = assertions(&r.assertions, &result);
        return Ok(result);
    }
    bail!("Too many redirects")
}

/// Shared authentication and forbidden hop-header handling for HTTP and live protocols.
pub fn request_headers(r: &RequestSpec) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    for p in r.headers.iter().filter(|p| p.enabled) {
        headers.append(
            HeaderName::from_bytes(p.key.as_bytes())?,
            HeaderValue::from_str(&p.value)?,
        );
    }
    // Let the transport calculate framing and destination headers.
    for name in [
        "host",
        "content-length",
        "transfer-encoding",
        "connection",
        "proxy-authorization",
        "proxy-connection",
        "upgrade",
        "trailer",
        "te",
    ] {
        headers.remove(name);
    }
    if r.auth.kind == "bearer" {
        headers.insert(
            "authorization",
            HeaderValue::from_str(&format!("Bearer {}", r.auth.token))?,
        );
    }
    if r.auth.kind == "basic" {
        headers.insert(
            "authorization",
            HeaderValue::from_str(&format!(
                "Basic {}",
                STANDARD.encode(format!("{}:{}", r.auth.username, r.auth.password))
            ))?,
        );
    }
    Ok(headers)
}
