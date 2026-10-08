use crate::{ApiError, AppState, execution, workspaces::owned};
use moleapi_core::{Environment, OAuth1Auth, OAuth1Grant, Pair, VariableUpdate};
use serde::{Deserialize, Serialize};
#[derive(Deserialize)]
pub(crate) struct Input {
    pub workspace_id: String,
    pub config: OAuth1Auth,
    #[serde(default)]
    pub label: String,
    pub collection_id: Option<String>,
    pub environment_id: Option<String>,
    #[serde(default)]
    pub locals: Vec<VariableUpdate>,
    #[serde(default = "verify_tls")]
    pub verify_tls: bool,
}
fn verify_tls() -> bool {
    true
}
pub(super) fn label(s: &str) -> Result<String, ApiError> {
    if s.len() > 128 || s.chars().any(char::is_control) {
        return Err(ApiError::bad("Invalid OAuth1 token label"));
    }
    Ok(if s.trim().is_empty() {
        "OAuth1 token".into()
    } else {
        s.into()
    })
}
#[derive(Clone, Serialize)]
pub(super) struct Profile {
    version: u32,
    pub consumer_key: String,
    request_token_url: String,
    authorization_url: String,
    access_token_url: String,
    purpose_hash: String,
}
impl Profile {
    pub fn hash(&self) -> Result<String, ApiError> {
        use sha2::{Digest, Sha256};
        Ok(hex::encode(Sha256::digest(
            serde_json::to_vec(self).map_err(|_| ApiError::internal())?,
        )))
    }
    pub fn issuer(&self) -> String {
        url::Url::parse(&self.access_token_url)
            .map(|u| u.origin().ascii_serialization())
            .unwrap_or_default()
    }
}
fn field(s: &str, e: &Environment) -> Result<String, ApiError> {
    moleapi_core::resolve_value(s, e).map_err(|e| ApiError::bad(e.to_string()))
}
fn rows(source: &[Pair], e: &Environment) -> Result<Vec<Pair>, ApiError> {
    source
        .iter()
        .filter(|r| r.enabled)
        .map(|r| {
            Ok(Pair {
                id: r.id.clone(),
                key: field(&r.key, e)?,
                value: field(&r.value, e)?,
                enabled: true,
                secret: r.secret,
                local_value: None,
            })
        })
        .collect()
}
pub(super) fn profile(
    c: &OAuth1Auth,
    g: Option<&OAuth1Grant>,
    e: &Environment,
) -> Result<Profile, ApiError> {
    let url = |raw: &str| -> Result<String, ApiError> {
        let value = field(raw, e)?;
        if value.is_empty() {
            return Ok(value);
        }
        moleapi_core::valid_url(&value)
            .map(|url| url.to_string())
            .map_err(|e| ApiError::bad(e.to_string()))
    };
    let empty = OAuth1Grant::default();
    let g = g.unwrap_or(&empty);
    let public_params = |source: &[Pair]| -> Result<Vec<(String, String)>, ApiError> {
        source
            .iter()
            .filter(|row| row.enabled)
            .filter_map(|row| {
                let key = match field(&row.key, e) {
                    Ok(key) => key,
                    Err(error) => return Some(Err(error)),
                };
                let identity = [
                    "scope",
                    "scopes",
                    "audience",
                    "resource",
                    "tenant",
                    "tenant_id",
                    "username",
                    "user",
                    "user_id",
                ]
                .contains(&key.to_ascii_lowercase().as_str());
                if !identity
                    && (row.secret == Some(true) || moleapi_core::sensitive_query_key(&key))
                {
                    return None;
                }
                Some(field(&row.value, e).map(|value| (key, value)))
            })
            .collect()
    };
    use sha2::{Digest, Sha256};
    let purpose_hash = hex::encode(Sha256::digest(
        serde_json::to_vec(&(
            public_params(&g.request_params)?,
            public_params(&g.access_params)?,
        ))
        .map_err(|_| ApiError::internal())?,
    ));
    Ok(Profile {
        version: 1,
        consumer_key: c.consumer_key.clone(),
        request_token_url: url(&g.request_token_url)?,
        authorization_url: url(&g.authorization_url)?,
        access_token_url: url(&g.access_token_url)?,
        purpose_hash,
    })
}
pub(super) struct Resolved {
    pub auth: OAuth1Auth,
    pub grant: OAuth1Grant,
    pub profile: Profile,
    pub label: String,
}
pub(super) async fn resolve(
    s: &AppState,
    owner: &str,
    input: &Input,
    exchange: bool,
) -> Result<Resolved, ApiError> {
    moleapi_core::validate_oauth1(&input.config, true).map_err(|e| ApiError::bad(e.to_string()))?;
    let w = owned(s, owner, &input.workspace_id).await?;
    let collection = input
        .collection_id
        .as_deref()
        .map(|id| {
            w.data
                .collections
                .iter()
                .find(|c| c.id == id)
                .ok_or_else(ApiError::not_found)
        })
        .transpose()?;
    let env = execution::environment(&w, input.environment_id.as_deref())?;
    let scopes = execution::variables(s, &w, collection, env, &[], &[], &input.locals)?;
    let effective = scopes.effective();
    let mut source = input.config.clone();
    source.token_id = None;
    source.token.clear();
    source.token_secret.clear();
    source.callback.clear();
    source.verifier.clear();
    source.nonce.clear();
    source.timestamp.clear();
    source.realm.clear();
    if !exchange {
        source.consumer_secret.clear();
        source.private_key.clear();
    }
    let auth = if exchange {
        let request:moleapi_core::RequestSpec=serde_json::from_value(serde_json::json!({"id":"oauth1-config","name":"OAuth1","method":"GET","url":"https://oauth1.invalid/","description":"","headers":[],"query":[],"body_kind":"none","body":"","auth":{"kind":"oauth1","token":"","username":"","password":"","oauth1":source},"timeout_ms":30000,"verify_tls":true,"follow_redirects":false,"assertions":[],"examples":[]})).map_err(|_|ApiError::bad("Invalid OAuth1 configuration"))?;
        moleapi_core::resolve_request(&request, Some(&effective))
            .map_err(|e| ApiError::bad(e.to_string()))?
            .auth
            .oauth1
            .map(|c| *c)
            .ok_or_else(ApiError::internal)?
    } else {
        source.consumer_key = field(&input.config.consumer_key, &effective)?;
        source.grant = None;
        source
    };
    if auth.consumer_key.is_empty() {
        return Err(ApiError::bad("Configure OAuth1 consumer key"));
    }
    let profile = profile(&auth, input.config.grant.as_deref(), &effective)?;
    let mut grant = OAuth1Grant {
        request_token_url: profile.request_token_url.clone(),
        authorization_url: profile.authorization_url.clone(),
        access_token_url: profile.access_token_url.clone(),
        ..Default::default()
    };
    if exchange {
        let g = input
            .config
            .grant
            .as_ref()
            .ok_or_else(|| ApiError::bad("Configure OAuth1 authorization endpoints"))?;
        if [
            &grant.request_token_url,
            &grant.authorization_url,
            &grant.access_token_url,
        ]
        .iter()
        .any(|s| s.is_empty())
        {
            return Err(ApiError::bad("Configure OAuth1 authorization endpoints"));
        }
        grant.callback_url = field(&g.callback_url, &effective)?;
        if grant.callback_url.is_empty() {
            grant.callback_url = "oob".into();
        }
        grant.request_params = rows(&g.request_params, &effective)?;
        grant.access_params = rows(&g.access_params, &effective)?;
        moleapi_core::validate_oauth1_grant_fields(&grant)
            .map_err(|e| ApiError::bad(e.to_string()))?;
        moleapi_core::validate_oauth1(&auth, false).map_err(|e| ApiError::bad(e.to_string()))?;
    }
    Ok(Resolved {
        auth,
        grant,
        profile,
        label: label(&input.label)?,
    })
}
