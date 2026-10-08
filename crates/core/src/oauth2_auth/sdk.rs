use super::transport::HttpClient;
use crate::{NetworkPolicy, OAuth2Auth, OAuth2ClientAuth, OAuth2Grant, validate_oauth2};
use anyhow::{Context, Result, ensure};
use oauth2::{
    AuthType, ClientId, ClientSecret, EndpointNotSet, EndpointSet, Scope, TokenUrl,
    basic::{BasicClient, BasicTokenResponse},
};
use std::time::Duration;
fn base_client(config: &OAuth2Auth) -> BasicClient {
    let client = BasicClient::new(ClientId::new(config.client_id.clone())).set_auth_type(
        match config.client_auth {
            OAuth2ClientAuth::Basic => AuthType::BasicAuth,
            OAuth2ClientAuth::Body => AuthType::RequestBody,
        },
    );
    if config.client_secret.is_empty() {
        client
    } else {
        client.set_client_secret(ClientSecret::new(config.client_secret.clone()))
    }
}
type TokenClient =
    BasicClient<EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;
fn token_client(config: &OAuth2Auth) -> Result<TokenClient> {
    Ok(base_client(config).set_token_uri(
        TokenUrl::new(config.token_url.clone()).context("Invalid OAuth2 token URL")?,
    ))
}
fn token_error(
    error: oauth2::RequestTokenError<std::io::Error, oauth2::basic::BasicErrorResponse>,
) -> anyhow::Error {
    match error {
        oauth2::RequestTokenError::ServerResponse(value) => match value.error() {
            oauth2::basic::BasicErrorResponseType::InvalidClient => {
                anyhow::anyhow!("OAuth2 provider rejected the client credentials")
            }
            oauth2::basic::BasicErrorResponseType::InvalidGrant => {
                anyhow::anyhow!("OAuth2 grant is invalid or expired")
            }
            _ => anyhow::anyhow!("OAuth2 provider rejected the grant request"),
        },
        _ => anyhow::anyhow!("OAuth2 token response could not be obtained or parsed"),
    }
}
pub async fn oauth2_acquire(
    config: &OAuth2Auth,
    policy: NetworkPolicy,
    verify_tls: bool,
) -> Result<BasicTokenResponse> {
    let active = config.grant_configuration();
    let config = &active;
    validate_oauth2(config, false, true)?;
    let client = token_client(config)?;
    let http = HttpClient {
        config: config.clone(),
        policy,
        verify_tls,
    };
    match config.grant {
        OAuth2Grant::ClientCredentials => {
            let mut request = client.exchange_client_credentials();
            for scope in &config.scopes {
                request = request.add_scope(Scope::new(scope.clone()));
            }
            for row in config.token_params.iter().filter(|p| p.enabled) {
                request = request.add_extra_param(&row.key, &row.value);
            }
            request.request_async(&http).await.map_err(token_error)
        }
        OAuth2Grant::Password => {
            let username = oauth2::ResourceOwnerUsername::new(config.username.clone());
            let password = oauth2::ResourceOwnerPassword::new(config.password.clone());
            let mut request = client.exchange_password(&username, &password);
            for scope in &config.scopes {
                request = request.add_scope(Scope::new(scope.clone()));
            }
            for row in config.token_params.iter().filter(|p| p.enabled) {
                request = request.add_extra_param(&row.key, &row.value);
            }
            request.request_async(&http).await.map_err(token_error)
        }
        _ => anyhow::bail!("This OAuth2 grant requires an authorization flow"),
    }
}
pub async fn oauth2_refresh(
    config: &OAuth2Auth,
    refresh: &str,
    policy: NetworkPolicy,
    verify_tls: bool,
) -> Result<BasicTokenResponse> {
    let active = config.grant_configuration();
    let config = &active;
    validate_oauth2(config, false, false)?;
    ensure!(
        !config.token_url.is_empty() && refresh.len() <= 65536 && !refresh.is_empty(),
        "OAuth2 refresh configuration is missing or exceeds limits"
    );
    let client = token_client(config)?;
    let refresh = oauth2::RefreshToken::new(refresh.to_owned());
    let mut request = client.exchange_refresh_token(&refresh);
    for scope in &config.scopes {
        request = request.add_scope(Scope::new(scope.clone()));
    }
    for row in config.token_params.iter().filter(|p| p.enabled) {
        request = request.add_extra_param(&row.key, &row.value);
    }
    request
        .request_async(&HttpClient {
            config: config.clone(),
            policy,
            verify_tls,
        })
        .await
        .map_err(token_error)
}
/// An expiring broker owns this secret tuple; it is never serialized to callers.
pub struct OAuth2Authorization {
    pub url: String,
    pub state: oauth2::CsrfToken,
    pub verifier: Option<oauth2::PkceCodeVerifier>,
}
pub fn oauth2_authorize(config: &OAuth2Auth) -> Result<OAuth2Authorization> {
    let active = config.grant_configuration();
    let config = &active;
    validate_oauth2(config, false, true)?;
    ensure!(
        matches!(
            config.grant,
            OAuth2Grant::AuthorizationCode | OAuth2Grant::Implicit
        ),
        "OAuth2 grant does not use browser authorization"
    );
    let client = base_client(config)
        .set_auth_uri(oauth2::AuthUrl::new(config.authorization_url.clone())?)
        .set_redirect_uri(oauth2::RedirectUrl::new(config.redirect_url.clone())?);
    let mut request = client.authorize_url(oauth2::CsrfToken::new_random);
    if config.grant == OAuth2Grant::Implicit {
        request = request.use_implicit_flow();
    }
    let verifier = if config.pkce && config.grant == OAuth2Grant::AuthorizationCode {
        let (challenge, verifier) = oauth2::PkceCodeChallenge::new_random_sha256();
        request = request.set_pkce_challenge(challenge);
        Some(verifier)
    } else {
        None
    };
    for scope in &config.scopes {
        request = request.add_scope(Scope::new(scope.clone()));
    }
    for row in config.authorization_params.iter().filter(|p| p.enabled) {
        request = request.add_extra_param(&row.key, &row.value);
    }
    let (url, state) = request.url();
    Ok(OAuth2Authorization {
        url: url.into(),
        state,
        verifier,
    })
}
pub async fn oauth2_exchange_code(
    config: &OAuth2Auth,
    code: &str,
    verifier: Option<oauth2::PkceCodeVerifier>,
    policy: NetworkPolicy,
    verify_tls: bool,
) -> Result<BasicTokenResponse> {
    let active = config.grant_configuration();
    let config = &active;
    validate_oauth2(config, false, true)?;
    ensure!(
        config.grant == OAuth2Grant::AuthorizationCode && !code.is_empty() && code.len() <= 65536,
        "Invalid OAuth2 authorization code"
    );
    ensure!(
        !config.pkce || verifier.is_some(),
        "OAuth2 PKCE verifier is missing"
    );
    let client = token_client(config)?
        .set_redirect_uri(oauth2::RedirectUrl::new(config.redirect_url.clone())?);
    let mut request = client.exchange_code(oauth2::AuthorizationCode::new(code.into()));
    if let Some(verifier) = verifier {
        request = request.set_pkce_verifier(verifier);
    }
    for row in config.token_params.iter().filter(|p| p.enabled) {
        request = request.add_extra_param(&row.key, &row.value);
    }
    request
        .request_async(&HttpClient {
            config: config.clone(),
            policy,
            verify_tls,
        })
        .await
        .map_err(token_error)
}

