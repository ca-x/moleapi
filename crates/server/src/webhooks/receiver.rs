use super::{
    manager::{Gates, capacity},
    models::*,
    storage as vault,
};
use crate::{ApiError, entities::document, storage};
use axum::{
    body::{Body, to_bytes},
    extract::{Path, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, TransactionTrait};
use std::{sync::Arc, time::Duration};
use tokio::sync::Semaphore;
#[derive(Clone)]
pub struct ReceiverState {
    pub db: DatabaseConnection,
    pub gates: Arc<Gates>,
    pub intake: Arc<Semaphore>,
    pub stop: tokio_util::sync::CancellationToken,
}
pub async fn ingest(
    State(s): State<ReceiverState>,
    Path(token): Path<String>,
    request: axum::extract::Request,
) -> Result<Response, ApiError> {
    if token.len() != 32 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(ApiError::not_found());
    }
    let id = vault::token_id(&token);
    let (_, initial) = vault::inbox(&s.db, &id, None)
        .await?
        .ok_or_else(ApiError::not_found)?;
    if !initial.active {
        return Err(ApiError::not_found());
    }
    let _slot = s
        .intake
        .clone()
        .try_acquire_owned()
        .map_err(|_| capacity("Webhook intake capacity reached"))?;
    let (parts, body) = request.into_parts();
    let header_bytes: usize = parts
        .headers
        .iter()
        .map(|(k, v)| k.as_str().len() + v.as_bytes().len())
        .sum();
    if parts.headers.len() > 512
        || header_bytes > 32768
        || parts.uri.query().is_some_and(|q| q.len() > 32768)
    {
        return Err(ApiError::bad("Webhook request metadata exceeds limits"));
    }
    let headers = parts
        .headers
        .iter()
        .map(|(key, value)| Header {
            name: key.as_str().to_owned(),
            value_base64: STANDARD.encode(value.as_bytes()),
            value_text: std::str::from_utf8(value.as_bytes()).ok().map(String::from),
        })
        .collect();
    let bytes = tokio::select! { biased;
        _ = s.stop.cancelled() => return Err(ApiError::bad("Webhook listener stopped")),
        result = async { tokio::time::timeout(Duration::from_secs(5), to_bytes(body, 1048576))
        .await
        .map_err(|_| ApiError {
            status: StatusCode::REQUEST_TIMEOUT,
            message: "Webhook intake timed out".into(),
        })?
        .map_err(|_| ApiError {
            status: StatusCode::PAYLOAD_TOO_LARGE,
            message: "Webhook body exceeds 1 MiB".into(),
        }) } => result?,
    };
    let (row, _) = vault::inbox(&s.db, &id, None)
        .await?
        .ok_or_else(ApiError::not_found)?;
    let _gate = s.gates.lock(&row.owner, &initial.workspace_id).await;
    if s.stop.is_cancelled() {
        return Err(ApiError::bad("Webhook listener stopped"));
    }
    let tx = s.db.begin().await?;
    let (row, mut inbox) = vault::inbox(&tx, &id, None)
        .await?
        .ok_or_else(ApiError::not_found)?;
    if !inbox.active || inbox.config_epoch != initial.config_epoch {
        return Err(ApiError::not_found());
    }
    vault::ensure_workspace(&tx, &row.owner, &inbox.workspace_id).await?;
    if inbox.received == u64::MAX || inbox.revision >= i64::MAX - 1 {
        return Err(capacity("Webhook lifetime receipt limit reached"));
    }
    let previous = inbox.revision;
    inbox.received += 1;
    inbox.revision += 1;
    inbox.updated_at = storage::now();
    let capture = Capture {
        id: format!("wc_{}", uuid::Uuid::new_v4().simple()),
        inbox_id: id.clone(),
        workspace_id: inbox.workspace_id.clone(),
        cursor: inbox.received,
        received_at: inbox.updated_at.clone(),
        method: parts.method.as_str().to_owned(),
        query: parts.uri.query().unwrap_or_default().to_owned(),
        headers,
        body_base64: STANDARD.encode(&bytes),
        body_bytes: bytes.len(),
        response_status: inbox.response.status,
    };
    storage::insert_doc(
        &tx,
        capture.id.clone(),
        &row.owner,
        vault::CAPTURE,
        &id,
        1,
        &capture,
    )
    .await?;
    let mut all = vault::captures(&tx, &row.owner, &id).await?;
    all.sort_by_key(|c| c.cursor);
    let mut size: usize = all
        .iter()
        .map(|c| {
            c.body_bytes
                + c.headers
                    .iter()
                    .map(|h| h.name.len() + h.value_base64.len())
                    .sum::<usize>()
                + c.query.len()
        })
        .sum();
    while all.len() > 64 || size > 8 * 1048576 {
        let old = all.remove(0);
        size = size.saturating_sub(
            old.body_bytes
                + old
                    .headers
                    .iter()
                    .map(|h| h.name.len() + h.value_base64.len())
                    .sum::<usize>()
                + old.query.len(),
        );
        document::Entity::delete_many()
            .filter(document::Column::Id.eq(old.id))
            .filter(document::Column::Owner.eq(&row.owner))
            .filter(document::Column::Kind.eq(vault::CAPTURE))
            .exec(&tx)
            .await?;
        inbox.dropped += 1;
    }
    vault::update(&tx, &row.owner, &inbox, previous).await?;
    if s.stop.is_cancelled() {
        return Err(ApiError::bad("Webhook listener stopped"));
    }
    tx.commit().await?;
    response(&inbox.response, parts.method == axum::http::Method::HEAD)
}
fn response(reply: &Reply, head: bool) -> Result<Response, ApiError> {
    let nominations: Vec<String> = reply
        .headers
        .iter()
        .filter(|h| h.enabled && h.key.eq_ignore_ascii_case("connection"))
        .flat_map(|h| h.value.split(',').map(|s| s.trim().to_ascii_lowercase()))
        .collect();
    let mut headers = HeaderMap::new();
    for pair in reply.headers.iter().filter(|h| h.enabled) {
        let name = pair.key.to_ascii_lowercase();
        if [
            "host",
            "connection",
            "keep-alive",
            "proxy-authenticate",
            "proxy-authorization",
            "te",
            "trailer",
            "transfer-encoding",
            "upgrade",
            "content-length",
            "content-security-policy",
            "x-content-type-options",
        ]
        .contains(&name.as_str())
            || nominations.contains(&name)
        {
            continue;
        }
        headers.append(
            HeaderName::from_bytes(pair.key.as_bytes())
                .map_err(|_| ApiError::bad("Invalid receiver response header"))?,
            HeaderValue::from_str(&pair.value)
                .map_err(|_| ApiError::bad("Invalid receiver response header"))?,
        );
    }
    headers.insert(
        "content-security-policy",
        HeaderValue::from_static("sandbox; default-src 'none'; base-uri 'none'"),
    );
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    if !headers.contains_key("content-type") {
        headers.insert(
            "content-type",
            HeaderValue::from_static("text/plain; charset=utf-8"),
        );
    }
    Ok((
        StatusCode::from_u16(reply.status)
            .map_err(|_| ApiError::bad("Invalid receiver response status"))?,
        headers,
        if head {
            Body::empty()
        } else {
            Body::from(reply.body.clone())
        },
    )
        .into_response())
}
