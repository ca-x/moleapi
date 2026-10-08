//! Signed OAuth1 exchanges; SDK signing and mature form/HTTP parsers own the wire format.
use super::{OAuth1Auth, OAuth1Location, sign_oauth1_request};
use crate::{NetworkPolicy, Pair, checked_client, valid_url};
use anyhow::{Context, Result, ensure};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OAuth1Grant {
    pub request_token_url: String,
    pub authorization_url: String,
    pub access_token_url: String,
    pub callback_url: String,
    pub request_params: Vec<Pair>,
    pub access_params: Vec<Pair>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OAuth1Credentials {
    pub token: String,
    pub secret: String,
}
impl OAuth1Credentials {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.token.is_empty()
                && !self.secret.is_empty()
                && self.token.len() <= 4096
                && self.secret.len() <= 65536
                && !self.token.chars().any(char::is_control)
                && !self.secret.chars().any(char::is_control),
            "Invalid OAuth1 credentials"
        );
        Ok(())
    }
}
pub fn validate_oauth1_grant_fields(g: &OAuth1Grant) -> Result<()> {
    ensure!(
        [
            &g.request_token_url,
            &g.authorization_url,
            &g.access_token_url,
            &g.callback_url
        ]
        .iter()
        .all(|s| s.len() <= 8192),
        "OAuth1 grant URL exceeds limit"
    );
    for params in [&g.request_params, &g.access_params] {
        ensure!(
            params.len() <= 128 && serde_json::to_vec(params)?.len() <= 128 * 1024,
            "OAuth1 grant parameters exceed limits"
        );
        for row in params {
            ensure!(
                row.key.len() <= 1024
                    && row.value.len() <= 8192
                    && row.local_value.as_ref().is_none_or(|v| v.len() <= 8192),
                "OAuth1 grant parameter exceeds limit"
            );
        }
    }
    Ok(())
}
#[derive(Deserialize)]
struct Reply {
    oauth_token: String,
    oauth_token_secret: String,
    oauth_callback_confirmed: Option<String>,
}
async fn exchange(
    c: &OAuth1Auth,
    endpoint: &str,
    params: &[Pair],
    policy: NetworkPolicy,
    verify_tls: bool,
    request_token: bool,
) -> Result<OAuth1Credentials> {
    tokio::time::timeout(Duration::from_secs(30), async {
        let url = valid_url(endpoint)?;
        let client = checked_client(&url, policy, verify_tls).await?;
        let enabled: Vec<_> = params.iter().filter(|row| row.enabled).collect();
        ensure!(
            enabled
                .iter()
                .all(|r| !r.key.starts_with("oauth_") && !r.key.is_empty()),
            "OAuth1 grant owns oauth_ parameter names"
        );
        let body = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(
                enabled
                    .into_iter()
                    .map(|r| (r.key.as_str(), r.value.as_str())),
            )
            .finish();
        let mut request = client
            .post(url)
            .header("content-type", "application/x-www-form-urlencoded")
            .body(body)
            .build()?;
        let mut config = c.clone();
        config.location = OAuth1Location::Header;
        config.token_id = None;
        config.grant = None;
        config.nonce.clear();
        config.timestamp.clear();
        config.realm.clear();
        config.include_body_hash = false;
        config.include_empty_params = true;
        sign_oauth1_request(&config, &mut request, SystemTime::now())?;
        let response = client
            .execute(request)
            .await
            .context("OAuth1 provider request failed")?;
        ensure!(
            response.status().is_success(),
            "OAuth1 provider rejected token exchange (HTTP {})",
            response.status().as_u16()
        );
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Reading OAuth1 provider response failed")?;
            ensure!(
                bytes.len() + chunk.len() <= 65536,
                "OAuth1 provider response exceeds 64 KiB"
            );
            bytes.extend_from_slice(&chunk);
        }
        let text = std::str::from_utf8(&bytes).context("OAuth1 provider response is not UTF-8")?;
        // percent_decode also rejects invalid decoded UTF8 before the form parser's lossy path.
        percent_encoding::percent_decode_str(text)
            .decode_utf8()
            .context("OAuth1 provider response decodes to invalid UTF-8")?;
        let reply: Reply = serde_urlencoded::from_str(text)
            .map_err(|_| anyhow::anyhow!("Invalid OAuth1 token response"))?;
        ensure!(
            !request_token || reply.oauth_callback_confirmed.as_deref() == Some("true"),
            "OAuth1 provider did not confirm callback"
        );
        let credentials = OAuth1Credentials {
            token: reply.oauth_token,
            secret: reply.oauth_token_secret,
        };
        credentials.validate()?;
        Ok(credentials)
    })
    .await
    .context("OAuth1 token exchange timed out")?
}
pub async fn oauth1_request_token(
    c: &OAuth1Auth,
    g: &OAuth1Grant,
    callback: &str,
    policy: NetworkPolicy,
    verify_tls: bool,
) -> Result<OAuth1Credentials> {
    validate_oauth1_grant_fields(g)?;
    ensure!(
        !callback.is_empty() && callback.len() <= 8192,
        "Invalid OAuth1 callback URI"
    );
    let mut config = c.clone();
    config.token.clear();
    config.token_secret.clear();
    config.verifier.clear();
    config.callback = callback.into();
    exchange(
        &config,
        &g.request_token_url,
        &g.request_params,
        policy,
        verify_tls,
        true,
    )
    .await
}
pub async fn oauth1_access_token(
    c: &OAuth1Auth,
    g: &OAuth1Grant,
    temporary: &OAuth1Credentials,
    verifier: &str,
    policy: NetworkPolicy,
    verify_tls: bool,
) -> Result<OAuth1Credentials> {
    validate_oauth1_grant_fields(g)?;
    temporary.validate()?;
    ensure!(
        !verifier.is_empty() && verifier.len() <= 4096 && !verifier.chars().any(char::is_control),
        "Invalid OAuth1 verifier"
    );
    let mut config = c.clone();
    config.token = temporary.token.clone();
    config.token_secret = temporary.secret.clone();
    config.verifier = verifier.into();
    config.callback.clear();
    exchange(
        &config,
        &g.access_token_url,
        &g.access_params,
        policy,
        verify_tls,
        false,
    )
    .await
}