pub async fn oauth2_device_start(
    config: &OAuth2Auth,
    policy: NetworkPolicy,
    verify_tls: bool,
) -> Result<oauth2::StandardDeviceAuthorizationResponse> {
    let active = config.grant_configuration();
    let config = &active;
    validate_oauth2(config, false, true)?;
    ensure!(
        config.grant == OAuth2Grant::DeviceCode,
        "OAuth2 grant is not device authorization"
    );
    let client = base_client(config).set_device_authorization_url(
        oauth2::DeviceAuthorizationUrl::new(config.device_url.clone())?,
    );
    let mut request = client.exchange_device_code();
    for scope in &config.scopes {
        request = request.add_scope(Scope::new(scope.clone()));
    }
    for row in config.token_params.iter().filter(|p| p.enabled) {
        request = request.add_extra_param(&row.key, &row.value);
    }
    let response: oauth2::StandardDeviceAuthorizationResponse = request
        .request_async(&HttpClient {
            config: config.clone(),
            policy,
            verify_tls,
        })
        .await
        .map_err(token_error)?;
    ensure!(
        response.device_code().secret().len() <= 65536
            && response.user_code().secret().len() <= 4096,
        "OAuth2 device response exceeds limits"
    );
    crate::valid_url(response.verification_uri().as_str())?;
    if let Some(url) = response.verification_uri_complete() {
        crate::valid_url(url.secret())?;
    }
    Ok(response)
}
pub async fn oauth2_device_poll(
    config: &OAuth2Auth,
    device: &oauth2::StandardDeviceAuthorizationResponse,
    policy: NetworkPolicy,
    verify_tls: bool,
    timeout: Duration,
) -> Result<BasicTokenResponse> {
    let active = config.grant_configuration();
    let config = &active;
    validate_oauth2(config, false, true)?;
    ensure!(
        config.grant == OAuth2Grant::DeviceCode,
        "OAuth2 grant is not device authorization"
    );
    let client = token_client(config)?;
    tokio::time::timeout(
        timeout,
        client.exchange_device_access_token(device).request_async(
            &HttpClient {
                config: config.clone(),
                policy,
                verify_tls,
            },
            tokio::time::sleep,
            Some(timeout),
        ),
    )
    .await
    .context("OAuth2 device authorization timed out")?
    .map_err(|_| anyhow::anyhow!("OAuth2 device authorization failed or expired"))
}
pub async fn oauth2_introspect(
    config: &OAuth2Auth,
    access: &str,
    policy: NetworkPolicy,
    verify_tls: bool,
) -> Result<oauth2::basic::BasicTokenIntrospectionResponse> {
    let mut active = config.grant_configuration();
    active.authorization_url.clear();
    active.redirect_url.clear();
    active.device_url.clear();
    active.token_url.clear();
    active.authorization_params.clear();
    active.token_params.clear();
    active.scopes.clear();
    active.client_secret = config.client_secret.clone();
    active.client_auth = config.client_auth;
    active.token_headers = config
        .token_headers
        .iter()
        .filter(|row| row.enabled)
        .cloned()
        .collect();
    active.introspection_url = config.introspection_url.clone();
    let config = &active;
    validate_oauth2(config, false, false)?;
    ensure!(
        !config.introspection_url.is_empty() && !access.is_empty() && access.len() <= 65536,
        "OAuth2 introspection configuration/token missing or exceeds limits"
    );
    let client = base_client(config).set_introspection_url(oauth2::IntrospectionUrl::new(
        config.introspection_url.clone(),
    )?);
    let token = oauth2::AccessToken::new(access.into());
    client
        .introspect(&token)
        .request_async(&HttpClient {
            config: config.clone(),
            policy,
            verify_tls,
        })
        .await
        .map_err(token_error)
}
pub async fn oauth2_revoke(
    config: &OAuth2Auth,
    token: &str,
    refresh: bool,
    policy: NetworkPolicy,
    verify_tls: bool,
) -> Result<()> {
    let mut active = config.grant_configuration();
    active.authorization_url.clear();
    active.redirect_url.clear();
    active.device_url.clear();
    active.token_url.clear();
    active.authorization_params.clear();
    active.token_params.clear();
    active.scopes.clear();
    active.client_secret = config.client_secret.clone();
    active.client_auth = config.client_auth;
    active.token_headers = config
        .token_headers
        .iter()
        .filter(|row| row.enabled)
        .cloned()
        .collect();
    active.revocation_url = config.revocation_url.clone();
    let config = &active;
    validate_oauth2(config, false, false)?;
    ensure!(
        !config.revocation_url.is_empty() && !token.is_empty() && token.len() <= 65536,
        "OAuth2 revocation configuration/token missing or exceeds limits"
    );
    let client = base_client(config)
        .set_revocation_url(oauth2::RevocationUrl::new(config.revocation_url.clone())?);
    let token = if refresh {
        oauth2::StandardRevocableToken::RefreshToken(oauth2::RefreshToken::new(token.into()))
    } else {
        oauth2::StandardRevocableToken::AccessToken(oauth2::AccessToken::new(token.into()))
    };
    client
        .revoke_token(token)
        .context("OAuth2 revocation requires a secure endpoint")?
        .request_async(&HttpClient {
            config: config.clone(),
            policy,
            verify_tls,
        })
        .await
        .map_err(|_| anyhow::anyhow!("OAuth2 provider could not revoke the credential"))
}
