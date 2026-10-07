//! Anonymous redirects authorize only a previously authenticated, high-entropy flow.
use super::flows::{Complete, complete_flow};
use crate::{ApiError, AppState};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, OriginalUri, Query, State},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use moleapi_core::{OAuth2Auth, OAuth2Grant};
use serde::Deserialize;

#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Mode {
    #[default]
    Manual,
    Hosted,
    Loopback,
}

pub(super) async fn listener(
    s: &AppState,
    config: &OAuth2Auth,
    mode: Mode,
) -> Result<Option<tokio::net::TcpListener>, ApiError> {
    if matches!(mode, Mode::Manual) {
        return Ok(None);
    }
    if !matches!(
        config.grant,
        OAuth2Grant::AuthorizationCode | OAuth2Grant::Implicit
    ) {
        return Err(ApiError::bad(
            "Automatic OAuth2 callbacks require a browser grant",
        ));
    }
    let uri = url::Url::parse(&config.redirect_url)
        .map_err(|_| ApiError::bad("Invalid OAuth2 callback URI"))?;
    if uri.path() != "/api/oauth2/callback"
        || uri.query().is_some()
        || uri.fragment().is_some()
        || !uri.username().is_empty()
        || uri.password().is_some()
    {
        return Err(ApiError::bad(
            "Automatic OAuth2 callbacks require /api/oauth2/callback without query or fragment",
        ));
    }
    match mode {
        Mode::Hosted => {
            if s.local || !matches!(uri.scheme(), "http" | "https") {
                return Err(ApiError::bad(
                    "Hosted OAuth2 callbacks require the hosted server URL",
                ));
            }
            Ok(None)
        }
        Mode::Loopback => {
            if !s.local
                || uri.scheme() != "http"
                || uri.host_str() != Some("127.0.0.1")
                || uri.port().is_none_or(|port| port == 0)
            {
                return Err(ApiError::bad(
                    "Desktop OAuth2 callbacks require http://127.0.0.1:<port>/api/oauth2/callback",
                ));
            }
            let listener =
                tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, uri.port().unwrap()))
                    .await
                    .map_err(|_| ApiError::bad("OAuth2 callback port is already in use"))?;
            Ok(Some(listener))
        }
        Mode::Manual => Ok(None),
    }
}
pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/oauth2/callback", get(redirect).post(fragment))
        .layer(DefaultBodyLimit::max(16 * 1024))
}
#[derive(Deserialize)]
struct Redirect {
    state: Option<String>,
    code: Option<String>,
    error: Option<String>,
}
fn page(ok: bool) -> Response {
    let text = if ok {
        "Authorization completed. Return to MoleAPI. / 授权完成，请返回 MoleAPI。"
    } else {
        "Authorization failed or expired. Return to MoleAPI and retry. / 授权失败或已过期，请返回 MoleAPI 重试。"
    };
    ([("cache-control", "no-store"), ("referrer-policy", "no-referrer"), ("content-security-policy", "default-src 'none'; base-uri 'none'; frame-ancestors 'none'")], Html(format!("<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>MoleAPI</title><p>{text}</p></html>"))).into_response()
}
async fn redirect(
    State(s): State<AppState>,
    OriginalUri(uri): OriginalUri,
    Query(input): Query<Redirect>,
) -> Response {
    if uri.to_string().len() > 16384 {
        return page(false);
    }
    if input.state.is_none() && input.code.is_none() && input.error.is_none() {
        // Implicit credentials remain in this browser until sent to the state-bound broker.
        let nonce = uuid::Uuid::new_v4().simple().to_string();
        let script = r#"const p=new URLSearchParams(location.hash.slice(1));history.replaceState(null,'',location.pathname);const v={state:p.get('state'),error:p.get('error'),implicit:{access_token:p.get('access_token'),token_type:p.get('token_type'),scope:p.get('scope')||undefined,expires_in:p.has('expires_in')?Number(p.get('expires_in')):undefined}};fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(v),credentials:'omit',cache:'no-store'}).then(r=>{document.getElementById('status').textContent=r.ok?'Authorization completed. Return to MoleAPI. / 授权完成，请返回 MoleAPI。':'Authorization failed. Return to MoleAPI and retry. / 授权失败，请返回 MoleAPI 重试。';}).catch(()=>{document.getElementById('status').textContent='Authorization failed. Return to MoleAPI and retry.';});"#;
        let csp = format!(
            "default-src 'none'; script-src 'nonce-{nonce}'; connect-src 'self'; base-uri 'none'; frame-ancestors 'none'"
        );
        return ([("cache-control", "no-store".to_string()), ("referrer-policy", "no-referrer".to_string()), ("content-security-policy", csp)], Html(format!("<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>MoleAPI</title><p id=\"status\">Completing authorization / 正在完成授权</p><script nonce=\"{nonce}\">{script}</script></html>"))).into_response();
    }
    let Some(state) = input.state else {
        return page(false);
    };
    let Ok(flow) = s.oauth2_flows.callback(&state) else {
        return page(false);
    };
    let result = complete_flow(
        &s,
        &flow,
        Complete {
            state,
            code: input.code,
            implicit: None,
            error: input.error,
        },
    )
    .await;
    page(result.is_ok_and(|status| status.0.stage == "completed"))
}
async fn fragment(State(s): State<AppState>, Json(input): Json<Complete>) -> Response {
    let result = async {
        let flow = s.oauth2_flows.callback(&input.state)?;
        let status = complete_flow(&s, &flow, input).await?;
        if status.0.stage != "completed" {
            return Err(ApiError::bad("OAuth2 authorization failed"));
        }
        Ok::<_, ApiError>(())
    }
    .await;
    // Never return token IDs, provider errors, verifier or credentials to an anonymous caller.
    match result {
        Ok(()) => (
            [("cache-control", "no-store")],
            Json(serde_json::json!({"ok":true})),
        )
            .into_response(),
        Err(_) => (
            axum::http::StatusCode::BAD_REQUEST,
            [("cache-control", "no-store")],
            Json(serde_json::json!({"error":"OAuth2 callback rejected"})),
        )
            .into_response(),
    }
}