/// Credential source values used by the shared privacy pipeline, including grant extras.
pub fn oauth1_private_sources(c: &OAuth1Auth) -> Vec<String> {
    let mut values = vec![
        c.consumer_key.clone(),
        c.consumer_secret.clone(),
        c.token.clone(),
        c.token_secret.clone(),
        c.private_key.clone(),
        c.verifier.clone(),
    ];
    if let Some(id) = &c.token_id {
        values.push(id.clone());
    }
    if let Some(g) = &c.grant {
        for row in g
            .request_params
            .iter()
            .chain(&g.access_params)
            .filter(|row| {
                row.secret == Some(true)
                    || crate::sensitive_query_key(&row.key)
                    || row.key.contains("{{")
            })
        {
            values.push(row.value.clone());
            if let Some(local) = &row.local_value {
                values.push(local.clone());
            }
        }
        for raw in [
            &g.request_token_url,
            &g.authorization_url,
            &g.access_token_url,
            &g.callback_url,
        ] {
            let parsed = url::Url::parse(raw).ok().or_else(|| {
                raw.split_once('?').and_then(|(_, query)| {
                    url::Url::parse(&format!("https://moleapi.invalid/?{query}")).ok()
                })
            });
            if let Some(url) = parsed {
                if !url.username().is_empty() {
                    values.push(url.username().into());
                }
                if let Some(password) = url.password() {
                    values.push(password.into());
                }
                for (_, value) in url
                    .query_pairs()
                    .filter(|(key, _)| crate::sensitive_query_key(key))
                {
                    values.push(value.into_owned());
                }
            }
        }
    }
    values
}
