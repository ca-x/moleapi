//! Source and HTTP adapter; the mature Hawk SDK owns MAC, payload hash and header format.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HawkAuth {
    pub id: String,
    pub key: String,
    pub algorithm: String,
    pub nonce: String,
    pub timestamp: String,
    pub ext: String,
    pub app: String,
    pub delegation: String,
    pub user: String,
    pub include_payload_hash: bool,
}
impl Default for HawkAuth {
    fn default() -> Self {
        Self {
            id: String::new(),
            key: String::new(),
            algorithm: "sha256".into(),
            nonce: String::new(),
            timestamp: String::new(),
            ext: String::new(),
            app: String::new(),
            delegation: String::new(),
            user: String::new(),
            include_payload_hash: false,
        }
    }
}
pub(crate) fn validate_hawk_fields(config: &HawkAuth) -> Result<()> {
    ensure!(
        config.id.len() <= 4096
            && config.key.len() <= 65536
            && config.algorithm.len() <= 128
            && config.nonce.len() <= 1024
            && config.timestamp.len() <= 128
            && config.ext.len() <= 4096
            && config.app.len() <= 1024
            && config.delegation.len() <= 1024
            && config.user.len() <= 4096,
        "Hawk fields exceed limits"
    );
    Ok(())
}
pub fn validate_hawk(config: &HawkAuth, templates: bool) -> Result<()> {
    validate_hawk_fields(config)?;
    if !(templates && config.algorithm.contains("{{")) {
        ensure!(
            ["sha1", "sha256", "sha384", "sha512"]
                .contains(&config.algorithm.to_ascii_lowercase().as_str()),
            "Unsupported Hawk algorithm"
        );
    }
    if !templates {
        ensure!(
            !config.id.is_empty() && !config.key.is_empty(),
            "Hawk ID/key are missing; supply them again"
        );
        for value in [&config.id, &config.nonce, &config.app, &config.delegation] {
            ensure!(
                !value.chars().any(char::is_control) && !value.contains('"'),
                "Hawk header fields contain unsupported characters"
            );
        }
        ensure!(
            !config.ext.chars().any(char::is_control),
            "Hawk extra data contains control characters"
        );
        ensure!(
            config.delegation.is_empty() || !config.app.is_empty(),
            "Hawk delegation requires an application ID"
        );
        if !config.timestamp.is_empty() {
            let timestamp = config
                .timestamp
                .parse::<u64>()
                .context("Hawk timestamp must be Unix seconds")?;
            ensure!(
                std::time::UNIX_EPOCH
                    .checked_add(std::time::Duration::from_secs(timestamp))
                    .is_some(),
                "Hawk timestamp exceeds supported range"
            );
        }
    }
    Ok(())
}
pub fn sign_hawk_request(
    config: &HawkAuth,
    request: &mut reqwest::Request,
    time: std::time::SystemTime,
) -> Result<Vec<String>> {
    validate_hawk(config, false)?;
    ensure!(
        !request.headers().contains_key("authorization"),
        "Hawk request already contains an authorization header"
    );
    let algorithm = match config.algorithm.to_ascii_lowercase().as_str() {
        "sha1" => hawk::DigestAlgorithm::Sha1,
        "sha256" => hawk::DigestAlgorithm::Sha256,
        "sha384" => hawk::DigestAlgorithm::Sha384,
        "sha512" => hawk::DigestAlgorithm::Sha512,
        _ => unreachable!(),
    };
    let credentials = hawk::Credentials {
        id: config.id.clone(),
        key: hawk::Key::new(config.key.as_bytes(), algorithm)
            .map_err(|_| anyhow::anyhow!("Hawk credentials cannot be prepared"))?,
    };
    let host = match request
        .url()
        .host()
        .context("Hawk request host is missing")?
    {
        url::Host::Domain(host) => host.to_owned(),
        url::Host::Ipv4(ip) => ip.to_string(),
        url::Host::Ipv6(ip) => ip.to_string(),
    };
    let port = request
        .url()
        .port_or_known_default()
        .context("Hawk request port is missing")?;
    let path = match request.url().query() {
        Some(query) => format!("{}?{query}", request.url().path()),
        None => request.url().path().to_owned(),
    };
    let hash = if config.include_payload_hash {
        let body = request
            .body()
            .map(|body| {
                body.as_bytes()
                    .context("Hawk signing requires materialized body bytes")
            })
            .transpose()?
            .unwrap_or(&[]);
        let content_type = request
            .headers()
            .get("content-type")
            .map(|value| value.to_str())
            .transpose()?
            .unwrap_or("");
        let normalized = if content_type.is_empty() {
            String::new()
        } else {
            let mime: mime::Mime = content_type
                .parse()
                .context("Invalid Hawk payload MIME type")?;
            mime.essence_str().to_ascii_lowercase()
        };
        Some(
            hawk::PayloadHasher::hash(&normalized, algorithm, body)
                .map_err(|_| anyhow::anyhow!("Hawk payload cannot be hashed"))?,
        )
    } else {
        None
    };
    let mut builder = hawk::RequestBuilder::new(request.method().as_str(), &host, port, &path)
        .hash(hash.as_deref());
    if !config.ext.is_empty() {
        builder = builder.ext(config.ext.as_str());
    }
    if !config.app.is_empty() {
        builder = builder.app(config.app.as_str());
    }
    if !config.delegation.is_empty() {
        builder = builder.dlg(config.delegation.as_str());
    }
    let timestamp = if config.timestamp.is_empty() {
        time
    } else {
        std::time::UNIX_EPOCH
            .checked_add(std::time::Duration::from_secs(config.timestamp.parse()?))
            .context("Invalid Hawk timestamp")?
    };
    let nonce = if config.nonce.is_empty() {
        uuid::Uuid::new_v4().simple().to_string()
    } else {
        config.nonce.clone()
    };
    let header = builder
        .request()
        .make_header_full(&credentials, timestamp, nonce)
        .map_err(|_| anyhow::anyhow!("Hawk request cannot be signed"))?;
    let value = format!("Hawk {header}");
    ensure!(value.len() <= 16384, "Hawk header exceeds limit");
    let mut private = vec![value.clone()];
    if let Some(hash) = &header.hash {
        use base64::{Engine, engine::general_purpose::STANDARD};
        private.push(STANDARD.encode(hash));
    }
    if let Some(mac) = header.mac {
        use base64::{Engine, engine::general_purpose::STANDARD};
        private.push(STANDARD.encode(mac));
    }
    request.headers_mut().insert(
        "authorization",
        reqwest::header::HeaderValue::from_str(&value)?,
    );
    Ok(private)
}
