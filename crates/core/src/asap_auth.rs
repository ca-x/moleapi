//! ASAP claims adapter; mature JOSE/PEM/DER/data-URL libraries own key and JWS formats.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::time::{SystemTime, UNIX_EPOCH};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AsapAuth {
    pub algorithm: String,
    pub private_key: String,
    pub key_id: String,
    pub issuer: String,
    pub audience: Vec<String>,
    pub subject: String,
    pub ttl_seconds: u64,
    pub claims_source: String,
}
impl Default for AsapAuth {
    fn default() -> Self {
        Self {
            algorithm: "RS256".into(),
            private_key: String::new(),
            key_id: String::new(),
            issuer: String::new(),
            audience: Vec::new(),
            subject: String::new(),
            ttl_seconds: 3600,
            claims_source: "{}".into(),
        }
    }
}
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::Number(v) => v.as_f64().is_some_and(|v| v != 0.0),
        Value::String(v) => !v.is_empty(),
        _ => true,
    }
}
pub(crate) fn overrides(claims: &Map<String, Value>, name: &str) -> bool {
    claims.get(name).is_some_and(truthy)
}
pub(crate) fn validate_asap_fields(c: &AsapAuth) -> Result<()> {
    ensure!(
        c.algorithm.len() <= 128
            && c.private_key.len() <= 65536
            && c.key_id.len() <= 4096
            && c.issuer.len() <= 4096
            && c.subject.len() <= 4096
            && c.claims_source.len() <= 65536,
        "ASAP fields exceed limits"
    );
    ensure!(
        c.audience.len() <= 64 && c.audience.iter().all(|s| s.len() <= 4096),
        "ASAP audience exceeds limits"
    );
    ensure!(
        (1..=86400).contains(&c.ttl_seconds),
        "ASAP lifetime must be1..86400 seconds"
    );
    Ok(())
}
pub fn validate_asap(c: &AsapAuth, templates: bool) -> Result<()> {
    validate_asap_fields(c)?;
    if !(templates && c.algorithm.contains("{{")) {
        ensure!(
            [
                "RS256", "RS384", "RS512", "PS256", "PS384", "PS512", "ES256", "ES384", "ES512"
            ]
            .contains(&c.algorithm.as_str()),
            "Unsupported ASAP signing algorithm"
        );
    }
    let claims: Value =
        serde_json::from_str(&c.claims_source).context("Invalid ASAP claims JSON")?;
    ensure!(claims.is_object(), "ASAP claims must be an object");
    if !templates {
        ensure!(
            !c.private_key.is_empty(),
            "ASAP private key is missing; supply it again"
        );
        effective_claims(c, SystemTime::now())?;
    }
    Ok(())
}
fn string(claims: &Map<String, Value>, key: &str, fallback: &str) -> Result<String> {
    let value = if overrides(claims, key) {
        claims[key]
            .as_str()
            .context("ASAP identity claim must be a string")?
    } else {
        fallback
    };
    ensure!(
        !value.is_empty() && value.len() <= 4096 && !value.chars().any(char::is_control),
        "ASAP required identity claim is missing or invalid"
    );
    Ok(value.into())
}
fn date(value: &Value) -> Result<u64> {
    let n = if let Some(s) = value.as_str() {
        s.parse::<u64>()
            .context("ASAP numeric date must be an integer")?
    } else {
        value
            .as_u64()
            .context("ASAP numeric date must be an integer")?
    };
    ensure!(
        n <= 9_007_199_254_740_991,
        "ASAP numeric date exceeds safe integer range"
    );
    Ok(n)
}
fn effective_claims(c: &AsapAuth, now: SystemTime) -> Result<(Map<String, Value>, String)> {
    let mut claims = serde_json::from_str::<Value>(&c.claims_source)?
        .as_object()
        .cloned()
        .context("ASAP claims must be an object")?;
    let issuer = string(&claims, "iss", &c.issuer)?;
    let subject = if overrides(&claims, "sub") {
        string(&claims, "sub", "")?
    } else if !c.subject.is_empty() {
        string(&Map::new(), "sub", &c.subject)?
    } else {
        issuer.clone()
    };
    let kid = string(&claims, "kid", &c.key_id)?;
    let audience = if overrides(&claims, "aud") {
        claims["aud"].clone()
    } else if c.audience.len() == 1 {
        Value::String(c.audience[0].clone())
    } else {
        serde_json::to_value(&c.audience)?
    };
    let values: Vec<&str> = match &audience {
        Value::String(s) => vec![s],
        Value::Array(items) => items
            .iter()
            .map(|v| v.as_str().context("ASAP audience must contain strings"))
            .collect::<Result<_>>()?,
        _ => anyhow::bail!("ASAP audience must be a string or array"),
    };
    ensure!(
        !values.is_empty()
            && values.len() <= 64
            && values
                .iter()
                .all(|s| !s.is_empty() && s.len() <= 4096 && !s.chars().any(char::is_control)),
        "ASAP audience is missing or invalid"
    );
    let now = now.duration_since(UNIX_EPOCH)?.as_secs();
    ensure!(
        now <= 9_007_199_254_654_591,
        "ASAP clock exceeds supported range"
    );
    let issued = if overrides(&claims, "iat") {
        date(&claims["iat"])?
    } else {
        now
    };
    let expires = if overrides(&claims, "exp") {
        date(&claims["exp"])?
    } else {
        now.checked_add(c.ttl_seconds)
            .context("ASAP expiry overflow")?
    };
    let jti = if overrides(&claims, "jti") {
        string(&claims, "jti", "")?
    } else {
        uuid::Uuid::new_v4().to_string()
    };
    claims.insert("iss".into(), issuer.into());
    claims.insert("sub".into(), subject.into());
    claims.insert("aud".into(), audience);
    claims.insert("jti".into(), jti.into());
    claims.insert("iat".into(), issued.into());
    claims.insert("exp".into(), expires.into());
    ensure!(
        serde_json::to_vec(&claims)?.len() <= 65536,
        "Resolved ASAP claims exceed64KiB"
    );
    Ok((claims, kid))
}
fn decoded_source(raw: &str) -> Result<String> {
    let raw = raw.trim();
    let raw = raw
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(raw);
    ensure!(raw.len() <= 65536, "ASAP private key source exceeds limit");
    Ok(percent_encoding::percent_decode_str(raw)
        .decode_utf8()
        .context("ASAP private key source is not UTF-8")?
        .into_owned())
}
pub(crate) fn private_key_der(c: &AsapAuth, kid: &str) -> Result<Vec<u8>> {
    let text = decoded_source(&c.private_key)?;
    let der = if text.starts_with("data:") {
        let uri = data_url::DataUrl::process(&text)
            .map_err(|_| anyhow::anyhow!("Invalid ASAP key data URI"))?;
        ensure!(
            uri.mime_type().matches("application", "pkcs8"),
            "ASAP key data URI must use application/pkcs8"
        );
        ensure!(
            uri.mime_type().get_parameter("kid") == Some(kid),
            "ASAP key data URI kid does not match configured key ID"
        );
        let (bytes, fragment) = uri
            .decode_to_vec()
            .map_err(|_| anyhow::anyhow!("Invalid ASAP key data URI encoding"))?;
        ensure!(
            fragment.is_none(),
            "ASAP key data URI cannot contain a fragment"
        );
        bytes
    } else {
        let key = pem::parse(&text).map_err(|_| anyhow::anyhow!("Invalid ASAP private PEM key"))?;
        ensure!(key.contents().len() <= 16384, "ASAP key DER exceeds16KiB");
        use rsa::pkcs8::EncodePrivateKey;
        match key.tag() {
            "PRIVATE KEY" => key.contents().to_vec(),
            "RSA PRIVATE KEY" => {
                use rsa::pkcs1::DecodeRsaPrivateKey;
                rsa::RsaPrivateKey::from_pkcs1_der(key.contents())
                    .map_err(|_| anyhow::anyhow!("Invalid ASAP RSA private key"))?
                    .to_pkcs8_der()?
                    .as_bytes()
                    .to_vec()
            }
            "EC PRIVATE KEY" => match c.algorithm.as_str() {
                "ES256" => p256::SecretKey::from_sec1_der(key.contents())
                    .map_err(|_| anyhow::anyhow!("Invalid ASAP P-256 key"))?
                    .to_pkcs8_der()?
                    .as_bytes()
                    .to_vec(),
                "ES384" => p384::SecretKey::from_sec1_der(key.contents())
                    .map_err(|_| anyhow::anyhow!("Invalid ASAP P-384 key"))?
                    .to_pkcs8_der()?
                    .as_bytes()
                    .to_vec(),
                "ES512" => p521::SecretKey::from_sec1_der(key.contents())
                    .map_err(|_| anyhow::anyhow!("Invalid ASAP P-521 key"))?
                    .to_pkcs8_der()?
                    .as_bytes()
                    .to_vec(),
                _ => anyhow::bail!("ASAP EC key does not match selected algorithm"),
            },
            _ => anyhow::bail!("ASAP requires an unencrypted private PEM key"),
        }
    };
    ensure!(der.len() <= 16384, "ASAP key DER exceeds16KiB");
    if c.algorithm.starts_with("RS") || c.algorithm.starts_with("PS") {
        use rsa::{pkcs8::DecodePrivateKey, traits::PublicKeyParts};
        let key = rsa::RsaPrivateKey::from_pkcs8_der(&der)
            .map_err(|_| anyhow::anyhow!("Invalid ASAP RSA private key"))?;
        ensure!(
            (2048..=4096).contains(&key.n().bits()),
            "ASAP RSA key must be2048..4096 bits"
        );
        key.validate()
            .map_err(|_| anyhow::anyhow!("Invalid ASAP RSA private key"))?;
    }
    Ok(der)
}
pub fn sign_asap(c: &AsapAuth, now: SystemTime) -> Result<String> {
    validate_asap_fields(c)?;
    ensure!(
        [
            "RS256", "RS384", "RS512", "PS256", "PS384", "PS512", "ES256", "ES384", "ES512"
        ]
        .contains(&c.algorithm.as_str()),
        "Unsupported ASAP signing algorithm"
    );
    let (claims, kid) = effective_claims(c, now)?;
    let der = private_key_der(c, &kid)?;
    let mut jwk = jose_rs::jwk::jwk_from_pkcs8_der(&der)
        .map_err(|_| anyhow::anyhow!("ASAP private key cannot be decoded for signing"))?;
    jwk.alg = Some(c.algorithm.clone());
    let mut header = jose_rs::header::JoseHeader::new(&c.algorithm);
    header.kid = Some(kid);
    let token = jose_rs::jws::compact::sign_with_jwk(&jwk, &serde_json::to_vec(&claims)?, &header)
        .map_err(|_| anyhow::anyhow!("ASAP key is incompatible or signing failed"))?;
    ensure!(
        token.len() <= 128 * 1024,
        "Generated ASAP token exceeds128KiB"
    );
    Ok(token)
}
pub fn sign_asap_request(
    c: &AsapAuth,
    request: &mut reqwest::Request,
    now: SystemTime,
) -> Result<Vec<String>> {
    ensure!(
        !request.headers().contains_key("authorization"),
        "ASAP signing owns the Authorization header"
    );
    let token = sign_asap(c, now)?;
    let value = format!("Bearer {token}");
    request
        .headers_mut()
        .insert("authorization", value.parse()?);
    Ok(vec![token, value, c.private_key.clone()])
}
/// Expanded key representations for the existing export/history privacy pipeline.
pub fn asap_private_sources(c: &AsapAuth) -> Vec<String> {
    let mut values = vec![c.private_key.clone()];
    if let Ok(text) = decoded_source(&c.private_key) {
        values.push(text.clone());
        let raw = if text.starts_with("data:") {
            data_url::DataUrl::process(&text)
                .ok()
                .and_then(|url| url.decode_to_vec().ok().map(|(bytes, _)| bytes))
        } else {
            pem::parse(&text).ok().map(|key| key.contents().to_vec())
        };
        if let Some(der) = raw
            && der.len() <= 16384
        {
            use base64::{Engine, engine::general_purpose::STANDARD};
            values.push(STANDARD.encode(&der));
            if text.starts_with("data:")
                || pem::parse(&text).is_ok_and(|p| p.tag() == "PRIVATE KEY")
            {
                values.push(pem::encode(&pem::Pem::new("PRIVATE KEY", der)));
            }
        }
    }
    if let Ok(claims) = serde_json::from_str::<Value>(&c.claims_source)
        && let Some(claims) = claims.as_object()
        && let Ok(kid) = string(claims, "kid", &c.key_id)
        && let Ok(der) = private_key_der(c, &kid)
    {
        use base64::{Engine, engine::general_purpose::STANDARD};
        values.push(STANDARD.encode(&der));
        values.push(pem::encode(&pem::Pem::new("PRIVATE KEY", der)));
    }
    let unix: Vec<String> = values
        .iter()
        .filter(|value| value.starts_with("-----BEGIN "))
        .map(|value| value.replace("\r\n", "\n"))
        .collect();
    values.extend(unix);
    values
}
