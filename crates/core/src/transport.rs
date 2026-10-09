use crate::*;
use anyhow::{Context, Result, ensure};
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
    execute_with_cookies(request, environment, policy, None).await
}
/// Shared finite transport with an optional private session jar.
pub async fn execute_with_cookies(
    request: &RequestSpec,
    environment: Option<&Environment>,
    policy: NetworkPolicy,
    cookies: Option<&CookieJar>,
) -> Result<Response> {
    ensure!(
        request.protocol == Protocol::Http
            || request.protocol.is_graphql()
            || request.protocol.is_soap(),
        "Live protocols require the session API"
    );
    let prepared = prepare_soap(&prepare_graphql(request)?)?;
    let r = prepare_authentication(&resolve_request(&prepared, environment)?)?;
    validate_soap(&r, false)?;
    if r.protocol.is_graphql() {
        ensure!(
            !graphql_is_subscription(&r)?,
            "GraphQL subscriptions require the session API"
        );
    }
    tokio::time::timeout(
        Duration::from_millis(r.timeout_ms),
        execute_inner(&r, policy, None, cookies),
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
    let prepared = prepare_authentication(request)?;
    tokio::time::timeout(
        Duration::from_millis(prepared.timeout_ms),
        execute_inner(&prepared, policy, Some(body), None),
    )
    .await
    .context("Request timed out")?
}
async fn execute_inner(
    r: &RequestSpec,
    policy: NetworkPolicy,
    raw_body: Option<Vec<u8>>,
    cookies: Option<&CookieJar>,
) -> Result<Response> {
    let cookie_generation = cookies.map(CookieJar::generation);
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
    let structured = if raw_body.is_none() && matches!(r.body_kind.as_str(), "binary" | "multipart")
    {
        Some(crate::request_body::prepare_structured_body(r).await?)
    } else {
        None
    };
    if let Some((_, content_type)) = &structured
        && (r.body_kind == "multipart" || !headers.contains_key("content-type"))
    {
        headers.insert("content-type", HeaderValue::from_str(content_type)?);
    }
    let mut body = raw_body
        .or_else(|| structured.map(|(bytes, _)| bytes))
        .or_else(|| {
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
    let mut network = r.network.clone();
    let mut redirect = 0;
    let mut private_auth_values = Vec::new();
    let mut digest_attempts = 0;
    let mut digest_allowed = r.auth.kind == "digest";
    let mut ntlm_allowed = r.auth.kind == "ntlm";
    let mut ntlm_connection: Option<crate::ntlm_auth::Connection> = None;
    let mut ntlm_handshake: Option<crate::ntlm_auth::Handshake> = None;
    let mut signing_allowed = matches!(
        r.auth.kind.as_str(),
        "aws" | "hawk" | "oauth1" | "edgegrid" | "asap"
    );
    loop {
        let client = if ntlm_allowed {
            if ntlm_connection.is_none() {
                let binding = r.auth.ntlm.as_deref().is_none_or(|c| c.channel_binding);
                ntlm_connection = Some(
                    crate::ntlm_auth::Connection::connect(
                        &url,
                        policy,
                        r.verify_tls,
                        binding,
                        network.as_deref(),
                    )
                    .await?,
                );
            }
            None
        } else {
            Some(
                crate::checked_request_client(&url, policy, r.verify_tls, network.as_deref())
                    .await?,
            )
        };
        let mut request = reqwest::Request::new(method.clone(), url.clone());
        *request.headers_mut() = headers.clone();
        if let Some(bytes) = &body {
            *request.body_mut() = Some(bytes.clone().into());
        }
        if !request.headers().contains_key("cookie")
            && let (Some(jar), Some(generation)) = (cookies, cookie_generation)
            && let Some(value) = jar.header(&url, generation)
        {
            private_auth_values.push(value.clone());
            for pair in value.split("; ") {
                if let Some((_, value)) = pair.split_once('=') {
                    private_auth_values.push(value.into());
                }
            }
            request
                .headers_mut()
                .insert("cookie", HeaderValue::from_str(&value)?);
        }
        if signing_allowed && r.auth.kind == "asap" {
            private_auth_values.extend(crate::sign_asap_request(
                r.auth.asap.as_deref().context("ASAP settings missing")?,
                &mut request,
                std::time::SystemTime::now(),
            )?);
        }
        if signing_allowed && r.auth.kind == "edgegrid" {
            private_auth_values.extend(crate::sign_edgegrid_request(
                r.auth
                    .edgegrid
                    .as_deref()
                    .context("EdgeGrid settings missing")?,
                &mut request,
                std::time::SystemTime::now(),
            )?);
        }
        if signing_allowed && r.auth.kind == "oauth1" {
            private_auth_values.extend(crate::sign_oauth1_request(
                r.auth.oauth1.as_ref().context("OAuth1 settings missing")?,
                &mut request,
                std::time::SystemTime::now(),
            )?);
        }
        if signing_allowed && r.auth.kind == "aws" {
            private_auth_values.extend(crate::sign_aws_request(
                r.auth.aws.as_ref().context("AWS settings missing")?,
                &mut request,
                std::time::SystemTime::now(),
            )?);
        }
        if signing_allowed && r.auth.kind == "hawk" {
            private_auth_values.extend(crate::sign_hawk_request(
                r.auth.hawk.as_ref().context("Hawk settings missing")?,
                &mut request,
                std::time::SystemTime::now(),
            )?);
        }
        let response = if let Some(connection) = ntlm_connection.as_mut() {
            connection.send(request).await?
        } else {
            client
                .context("HTTP client missing")?
                .execute(request)
                .await
                .context("HTTP request failed")?
        };
        for raw in response.headers().get_all("set-cookie").iter() {
            if let Ok(raw) = raw.to_str() {
                private_auth_values.push(raw.into());
                if let Ok(parsed) = cookie_store::Cookie::parse(raw, &url) {
                    private_auth_values.push(parsed.value().into());
                }
                if let (Some(jar), Some(generation)) = (cookies, cookie_generation) {
                    jar.receive(&url, raw, generation);
                }
            }
        }
        let status = response.status();
        if status.as_u16() == 401
            && ntlm_allowed
            && ntlm_handshake.as_ref().is_none_or(|h| !h.complete())
        {
            let challenge = crate::ntlm_auth::challenge(response.headers())?;
            ensure!(
                ntlm_handshake.is_none() || challenge.is_some(),
                "NTLM server did not send a Type2 challenge"
            );
            if let Some(challenge) = challenge {
                ensure!(
                    !response
                        .headers()
                        .get_all("connection")
                        .iter()
                        .any(|v| v.to_str().is_ok_and(|s| s
                            .split(',')
                            .any(|v| v.trim().eq_ignore_ascii_case("close")))),
                    "NTLM server closed the authentication connection"
                );
                if ntlm_handshake.is_none() {
                    let host = url
                        .host_str()
                        .context("NTLM host missing")?
                        .trim_matches(['[', ']']);
                    let binding = ntlm_connection.as_ref().and_then(|c| c.binding.as_deref());
                    ntlm_handshake =
                        Some(crate::ntlm_auth::Handshake::new(&r.auth, host, binding)?);
                }
                let header = ntlm_handshake
                    .as_mut()
                    .unwrap()
                    .next(challenge.as_deref())?;
                private_auth_values.push(header.clone());
                if let Some(encoded) = header.strip_prefix("NTLM ") {
                    private_auth_values.push(encoded.into());
                }
                crate::ntlm_auth::drain(response).await?;
                headers.insert("authorization", HeaderValue::from_str(&header)?);
                continue;
            }
        }
        if status.as_u16() == 401 && digest_allowed && digest_attempts < 2 {
            let mut challenge = None;
            for value in response
                .headers()
                .get_all("www-authenticate")
                .iter()
                .take(16)
            {
                let value = value.to_str().context("Invalid Digest challenge header")?;
                ensure!(value.len() <= 16384, "Digest challenge exceeds limit");
                if let Ok(parsed) = digest_auth::parse(value) {
                    challenge = Some(parsed);
                    break;
                }
            }
            if let Some(mut challenge) = challenge
                && (digest_attempts == 0 || challenge.stale)
            {
                let target = match url.query() {
                    Some(query) => format!("{}?{query}", url.path()),
                    None => url.path().to_owned(),
                };
                let context = digest_auth::AuthContext::new_with_method(
                    &r.auth.username,
                    &r.auth.password,
                    target,
                    Some(body.as_deref().unwrap_or(&[])),
                    method.as_str().into(),
                );
                let answer = challenge.respond(&context)?.to_string();
                ensure!(answer.len() <= 16384, "Digest answer exceeds limit");
                private_auth_values.push(answer.clone());
                headers.insert("authorization", HeaderValue::from_str(&answer)?);
                digest_attempts += 1;
                drop(response);
                continue;
            }
        }
        if r.follow_redirects
            && matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308)
            && let Some(location) = response.headers().get("location")
        {
            ensure!(redirect < 10, "Too many redirects");
            let mut next = url.join(location.to_str().context("Invalid redirect location")?)?;
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
                if let Some(network) = &mut network {
                    network.identity.enabled = false;
                }
                headers.remove("authorization");
                headers.remove("cookie");
                for row in r.headers.iter().filter(|row| {
                    row.enabled
                        && (row.secret == Some(true) || crate::sensitive_query_key(&row.key))
                }) {
                    if let Ok(name) = HeaderName::from_bytes(row.key.as_bytes()) {
                        headers.remove(name);
                    }
                }
                let filtered = next
                    .query_pairs()
                    .filter(|(name, _)| {
                        !crate::sensitive_query_key(name)
                            && !r.query.iter().any(|row| {
                                row.enabled && row.secret == Some(true) && row.key == name.as_ref()
                            })
                    })
                    .map(|(k, v)| (k.into_owned(), v.into_owned()))
                    .collect::<Vec<_>>();
                next.set_query(None);
                if !filtered.is_empty() {
                    next.query_pairs_mut().extend_pairs(filtered);
                }
                digest_allowed = false;
                signing_allowed = false;
                ntlm_allowed = false;
            } else if r.auth.kind == "digest" {
                headers.remove("authorization");
                digest_attempts = 0;
            }
            if r.auth.kind == "ntlm" {
                headers.remove("authorization");
                ntlm_handshake = None;
                ntlm_connection = None;
            }
            if status.as_u16() == 303 && method != Method::HEAD
                || matches!(status.as_u16(), 301 | 302) && method == Method::POST
            {
                method = Method::GET;
                body = None;
                headers.remove("content-type");
            }
            redirect += 1;
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
            skipped: false,
            private_auth_values,
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
}

/// Shared authentication and forbidden hop-header handling for HTTP and live protocols.
pub fn request_headers(r: &RequestSpec) -> Result<HeaderMap> {
    let prepared = prepare_authentication(r)?;
    let r = &prepared;
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
    if matches!(r.auth.kind.as_str(), "digest" | "ntlm") {
        headers.remove("authorization");
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
