//! EdgeGrid SDK adapter: source validation and the actual materialized HTTP request.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EdgeGridAuth {
    pub access_token: String,
    pub client_token: String,
    pub client_secret: String,
    pub base_url: String,
    pub headers_to_sign: Vec<String>,
    pub nonce: String,
    pub timestamp: String,
    pub max_body_bytes: usize,
}
impl Default for EdgeGridAuth {
    fn default() -> Self {
        Self {
            access_token: String::new(),
            client_token: String::new(),
            client_secret: String::new(),
            base_url: String::new(),
            headers_to_sign: Vec::new(),
            nonce: String::new(),
            timestamp: String::new(),
            max_body_bytes: 131072,
        }
    }
}
pub(crate) fn validate_edgegrid_fields(c: &EdgeGridAuth) -> Result<()> {
    ensure!(
        c.access_token.len() <= 4096
            && c.client_token.len() <= 4096
            && c.client_secret.len() <= 65536
            && c.base_url.len() <= 8192
            && c.nonce.len() <= 1024
            && c.timestamp.len() <= 128,
        "EdgeGrid fields exceed limits"
    );
    ensure!(
        c.headers_to_sign.len() <= 64 && c.headers_to_sign.iter().all(|s| s.len() <= 512),
        "EdgeGrid signed headers exceed limits"
    );
    ensure!(
        (1..=crate::MAX_BODY).contains(&c.max_body_bytes),
        "EdgeGrid body hash limit must be 1..5242880 bytes"
    );
    Ok(())
}
pub fn validate_edgegrid(c: &EdgeGridAuth, templates: bool) -> Result<()> {
    validate_edgegrid_fields(c)?;
    if !templates {
        ensure!(
            [&c.access_token, &c.client_token, &c.client_secret]
                .iter()
                .all(|s| !s.is_empty()),
            "EdgeGrid credentials are missing; supply them again"
        );
        for value in [&c.access_token, &c.client_token, &c.nonce] {
            ensure!(
                !value.chars().any(char::is_control)
                    && !value.contains(';')
                    && !value.chars().any(char::is_whitespace),
                "Invalid EdgeGrid authorization field"
            );
        }
        if !c.timestamp.is_empty() {
            parse_timestamp(&c.timestamp)?;
        }
        if !c.base_url.is_empty() {
            crate::valid_url(&c.base_url)?;
        }
    }
    let mut unique = std::collections::HashSet::new();
    for raw in &c.headers_to_sign {
        if raw.trim().is_empty() {
            continue;
        }
        if templates && raw.contains("{{") {
            continue;
        }
        let name = reqwest::header::HeaderName::from_bytes(raw.trim().as_bytes())
            .context("Invalid EdgeGrid signed header name")?;
        ensure!(
            ![
                "authorization",
                "host",
                "content-length",
                "transfer-encoding",
                "connection",
                "proxy-authorization",
                "proxy-connection",
                "upgrade",
                "trailer",
                "te"
            ]
            .contains(&name.as_str()),
            "EdgeGrid cannot sign transport-owned or authorization headers"
        );
        ensure!(
            unique.insert(name),
            "EdgeGrid signed header names must be unique"
        );
    }
    Ok(())
}
fn parse_timestamp(raw: &str) -> Result<chrono::DateTime<chrono::FixedOffset>> {
    let date = chrono::DateTime::parse_from_str(raw, "%Y%m%dT%H:%M:%S%z")
        .context("EdgeGrid timestamp must use yyyyMMddTHH:mm:ss+0000")?;
    ensure!(
        raw.len() == 22
            && raw.ends_with("+0000")
            && date.format("%Y%m%dT%H:%M:%S%z").to_string() == raw,
        "EdgeGrid timestamp must use yyyyMMddTHH:mm:ss+0000"
    );
    Ok(date)
}
pub fn sign_edgegrid_request(
    c: &EdgeGridAuth,
    request: &mut reqwest::Request,
    now: SystemTime,
) -> Result<Vec<String>> {
    validate_edgegrid(c, false)?;
    ensure!(
        !request.headers().contains_key("authorization"),
        "EdgeGrid signing owns the Authorization header"
    );
    let timestamp = if c.timestamp.is_empty() {
        chrono::DateTime::<chrono::Utc>::from(now)
            .format("%Y%m%dT%H:%M:%S+0000")
            .to_string()
    } else {
        c.timestamp.clone()
    };
    let nonce = if c.nonce.is_empty() {
        uuid::Uuid::new_v4().to_string()
    } else {
        c.nonce.clone()
    };
    let mut params = tropel_auth::edgegrid::EdgeGridBuildParams::new(
        request.method().as_str(),
        request.url().as_str(),
        &c.client_token,
        &c.access_token,
        &c.client_secret,
    );
    params.timestamp = Some(timestamp);
    params.nonce = Some(nonce);
    params.max_body = c.max_body_bytes;
    params.headers_to_sign = c
        .headers_to_sign
        .iter()
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_ascii_lowercase())
        .collect();
    params.body = request
        .body()
        .map(|body| {
            body.as_bytes()
                .context("EdgeGrid requires materialized body bytes")
                .map(|bytes| bytes.to_vec())
        })
        .transpose()?;
    if !c.base_url.is_empty() {
        let base = crate::valid_url(&c.base_url)?;
        params.authority_override =
            Some(base[url::Position::BeforeHost..url::Position::AfterPort].to_string());
    }
    let mut selected = Vec::new();
    for name in &params.headers_to_sign {
        let values: Vec<_> = request.headers().get_all(name).iter().collect();
        ensure!(
            values.len() <= 1,
            "EdgeGrid selected header has ambiguous duplicate values"
        );
        if let Some(value) = values.first() {
            selected.push((
                name.clone(),
                value
                    .to_str()
                    .context("Invalid EdgeGrid signed header value")?
                    .to_owned(),
            ));
        }
    }
    let header = tropel_auth::edgegrid::edgegrid_build_header(&params, &selected)
        .map_err(|_| anyhow::anyhow!("EdgeGrid request signing failed"))?;
    ensure!(
        header.len() <= 16384,
        "EdgeGrid generated header exceeds limit"
    );
    let signature = header
        .rsplit_once("signature=")
        .context("EdgeGrid signature missing")?
        .1
        .to_owned();
    request.headers_mut().insert(
        "authorization",
        reqwest::header::HeaderValue::from_str(&header)?,
    );
    Ok(vec![
        header,
        signature,
        c.access_token.clone(),
        c.client_token.clone(),
        c.client_secret.clone(),
    ])
}
