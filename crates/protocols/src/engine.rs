use crate::*;
use anyhow::{Context, bail, ensure};
use eventsource_stream::Eventsource;
use futures_util::{SinkExt, StreamExt};
use moleapi_core::{Pair, checked_client, protocol_url, request_headers};
use reqwest_websocket::RequestBuilderExt;

pub(crate) fn headers(response: &reqwest::Response, mask: &dyn Fn(&str) -> String) -> Vec<Pair> {
    response
        .headers()
        .iter()
        .map(|(name, value)| Pair {
            id: uuid::Uuid::new_v4().to_string(),
            key: name.to_string(),
            value: if matches!(
                name.as_str(),
                "set-cookie" | "authorization" | "proxy-authenticate"
            ) {
                "[REDACTED]".into()
            } else {
                mask(value.to_str().unwrap_or("[binary]"))
            },
            enabled: true,
            secret: None,
            local_value: None,
        })
        .collect()
}
pub(crate) async fn run(
    session: &Arc<Session>,
    request: RequestSpec,
    policy: NetworkPolicy,
    mut commands: mpsc::Receiver<Message>,
    mask: Arc<dyn Fn(&str) -> String + Send + Sync>,
) -> Result<String> {
    if request.protocol.is_graphql() {
        return crate::graphql::run(session.clone(), request, policy, mask).await;
    }
    ensure!(
        request.method == "GET" && request.body_kind == "none",
        "SSE and WebSocket connections require GET with body mode None"
    );
    let ws = request.protocol == Protocol::Websocket;
    let mut url = protocol_url(&request.url, ws)?;
    for query in request.query.iter().filter(|p| p.enabled) {
        url.query_pairs_mut().append_pair(&query.key, &query.value);
    }
    let mut request_headers = request_headers(&request)?;
    if ws {
        url.set_scheme(if url.scheme() == "wss" {
            "https"
        } else {
            "http"
        })
        .map_err(|_| anyhow::anyhow!("Invalid WebSocket scheme"))?;
        let connect = async {
            for redirect in 0..=10 {
                let client = checked_client(&url, policy, request.verify_tls).await?;
                let config = tungstenite::protocol::WebSocketConfig::default()
                    .max_message_size(Some(MAX_MESSAGE))
                    .max_frame_size(Some(MAX_MESSAGE))
                    .write_buffer_size(0)
                    .max_write_buffer_size(MAX_MESSAGE * 2);
                let response = client
                    .get(url.clone())
                    .headers(request_headers.clone())
                    .upgrade()
                    .web_socket_config(config)
                    .send()
                    .await?;
                let status = response.status().as_u16();
                if request.follow_redirects
                    && matches!(status, 301 | 302 | 303 | 307 | 308)
                    && let Some(location) = response.headers().get("location")
                {
                    ensure!(redirect < 10, "Too many redirects");
                    let mut next =
                        url.join(location.to_str().context("Invalid redirect location")?)?;
                    if matches!(next.scheme(), "ws" | "wss") {
                        next.set_scheme(if next.scheme() == "wss" {
                            "https"
                        } else {
                            "http"
                        })
                        .map_err(|_| anyhow::anyhow!("Invalid redirect scheme"))?;
                    }
                    protocol_url(next.as_str(), false)?;
                    ensure!(
                        url.scheme() != "https" || next.scheme() == "https",
                        "HTTPS downgrade redirect blocked"
                    );
                    if url.origin() != next.origin() {
                        request_headers.remove("authorization");
                        request_headers.remove("cookie");
                    }
                    url = next;
                    continue;
                }
                let handshake = Handshake {
                    status,
                    headers: headers(&response, &*mask),
                };
                session.record.lock().unwrap().summary.handshake = Some(handshake.clone());
                let socket = response.into_websocket().await?;
                return Ok::<_, anyhow::Error>((socket, handshake));
            }
            bail!("Too many redirects")
        };
        let (mut socket, handshake) = tokio::select! {
            biased;
            _=session.cancel.cancelled()=>return Ok("Session closed while connecting".into()),
            result=tokio::time::timeout(Duration::from_millis(request.timeout_ms),connect)=>result.context("WebSocket connect timed out")??,
        };
        session.open(handshake)?;
        loop {
            tokio::select! {
                biased;
                _ = session.cancel.cancelled() => {
                    if matches!(tokio::time::timeout(Duration::from_secs(1),socket.send(Message::Close { code: reqwest_websocket::CloseCode::Normal, reason: "Client closed session".into() })).await,Ok(Ok(()))) {
                        session.event("outgoing",EventMessage::Close { code: Some(1000), reason: "Client closed session".into() })?;
                    }
                    return Ok("Session closed by client".into());
                }
                command = commands.recv() => {
                    let Some(command) = command else { return Ok("Session command channel closed".into()) };
                    let (event,size) = message_event(&command);
                    tokio::select! {
                        _=session.cancel.cancelled()=>return Ok("Session closed while sending".into()),
                        result=tokio::time::timeout(Duration::from_secs(5),socket.send(command))=>result.context("WebSocket send timed out")?.context("WebSocket send failed")?,
                    }
                    session.record.lock().unwrap().summary.sent_bytes += size as u64;
                    session.event("outgoing",event)?;
                }
                incoming = socket.next() => {
                    let Some(incoming) = incoming else { return Ok("WebSocket peer ended stream".into()) };
                    let incoming=incoming.context("WebSocket read failed")?;
                    let (event,size)=message_event(&incoming);
                    session.received(size)?;
                    if let Message::Close { code, reason } = &incoming {
                        session.event("incoming",event)?;
                        let _=tokio::time::timeout(Duration::from_secs(1),socket.flush()).await;
                        return Ok(mask(&format!("WebSocket closed ({}) {}",u16::from(*code),reason)));
                    }
                    session.event("incoming",event)?;
                    if let Message::Ping(bytes) = incoming {
                        // Tungstenite queues the protocol pong automatically; flushing sends it.
                        tokio::time::timeout(Duration::from_secs(1),socket.flush()).await.context("WebSocket pong timed out")??;
                        session.event("outgoing",EventMessage::Pong { base64: STANDARD.encode(bytes) })?;
                    }
                }
            }
        }
    }
    request_headers.insert(
        "accept",
        reqwest::header::HeaderValue::from_static("text/event-stream"),
    );
    let connect = async {
        for redirect in 0..=10 {
            let client = checked_client(&url, policy, request.verify_tls).await?;
            let response = client
                .get(url.clone())
                .headers(request_headers.clone())
                .send()
                .await?;
            if request.follow_redirects
                && matches!(response.status().as_u16(), 301 | 302 | 303 | 307 | 308)
                && let Some(location) = response.headers().get("location")
            {
                ensure!(redirect < 10, "Too many redirects");
                let next = url.join(location.to_str().context("Invalid redirect location")?)?;
                protocol_url(next.as_str(), false)?;
                ensure!(
                    url.scheme() != "https" || next.scheme() == "https",
                    "HTTPS downgrade redirect blocked"
                );
                if url.origin() != next.origin() {
                    request_headers.remove("authorization");
                    request_headers.remove("cookie");
                }
                url = next;
                continue;
            }
            ensure!(
                response.status().is_success(),
                "SSE endpoint returned HTTP {}",
                response.status().as_u16()
            );
            ensure!(
                response
                    .headers()
                    .get("content-type")
                    .and_then(|h| h.to_str().ok())
                    .is_some_and(|h| h
                        .split(';')
                        .next()
                        .is_some_and(|h| h.trim().eq_ignore_ascii_case("text/event-stream"))),
                "SSE endpoint requires Content-Type text/event-stream"
            );
            return Ok::<_, anyhow::Error>(response);
        }
        bail!("Too many redirects")
    };
    let response = tokio::select! {
        biased;
        _=session.cancel.cancelled()=>return Ok("Session closed while connecting".into()),
        result=tokio::time::timeout(Duration::from_millis(request.timeout_ms),connect)=>result.context("SSE connect timed out")??,
    };
    session.open(Handshake {
        status: response.status().as_u16(),
        headers: headers(&response, &*mask),
    })?;
    let mut wire = 0usize;
    // The mature parser does not expose its pending buffer. Conservatively cap wire
    // bytes since its last delivered event, retaining one bounded chunk after dispatch
    // as credit for a trailing partial event. Long comment-only streams may hit this cap.
    let pending = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let last_chunk = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let stream_pending = pending.clone();
    let stream_last = last_chunk.clone();
    let stream = response
        .bytes_stream()
        .flat_map(|chunk| {
            let chunks: Vec<Result<Vec<u8>>> = match chunk {
                Ok(chunk) if chunk.len() <= MAX_WIRE => chunk
                    .chunks(64 * 1024)
                    .map(|part| Ok(part.to_vec()))
                    .collect(),
                Ok(_) => vec![Err(anyhow::anyhow!("SSE wire-byte limit reached (8 MiB)"))],
                Err(error) => vec![Err(error).context("SSE read failed")],
            };
            futures_util::stream::iter(chunks)
        })
        .map(move |chunk| {
            use std::sync::atomic::Ordering::Relaxed;
            let chunk = chunk?;
            wire = wire.saturating_add(chunk.len());
            ensure!(wire <= MAX_WIRE, "SSE wire-byte limit reached (8 MiB)");
            let buffered = stream_pending.fetch_add(chunk.len(), Relaxed) + chunk.len();
            ensure!(
                buffered <= MAX_MESSAGE + 64 * 1024,
                "SSE pending-event wire limit reached (1 MiB plus one 64 KiB chunk)"
            );
            stream_last.store(chunk.len(), Relaxed);
            session.received(chunk.len())?;
            Ok::<_, anyhow::Error>(chunk)
        })
        .then(|chunk| async move {
            tokio::task::yield_now().await;
            chunk
        });
    let events = stream.eventsource();
    futures_util::pin_mut!(events);
    loop {
        let event = tokio::select! {
            biased;
            _=session.cancel.cancelled()=>return Ok("Session closed by client".into()),
            event=events.next()=>event,
        };
        let Some(event) = event else { break };
        let event = event.map_err(|e| anyhow::anyhow!("SSE parser/read error: {e}"))?;
        pending.store(
            last_chunk.load(std::sync::atomic::Ordering::Relaxed),
            std::sync::atomic::Ordering::Relaxed,
        );
        ensure!(
            event.data.len() + event.event.len() + event.id.len() <= MAX_MESSAGE,
            "SSE event exceeds 1 MiB"
        );
        session.event(
            "incoming",
            EventMessage::Sse {
                event: event.event,
                data: event.data,
                id: event.id,
                retry: event
                    .retry
                    .map(|d| d.as_millis().min(u64::MAX as u128) as u64),
            },
        )?;
    }
    Ok("SSE stream ended".into())
}
fn message_event(message: &Message) -> (EventMessage, usize) {
    match message {
        Message::Text(text) => (EventMessage::Text { text: text.clone() }, text.len()),
        Message::Binary(bytes) => (
            EventMessage::Binary {
                base64: STANDARD.encode(bytes),
            },
            bytes.len(),
        ),
        Message::Ping(bytes) => (
            EventMessage::Ping {
                base64: STANDARD.encode(bytes),
            },
            bytes.len(),
        ),
        Message::Pong(bytes) => (
            EventMessage::Pong {
                base64: STANDARD.encode(bytes),
            },
            bytes.len(),
        ),
        Message::Close { code, reason } => (
            EventMessage::Close {
                code: Some(u16::from(*code)),
                reason: reason.clone(),
            },
            reason.len(),
        ),
    }
}
