//! Thin OAuth SDK adapter; no protocol or cryptographic parser lives here.
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use oauth1_request::{
    Builder, Credentials,
    request::AssertSorted,
    signature_method::{HMAC_SHA1, SignatureMethod},
};
use rsa::{
    RsaPrivateKey, pkcs1::DecodeRsaPrivateKey, pkcs8::DecodePrivateKey, traits::PublicKeyParts,
};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::{
    num::NonZeroU64,
    time::{SystemTime, UNIX_EPOCH},
};
mod methods;
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OAuth1Location {
    #[default]
    Header,
    Query,
    Body,
    Automatic,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OAuth1Auth {
    pub consumer_key: String,
    pub consumer_secret: String,
    pub token: String,
    pub token_secret: String,
    pub private_key: String,
    pub algorithm: String,
    pub location: OAuth1Location,
    pub realm: String,
    pub nonce: String,
    pub timestamp: String,
    pub callback: String,
    pub verifier: String,
    pub include_version: bool,
    pub include_body_hash: bool,
    pub include_empty_params: bool,
}
impl Default for OAuth1Auth {
    fn default() -> Self {
        Self {
            consumer_key: String::new(),
            consumer_secret: String::new(),
            token: String::new(),
            token_secret: String::new(),
            private_key: String::new(),
            algorithm: "HMAC-SHA1".into(),
            location: OAuth1Location::Header,
            realm: String::new(),
            nonce: String::new(),
            timestamp: String::new(),
            callback: String::new(),
            verifier: String::new(),
            include_version: true,
            include_body_hash: false,
            include_empty_params: true,
        }
    }
}
pub(crate) fn validate_oauth1_fields(c: &OAuth1Auth) -> Result<()> {
    ensure!(
        [
            &c.consumer_key,
            &c.token,
            &c.realm,
            &c.nonce,
            &c.callback,
            &c.verifier
        ]
        .iter()
        .all(|s| s.len() <= 4096)
            && [&c.consumer_secret, &c.token_secret, &c.private_key]
                .iter()
                .all(|s| s.len() <= 65536)
            && c.algorithm.len() <= 128
            && c.timestamp.len() <= 128,
        "OAuth1 fields exceed limits"
    );
    Ok(())
}
pub fn validate_oauth1(c: &OAuth1Auth, templates: bool) -> Result<()> {
    validate_oauth1_fields(c)?;
    if !(templates && c.algorithm.contains("{{")) {
        ensure!(
            [
                "HMAC-SHA1",
                "HMAC-SHA256",
                "HMAC-SHA512",
                "RSA-SHA1",
                "RSA-SHA256",
                "RSA-SHA512",
                "PLAINTEXT"
            ]
            .contains(&c.algorithm.as_str()),
            "Unsupported OAuth1 signature method"
        );
    }
    if !templates {
        ensure!(
            !c.consumer_key.is_empty(),
            "OAuth1 consumer key is missing; supply it again"
        );
        if c.algorithm.starts_with("RSA-") {
            ensure!(
                !c.private_key.is_empty(),
                "OAuth1 private key is missing; supply it again"
            );
        } else {
            ensure!(
                !c.consumer_secret.is_empty(),
                "OAuth1 consumer secret is missing; supply it again"
            );
        }
        if !c.timestamp.is_empty() {
            timestamp(&c.timestamp)?;
        }
    }
    Ok(())
}
fn timestamp(s: &str) -> Result<NonZeroU64> {
    s.parse::<NonZeroU64>()
        .context("OAuth1 timestamp must be a positive integer")
}
pub(crate) fn encode(s: &str) -> String {
    const RESERVED: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'.')
        .remove(b'_')
        .remove(b'~');
    percent_encoding::utf8_percent_encode(s, RESERVED).to_string()
}
fn rsa_key(pem: &str) -> Result<RsaPrivateKey> {
    ensure!(pem.len() <= 16384, "OAuth1 RSA PEM exceeds 16 KiB");
    let key = RsaPrivateKey::from_pkcs8_pem(pem)
        .or_else(|_| RsaPrivateKey::from_pkcs1_pem(pem))
        .map_err(|_| anyhow::anyhow!("Invalid OAuth1 RSA private PEM key"))?;
    ensure!(
        (2048..=4096).contains(&key.n().bits()),
        "OAuth1 RSA key must be 2048..4096 bits"
    );
    key.validate()
        .map_err(|_| anyhow::anyhow!("Invalid OAuth1 RSA private key"))?;
    Ok(key)
}
fn output<M: SignatureMethod + Clone>(
    c: &OAuth1Auth,
    request: &reqwest::Request,
    params: &[(String, String)],
    nonce: &str,
    time: NonZeroU64,
    method: M,
    captured: &std::sync::Arc<std::sync::Mutex<Vec<String>>>,
) -> String {
    let mut base = request.url().clone();
    base.set_query(None);
    base.set_fragment(None);
    let mut builder: Builder<'_, _, &str, &str> = Builder::new(
        Credentials::new(c.consumer_key.as_str(), c.consumer_secret.as_str()),
        methods::Capture {
            method,
            values: captured.clone(),
        },
    );
    if !c.token.is_empty() {
        builder.token(Credentials::new(c.token.as_str(), c.token_secret.as_str()));
    }
    builder
        .nonce(nonce)
        .timestamp(time)
        .version(c.include_version);
    if !c.callback.is_empty() {
        builder.callback(c.callback.as_str());
    }
    if !c.verifier.is_empty() {
        builder.verifier(c.verifier.as_str());
    }
    let parameters = AssertSorted::new(
        params
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str())),
    );
    if c.location == OAuth1Location::Header {
        builder.authorize(request.method().as_str(), base.as_str(), &parameters)
    } else {
        builder.to_form(request.method().as_str(), base.as_str(), &parameters)
    }
}
pub fn sign_oauth1_request(
    c: &OAuth1Auth,
    request: &mut reqwest::Request,
    now: SystemTime,
) -> Result<Vec<String>> {
    validate_oauth1(c, false)?;
    ensure!(
        !request.headers().contains_key("authorization"),
        "OAuth1 signing owns the Authorization header"
    );
    let mime = request
        .headers()
        .get("content-type")
        .map(|v| v.to_str())
        .transpose()?
        .map(str::parse::<mime::Mime>)
        .transpose()
        .context("Invalid OAuth1 content type")?;
    let form = mime.as_ref().is_some_and(|m| {
        m.essence_str()
            .eq_ignore_ascii_case("application/x-www-form-urlencoded")
    });
    let mut effective = c.clone();
    if effective.location == OAuth1Location::Automatic {
        effective.location = if form && matches!(request.method().as_str(), "POST" | "PUT") {
            OAuth1Location::Body
        } else {
            OAuth1Location::Query
        };
    }
    let c = &effective;
    ensure!(
        c.location != OAuth1Location::Body || form,
        "OAuth1 body placement requires application/x-www-form-urlencoded"
    );
    let body = request
        .body()
        .map(|b| {
            b.as_bytes()
                .context("OAuth1 signing requires materialized body bytes")
        })
        .transpose()?
        .unwrap_or(&[]);
    if let Some(query) = request.url().query() {
        percent_encoding::percent_decode_str(query)
            .decode_utf8()
            .context("OAuth1 query must decode as UTF-8")?;
    }
    if form {
        let raw = std::str::from_utf8(body).context("OAuth1 form must be UTF-8")?;
        percent_encoding::percent_decode_str(raw)
            .decode_utf8()
            .context("OAuth1 form must decode as UTF-8")?;
    }
    let mut parameters: Vec<(String, String)> = request
        .url()
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    if form {
        parameters.extend(
            url::form_urlencoded::parse(body).map(|(k, v)| (k.into_owned(), v.into_owned())),
        );
    }
    ensure!(
        parameters.len() <= 2048,
        "OAuth1 signing parameter limit exceeded"
    );
    ensure!(
        parameters.iter().all(|(k, _)| !k.starts_with("oauth_")),
        "OAuth1 signing owns oauth_ parameters"
    );
    if !c.include_empty_params {
        parameters.retain(|(_, v)| !v.is_empty());
    }
    let body_hash = if c.include_body_hash && !form {
        let bytes = if c.algorithm.ends_with("SHA256") {
            sha2::Sha256::digest(body).to_vec()
        } else if c.algorithm.ends_with("SHA512") {
            sha2::Sha512::digest(body).to_vec()
        } else {
            sha1::Sha1::digest(body).to_vec()
        };
        Some(STANDARD.encode(bytes))
    } else {
        None
    };
    if let Some(hash) = &body_hash {
        parameters.push(("oauth_body_hash".into(), hash.clone()));
    }
    ensure!(
        parameters
            .iter()
            .map(|(k, v)| encode(k).len() + encode(v).len())
            .sum::<usize>()
            <= 256 * 1024,
        "OAuth1 signing parameters exceed 256 KiB"
    );
    // SDK keys are already part of the escaped base-string fragment; values are raw.
    parameters.sort_by_cached_key(|(k, v)| (encode(k), encode(v)));
    for (k, _) in &mut parameters {
        *k = encode(&encode(k));
    }
    let nonce = if c.nonce.is_empty() {
        uuid::Uuid::new_v4().simple().to_string()
    } else {
        c.nonce.clone()
    };
    let time = if c.timestamp.is_empty() {
        NonZeroU64::new(now.duration_since(UNIX_EPOCH)?.as_secs())
            .context("Invalid OAuth1 clock")?
    } else {
        timestamp(&c.timestamp)?
    };
    let captured = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let result = match c.algorithm.as_str() {
        "HMAC-SHA1" => output(c, request, &parameters, &nonce, time, HMAC_SHA1, &captured),
        "PLAINTEXT" => output(
            c,
            request,
            &parameters,
            &nonce,
            time,
            methods::Plaintext,
            &captured,
        ),
        "RSA-SHA1" => output(
            c,
            request,
            &parameters,
            &nonce,
            time,
            oauth1_request::signature_method::Rsa09Sha1::new(rsa_key(&c.private_key)?),
            &captured,
        ),
        _ => {
            let method = methods::Method::new(
                &c.algorithm,
                if c.algorithm.starts_with("RSA-") {
                    Some(rsa_key(&c.private_key)?)
                } else {
                    None
                },
            );
            let failed = method.failed.clone();
            let result = output(c, request, &parameters, &nonce, time, method, &captured);
            ensure!(
                !failed.load(std::sync::atomic::Ordering::Relaxed),
                "OAuth1 signing failed"
            );
            result
        }
    };
    ensure!(result.len() <= 65536, "OAuth1 authorization exceeds limit");
    let mut private = vec![
        result.clone(),
        c.consumer_key.clone(),
        c.consumer_secret.clone(),
        c.token.clone(),
        c.token_secret.clone(),
        c.verifier.clone(),
    ];
    private.extend(captured.lock().unwrap().iter().cloned());
    for value in [
        &c.consumer_key,
        &c.token,
        &c.consumer_secret,
        &c.token_secret,
        &c.verifier,
    ] {
        private.push(encode(value));
    }
    match c.location {
        OAuth1Location::Header => {
            let mut header = result;
            if !c.realm.is_empty() {
                header = header.replacen(
                    "OAuth ",
                    &format!("OAuth realm=\"{}\",", encode(&c.realm)),
                    1,
                );
            }
            if let Some(hash) = body_hash {
                header.push_str(&format!(",oauth_body_hash=\"{}\"", encode(&hash)));
            }
            ensure!(header.len() <= 65536, "OAuth1 authorization exceeds limit");
            private.push(header.clone());
            request
                .headers_mut()
                .insert("authorization", header.parse()?);
        }
        OAuth1Location::Query | OAuth1Location::Body => {
            let mut values: Vec<_> = url::form_urlencoded::parse(result.as_bytes())
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect();
            if let Some(hash) = body_hash {
                values.push(("oauth_body_hash".into(), hash));
            }
            private.extend(values.iter().map(|(_, v)| v.clone()));
            if c.location == OAuth1Location::Query {
                request.url_mut().query_pairs_mut().extend_pairs(values);
                ensure!(
                    request.url().as_str().len() <= 65536,
                    "OAuth1 signed URL exceeds 64 KiB"
                );
            } else {
                let encoded = url::form_urlencoded::Serializer::new(String::new())
                    .extend_pairs(values)
                    .finish();
                let mut actual = body.to_vec();
                if !actual.is_empty() {
                    actual.push(b'&');
                }
                actual.extend_from_slice(encoded.as_bytes());
                ensure!(
                    actual.len() <= crate::MAX_BODY,
                    "OAuth1 signed form exceeds 5 MiB"
                );
                *request.body_mut() = Some(actual.into());
            }
        }
        OAuth1Location::Automatic => unreachable!("projected placement"),
    }
    Ok(private)
}
