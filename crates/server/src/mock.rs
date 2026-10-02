use crate::{ApiError, AppState, auth::Identity, workspaces::owned};
use axum::{
    Extension,
    extract::{Path, State},
};
use axum::{
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::IntoResponse,
};
#[derive(Clone)]
pub(crate) struct SavedExample;

pub async fn mock(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path((id, request_id, example_id)): Path<(String, String, String)>,
) -> Result<axum::response::Response, ApiError> {
    let w = owned(&s, &owner.0, &id).await?;
    let e = w
        .data
        .collections
        .iter()
        .flat_map(|c| &c.requests)
        .find(|r| r.id == request_id)
        .and_then(|r| r.examples.iter().find(|e| e.id == example_id))
        .ok_or_else(ApiError::not_found)?;
    let mut headers = HeaderMap::new();
    let nominated: Vec<String> = e
        .headers
        .iter()
        .filter(|h| h.enabled && h.key.eq_ignore_ascii_case("connection"))
        .flat_map(|h| h.value.split(',').map(|v| v.trim().to_ascii_lowercase()))
        .collect();
    for h in e.headers.iter().filter(|h| h.enabled) {
        let name = h.key.to_ascii_lowercase();
        if [
            "connection",
            "keep-alive",
            "proxy-authenticate",
            "proxy-authorization",
            "te",
            "trailer",
            "transfer-encoding",
            "upgrade",
            "content-length",
        ]
        .contains(&name.as_str())
            || nominated.contains(&name)
        {
            continue;
        }
        headers.append(
            HeaderName::from_bytes(h.key.as_bytes())
                .map_err(|_| ApiError::bad("Invalid mock header"))?,
            HeaderValue::from_str(&h.value).map_err(|_| ApiError::bad("Invalid mock header"))?,
        );
    }
    let mut response = (
        StatusCode::from_u16(e.status).map_err(|_| ApiError::bad("Invalid mock status"))?,
        headers,
        e.body.clone(),
    )
        .into_response();
    response.extensions_mut().insert(SavedExample);
    Ok(response)
}
