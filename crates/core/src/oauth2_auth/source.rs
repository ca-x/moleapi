//! OAuth2 source and SDK adapter. Tokens are acquired explicitly by a workspace vault.
use crate::{AuthLocation, Pair};
use anyhow::{Context, Result, ensure};

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OAuth2Grant {
    #[default]
    AuthorizationCode,
    Implicit,
    ClientCredentials,
    Password,
    DeviceCode,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OAuth2ClientAuth {
    #[default]
    Basic,
    Body,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OAuth2Auth {
    pub grant: OAuth2Grant,
    pub authorization_url: String,
    pub token_url: String,
    pub device_url: String,
    pub revocation_url: String,
    pub introspection_url: String,
    pub redirect_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub username: String,
    pub password: String,
    pub scopes: Vec<String>,
    pub pkce: bool,
    pub client_auth: OAuth2ClientAuth,
    pub authorization_params: Vec<Pair>,
    pub token_params: Vec<Pair>,
    pub token_headers: Vec<Pair>,
    pub token_id: Option<String>,
    pub auto_refresh: bool,
    pub location: AuthLocation,
    pub name: String,
    pub prefix: String,
}
impl Default for OAuth2Auth {
    fn default() -> Self {
        Self {
            grant: OAuth2Grant::AuthorizationCode,
            authorization_url: String::new(),
            token_url: String::new(),
            device_url: String::new(),
            revocation_url: String::new(),
            introspection_url: String::new(),
            redirect_url: String::new(),
            client_id: String::new(),
            client_secret: String::new(),
            username: String::new(),
            password: String::new(),
            scopes: vec![],
            pkce: true,
            client_auth: OAuth2ClientAuth::Basic,
            authorization_params: vec![],
            token_params: vec![],
            token_headers: vec![],
            token_id: None,
            auto_refresh: true,
            location: AuthLocation::Header,
            name: "Authorization".into(),
            prefix: "Bearer".into(),
        }
    }
}
const AUTH_FIELDS: &[&str] = &[
    "response_type",
    "client_id",
    "redirect_uri",
    "scope",
    "state",
    "code_challenge",
    "code_challenge_method",
];
const TOKEN_FIELDS: &[&str] = &[
    "grant_type",
    "client_id",
    "client_secret",
    "scope",
    "code",
    "code_verifier",
    "redirect_uri",
    "refresh_token",
    "device_code",
    "username",
    "password",
    "token",
    "token_type_hint",
];
pub fn validate_oauth2(config: &OAuth2Auth, templates: bool, acquisition: bool) -> Result<()> {
    ensure!(
        config.client_id.len() <= 4096
            && config.client_secret.len() <= 65536
            && config.username.len() <= 4096
            && config.password.len() <= 65536,
        "OAuth2 credential fields exceed limits"
    );
    ensure!(
        config.name.len() <= 512
            && config.prefix.len() <= 128
            && config.token_id.as_ref().is_none_or(|s| s.len() <= 128),
        "OAuth2 token placement exceeds limits"
    );
    ensure!(
        config.scopes.len() <= 64 && config.scopes.iter().all(|s| s.len() <= 1024),
        "OAuth2 scopes exceed limits"
    );
    for (name, url, fields) in [
        ("authorization", &config.authorization_url, AUTH_FIELDS),
        ("token", &config.token_url, TOKEN_FIELDS),
        ("device", &config.device_url, TOKEN_FIELDS),
        ("revocation", &config.revocation_url, TOKEN_FIELDS),
        ("introspection", &config.introspection_url, TOKEN_FIELDS),
    ] {
        ensure!(url.len() <= 8192, "OAuth2 endpoint exceeds limit");
        if !url.is_empty() && !(templates && url.contains("{{")) {
            let url =
                crate::valid_url(url).with_context(|| format!("Invalid OAuth2 {name} endpoint"))?;
            ensure!(
                url.fragment().is_none(),
                "OAuth2 endpoint cannot contain a fragment"
            );
            ensure!(
                !url.query_pairs()
                    .any(|(key, _)| fields.contains(&key.as_ref())),
                "OAuth2 endpoint query contains an SDK-owned parameter"
            );
        }
    }
    ensure!(
        config.redirect_url.len() <= 8192,
        "OAuth2 redirect exceeds limit"
    );
    if !config.redirect_url.is_empty() && !(templates && config.redirect_url.contains("{{")) {
        let url = url::Url::parse(&config.redirect_url).context("Invalid OAuth2 redirect URI")?;
        ensure!(
            !url.scheme().is_empty()
                && url.fragment().is_none()
                && url.username().is_empty()
                && url.password().is_none(),
            "Invalid OAuth2 redirect URI"
        );
    }
    for (rows, owned, headers) in [
        (&config.authorization_params, AUTH_FIELDS, false),
        (&config.token_params, TOKEN_FIELDS, false),
        (&config.token_headers, &[][..], true),
    ] {
        ensure!(rows.len() <= 64, "OAuth2 parameter limit exceeded");
        let mut seen = HashSet::new();
        for row in rows {
            ensure!(
                row.key.len() <= 512
                    && row.value.len() <= 8192
                    && row.local_value.as_ref().is_none_or(|v| v.len() <= 8192),
                "OAuth2 parameter exceeds limits"
            );
            if !row.enabled {
                continue;
            }
            ensure!(
                !row.key.is_empty() && !row.key.chars().any(char::is_control),
                "Invalid OAuth2 parameter name"
            );
            if templates && row.key.contains("{{") {
                continue;
            }
            let key = if headers {
                row.key.to_ascii_lowercase()
            } else {
                row.key.clone()
            };
            ensure!(
                seen.insert(key.clone()) && !owned.contains(&key.as_str()),
                "Duplicate or SDK-owned OAuth2 parameter"
            );
            if headers {
                reqwest::header::HeaderName::from_bytes(key.as_bytes())?;
                ensure!(
                    ![
                        "authorization",
                        "host",
                        "content-length",
                        "transfer-encoding",
                        "connection",
                        "proxy-authorization",
                        "cookie",
                        "set-cookie",
                        "upgrade"
                    ]
                    .contains(&key.as_str()),
                    "OAuth2 header is transport-owned"
                );
                if !templates {
                    reqwest::header::HeaderValue::from_str(&row.value)?;
                }
            }
        }
    }
    if acquisition {
        ensure!(!config.client_id.is_empty(), "OAuth2 client ID is missing");
        if config.grant != OAuth2Grant::Implicit {
            ensure!(!config.token_url.is_empty(), "OAuth2 token URL is missing");
        }
        if matches!(
            config.grant,
            OAuth2Grant::AuthorizationCode | OAuth2Grant::Implicit
        ) {
            ensure!(
                !config.authorization_url.is_empty() && !config.redirect_url.is_empty(),
                "OAuth2 authorization/redirect URL is missing"
            );
        }
        if config.grant == OAuth2Grant::DeviceCode {
            ensure!(
                !config.device_url.is_empty(),
                "OAuth2 device URL is missing"
            );
        }
    }
    Ok(())
}

impl OAuth2Auth {
    /// An execution projection; never replaces saved configuration or inactive drafts.
    pub fn grant_configuration(&self) -> Self {
        let mut config = self.clone();
        config.revocation_url.clear();
        config.introspection_url.clear();
        if config.grant != OAuth2Grant::Password {
            config.username.clear();
            config.password.clear();
        }
        if !matches!(
            config.grant,
            OAuth2Grant::AuthorizationCode | OAuth2Grant::Implicit
        ) {
            config.authorization_url.clear();
            config.redirect_url.clear();
            config.authorization_params.clear();
        }
        if config.grant != OAuth2Grant::AuthorizationCode {
            config.pkce = false;
        }
        if config.grant != OAuth2Grant::DeviceCode {
            config.device_url.clear();
        }
        if config.grant == OAuth2Grant::Implicit {
            config.client_auth = OAuth2ClientAuth::Basic;
            config.client_secret.clear();
            config.token_url.clear();
            config.token_params.clear();
            config.token_headers.clear();
        }
        config.authorization_params.retain(|row| row.enabled);
        config.token_params.retain(|row| row.enabled);
        config.token_headers.retain(|row| row.enabled);
        config
    }
}
