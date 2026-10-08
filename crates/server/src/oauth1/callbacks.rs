use super::flows::{Complete, complete_flow};
use crate::{ApiError, AppState};
use axum::{
    Router,
    extract::{DefaultBodyLimit, OriginalUri, Query, State},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;
#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Mode {
    #[default]
    Manual,
    Hosted,
    Loopback,
}
pub(super) async fn prepare(
    s: &AppState,
    source: &str,
    mode: Mode,
    state: &str,
) -> Result<(String, Option<tokio::net::TcpListener>), ApiError> {
    if source == "oob" && matches!(mode, Mode::Manual) {
        return Ok((source.into(), None));
    }
    let mut uri = moleapi_core::valid_url(source)
        .map_err(|_| ApiError::bad("Invalid OAuth1 callback URI"))?;
    if uri.fragment().is_some()
        || uri
            .query_pairs()
            .any(|(k, _)| ["state", "oauth_token", "oauth_verifier"].contains(&k.as_ref()))
    {
        return Err(ApiError::bad(
            "OAuth1 callback owns state/token/verifier parameters",
        ));
    }
    let listener = match mode {
        Mode::Manual => None,
        Mode::Hosted => {
            if s.local || uri.path() != "/api/oauth1/callback" || uri.query().is_some() {
                return Err(ApiError::bad(
                    "Hosted OAuth1 callbacks require /api/oauth1/callback without a query",
                ));
            }
            None
        }
        Mode::Loopback => {
            if !s.local
                || uri.scheme() != "http"
                || uri.host_str() != Some("127.0.0.1")
                || uri.port().is_none_or(|p| p == 0)
                || uri.path() != "/api/oauth1/callback"
                || uri.query().is_some()
            {
                return Err(ApiError::bad(
                    "Desktop OAuth1 callbacks require http://127.0.0.1:<port>/api/oauth1/callback",
                ));
            }
            Some(
                tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, uri.port().unwrap()))
                    .await
                    .map_err(|_| ApiError::bad("OAuth1 callback port is already in use"))?,
            )
        }
    };
    uri.query_pairs_mut().append_pair("state", state);
    Ok((uri.to_string(), listener))
}
pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/oauth1/callback", get(redirect))
        .layer(DefaultBodyLimit::max(16384))
}
#[derive(Deserialize)]
struct Callback {
    state: String,
    oauth_token: Option<String>,
    oauth_verifier: Option<String>,
    denied: Option<String>,
    oauth_problem: Option<String>,
}
fn page(ok: bool) -> Response {
    let text = if ok {
        "Authorization completed. Return to MoleAPI. / 授权完成，请返回 MoleAPI。"
    } else {
        "Authorization failed or expired. Return to MoleAPI and retry. / 授权失败或已过期，请返回 MoleAPI 重试。"
    };
    let nonce = uuid::Uuid::new_v4().simple().to_string();
    let csp = format!(
        "default-src 'none'; script-src 'nonce-{nonce}'; base-uri 'none'; frame-ancestors 'none'"
    );
    ([("cache-control","no-store".to_string()),("referrer-policy","no-referrer".to_string()),("content-security-policy",csp)],Html(format!("<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>MoleAPI</title><p>{text}</p><script nonce=\"{nonce}\">history.replaceState(null,'',location.pathname)</script></html>"))).into_response()
}
async fn redirect(
    State(s): State<AppState>,
    OriginalUri(uri): OriginalUri,
    input: Result<Query<Callback>, axum::extract::rejection::QueryRejection>,
) -> Response {
    if uri.to_string().len() > 16384 {
        return page(false);
    }
    let Ok(Query(input)) = input else {
        return page(false);
    };
    let outcome = async {
        let flow = s.oauth1_flows.callback(&input.state)?;
        if input.denied.is_some() || input.oauth_problem.is_some() {
            super::flows::reject(
                &flow,
                input.denied.as_deref().or(input.oauth_token.as_deref()),
            )
            .await?;
            return Ok(false);
        }
        let token = input
            .oauth_token
            .ok_or_else(|| ApiError::bad("OAuth1 callback token missing"))?;
        let verifier = input
            .oauth_verifier
            .ok_or_else(|| ApiError::bad("OAuth1 callback verifier missing"))?;
        let status = complete_flow(
            &s,
            &flow,
            Complete {
                state: Some(input.state),
                token: Some(token),
                verifier,
            },
        )
        .await?;
        Ok::<_, ApiError>(status.stage == "completed")
    }
    .await;
    page(outcome.is_ok_and(|ok| ok))
}
