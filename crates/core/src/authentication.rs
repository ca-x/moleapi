use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthLocation {
    #[default]
    Header,
    Query,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ApiKeyAuth {
    pub name: String,
    pub value: String,
    pub location: AuthLocation,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct JwtAuth {
    pub algorithm: String,
    pub key: String,
    pub key_base64: bool,
    pub claims_source: String,
    pub kid: String,
    pub name: String,
    pub prefix: String,
    pub location: AuthLocation,
    pub add_time_claims: bool,
    pub ttl_seconds: u64,
}
impl Default for JwtAuth {
    fn default() -> Self {
        Self {
            algorithm: "HS256".into(),
            key: String::new(),
            key_base64: false,
            claims_source: "{}".into(),
            kid: String::new(),
            name: "Authorization".into(),
            prefix: "Bearer".into(),
            location: AuthLocation::Header,
            add_time_claims: true,
            ttl_seconds: 3600,
        }
    }
}
pub fn validate_authentication(auth: &crate::Auth, templates: bool) -> Result<()> {
    ensure!(
        [
            "none", "basic", "bearer", "apikey", "jwt", "digest", "oauth2", "aws", "hawk",
            "oauth1", "ntlm", "edgegrid", "asap"
        ]
        .contains(&auth.kind.as_str())
            || templates && auth.kind == "inherit"
            || templates && auth.kind.contains("{{"),
        "Unsupported authentication kind"
    );
    ensure!(
        auth.token.len() <= 65536 && auth.username.len() <= 4096 && auth.password.len() <= 65536,
        "Authentication fields exceed limits"
    );
    if let Some(key) = &auth.api_key {
        ensure!(
            key.name.len() <= 512 && key.value.len() <= 65536,
            "API key fields exceed limits"
        );
    }
    if let Some(jwt) = &auth.jwt {
        ensure!(
            jwt.claims_source.len() <= 65536
                && jwt.key.len() <= 65536
                && jwt.name.len() <= 512
                && jwt.kid.len() <= 512
                && jwt.prefix.len() <= 128
                && jwt.algorithm.len() <= 128,
            "JWT fields exceed limits"
        );
    }
    if let Some(aws) = &auth.aws {
        crate::aws_auth::validate_aws_fields(aws)?;
    }
    if auth.kind == "aws" {
        crate::validate_aws(
            auth.aws.as_ref().context("AWS settings missing")?,
            templates,
        )?;
    }
    if let Some(c) = &auth.asap {
        crate::asap_auth::validate_asap_fields(c)?;
    }
    if auth.kind == "asap" {
        crate::validate_asap(
            auth.asap.as_deref().context("ASAP settings missing")?,
            templates,
        )?;
    }
    if let Some(c) = &auth.edgegrid {
        crate::edgegrid_auth::validate_edgegrid_fields(c)?;
    }
    if auth.kind == "edgegrid" {
        crate::validate_edgegrid(
            auth.edgegrid
                .as_deref()
                .context("EdgeGrid settings missing")?,
            templates,
        )?;
    }
    if let Some(c) = &auth.ntlm {
        crate::ntlm_auth::validate_ntlm_fields(c)?;
    }
    if auth.kind == "ntlm" {
        crate::validate_ntlm(auth, templates)?;
    }
    if let Some(config) = &auth.oauth1 {
        crate::oauth1_auth::validate_oauth1_fields(config)?;
    }
    if auth.kind == "oauth1" {
        crate::validate_oauth1(
            auth.oauth1.as_ref().context("OAuth1 settings missing")?,
            templates,
        )?;
    }
    if let Some(hawk) = &auth.hawk {
        crate::hawk_auth::validate_hawk_fields(hawk)?;
    }
    if auth.kind == "hawk" {
        crate::validate_hawk(
            auth.hawk.as_ref().context("Hawk settings missing")?,
            templates,
        )?;
    }
    let validate_name = |name: &str, location: AuthLocation| -> Result<()> {
        ensure!(
            !name.is_empty() && name.len() <= 512 && !name.chars().any(char::is_control),
            "Invalid authentication field name"
        );
        if location == AuthLocation::Header && !(templates && name.contains("{{")) {
            let header = reqwest::header::HeaderName::from_bytes(name.as_bytes())?;
            ensure!(
                ![
                    "host",
                    "content-length",
                    "transfer-encoding",
                    "connection",
                    "proxy-authorization",
                    "upgrade",
                    "trailer",
                    "te"
                ]
                .contains(&header.as_str()),
                "Authentication cannot override transport-owned headers"
            );
        }
        Ok(())
    };
    if let Some(oauth) = &auth.oauth2 {
        ensure!(
            serde_json::to_vec(oauth)?.len() <= 256 * 1024,
            "OAuth2 settings exceed limit"
        );
        if auth.kind == "oauth2" {
            crate::validate_oauth2(oauth, templates, false)?;
            validate_name(&oauth.name, oauth.location)?;
        }
    }
    if auth.kind == "oauth2" {
        ensure!(auth.oauth2.is_some(), "Configure OAuth2 authentication");
    }
    if auth.kind == "apikey" {
        let key = auth.api_key.as_ref().context("Configure the API key")?;
        validate_name(&key.name, key.location)?;
        ensure!(key.value.len() <= 65536, "API key exceeds limit");
        if !templates && key.location == AuthLocation::Header {
            reqwest::header::HeaderValue::from_str(&key.value)?;
        }
    }
    if auth.kind == "jwt" {
        let jwt = auth.jwt.as_ref().context("Configure JWT signing")?;
        validate_name(&jwt.name, jwt.location)?;
        ensure!(
            jwt.claims_source.len() <= 65536
                && jwt.key.len() <= 65536
                && jwt.kid.len() <= 512
                && jwt.prefix.len() <= 128,
            "JWT fields exceed limits"
        );
        ensure!(
            (1..=86400).contains(&jwt.ttl_seconds),
            "JWT TTL must be1..86400 seconds"
        );
        if !(templates && jwt.algorithm.contains("{{")) {
            jwt.algorithm
                .parse::<jsonwebtoken::Algorithm>()
                .context("Unsupported JWT algorithm")?;
        }
        let claims: serde_json::Value =
            serde_json::from_str(&jwt.claims_source).context("Invalid JWT claims JSON")?;
        ensure!(claims.is_object(), "JWT claims must be an object");
        if !templates {
            ensure!(
                !jwt.key.is_empty(),
                "JWT signing key is missing; supply it again"
            );
        }
    }
    Ok(())
}
pub(crate) fn resolve_jwt_claims(
    source: &str,
    variables: &HashMap<&str, &str>,
    budget: &mut usize,
) -> Result<String> {
    ensure!(source.len() <= 65536, "JWT claims exceed limit");
    let mut value: serde_json::Value = serde_json::from_str(source)?;
    fn visit(
        value: &mut serde_json::Value,
        vars: &HashMap<&str, &str>,
        budget: &mut usize,
        depth: usize,
    ) -> Result<()> {
        ensure!(depth <= 32, "JWT claims nesting exceeds limit");
        match value {
            serde_json::Value::String(text) => {
                *text = crate::interpolation::interpolate_budget(text, vars, budget)?
            }
            serde_json::Value::Array(values) => {
                for v in values {
                    visit(v, vars, budget, depth + 1)?;
                }
            }
            serde_json::Value::Object(values) => {
                for v in values.values_mut() {
                    visit(v, vars, budget, depth + 1)?;
                }
            }
            _ => (),
        }
        Ok(())
    }
    let mut local = (*budget).min(65536);
    let before = local;
    visit(&mut value, variables, &mut local, 0)?;
    *budget -= before - local;
    let output = serde_json::to_string(&value)?;
    ensure!(output.len() <= 65536, "Resolved JWT claims exceed limit");
    Ok(output)
}
pub fn sign_jwt(config: &JwtAuth) -> Result<String> {
    ensure!(
        config.key.len() <= 65536 && config.claims_source.len() <= 65536 && config.kid.len() <= 512,
        "JWT signing inputs exceed limits"
    );
    ensure!(
        (1..=86400).contains(&config.ttl_seconds),
        "JWT TTL must be1..86400 seconds"
    );
    use jsonwebtoken::{Algorithm, EncodingKey, Header};
    let algorithm: Algorithm = config
        .algorithm
        .parse()
        .context("Unsupported JWT algorithm")?;
    ensure!(
        !config.key.is_empty(),
        "JWT signing key is missing; supply it again"
    );
    let key = match algorithm {
        Algorithm::HS256 | Algorithm::HS384 | Algorithm::HS512 => {
            if config.key_base64 {
                EncodingKey::from_base64_secret(&config.key)?
            } else {
                EncodingKey::from_secret(config.key.as_bytes())
            }
        }
        Algorithm::RS256
        | Algorithm::RS384
        | Algorithm::RS512
        | Algorithm::PS256
        | Algorithm::PS384
        | Algorithm::PS512 => EncodingKey::from_rsa_pem(config.key.as_bytes())?,
        Algorithm::ES256 | Algorithm::ES384 => EncodingKey::from_ec_pem(config.key.as_bytes())?,
        Algorithm::EdDSA => EncodingKey::from_ed_pem(config.key.as_bytes())?,
        _ => anyhow::bail!("Unsupported JWT signing algorithm"),
    };
    let mut claims: serde_json::Value = serde_json::from_str(&config.claims_source)?;
    let claims = claims
        .as_object_mut()
        .context("JWT claims must be an object")?;
    if config.add_time_claims {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        claims.entry("iat").or_insert(now.into());
        claims.entry("exp").or_insert(
            now.checked_add(config.ttl_seconds)
                .context("JWT expiry overflow")?
                .into(),
        );
    }
    let mut header = Header::new(algorithm);
    if !config.kid.is_empty() {
        header.kid = Some(config.kid.clone());
    }
    let token = jsonwebtoken::encode(&header, claims, &key)?;
    ensure!(token.len() <= 128 * 1024, "Generated JWT exceeds limit");
    Ok(token)
}
/// Materialize API placement only after variables resolve. Never called by save/export.
pub fn prepare_authentication(request: &crate::RequestSpec) -> Result<crate::RequestSpec> {
    validate_authentication(&request.auth, false)?;
    ensure!(
        request.auth.kind != "oauth2",
        "OAuth2 authentication requires an owner-bound token vault"
    );
    ensure!(
        request.auth.kind != "oauth1"
            || request
                .auth
                .oauth1
                .as_ref()
                .is_none_or(|c| c.token_id.is_none()),
        "OAuth1 token selection requires an owner-bound token vault"
    );
    let mut r = request.clone();
    let (name, value, location) = match r.auth.kind.as_str() {
        "apikey" => {
            let key = r
                .auth
                .api_key
                .as_ref()
                .context("API key settings missing")?;
            (key.name.clone(), key.value.clone(), key.location)
        }
        "jwt" => {
            let jwt = r.auth.jwt.as_ref().context("JWT settings missing")?;
            let token = sign_jwt(jwt)?;
            (
                jwt.name.clone(),
                if jwt.prefix.is_empty() {
                    token
                } else {
                    format!("{} {token}", jwt.prefix)
                },
                jwt.location,
            )
        }
        _ => return Ok(r),
    };
    if location == AuthLocation::Query {
        let mut url = url::Url::parse(&r.url)?;
        let retained = url
            .query_pairs()
            .filter(|(key, _)| key != &name)
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect::<Vec<_>>();
        url.set_query(None);
        if !retained.is_empty() {
            url.query_pairs_mut().extend_pairs(retained);
        }
        r.url = url.into();
    }
    let rows = if location == AuthLocation::Header {
        &mut r.headers
    } else {
        &mut r.query
    };
    rows.retain(|p| {
        if location == AuthLocation::Header {
            !p.key.eq_ignore_ascii_case(&name)
        } else {
            p.key != name
        }
    });
    rows.push(crate::Pair {
        id: "selected-auth".into(),
        key: name,
        value,
        enabled: true,
        secret: Some(true),
        local_value: None,
    });
    r.auth.kind = "none".into();
    Ok(r)
}
