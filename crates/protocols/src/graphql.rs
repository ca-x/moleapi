//! Checked WebSocket transport adapter for graphql-ws-client's protocol engine.
use crate::*;
use anyhow::{Context, bail, ensure};
use futures_util::{SinkExt, StreamExt};
use graphql_ws_client::{
    Connection, Message as GqlMessage, graphql::GraphqlOperation, protocol::Event,
};
use moleapi_core::{
    GraphqlPayload, checked_client, graphql_is_subscription, graphql_payload, protocol_url,
    request_headers,
};
use reqwest_websocket::{RequestBuilderExt, WebSocket};
use serde_json::Value;
use std::future::IntoFuture;

struct DynamicOperation(GraphqlPayload);
impl serde::Serialize for DynamicOperation {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}
impl GraphqlOperation for DynamicOperation {
    type Response = Value;
    type Error = serde_json::Error;
    fn decode(&self, value: Value) -> std::result::Result<Value, Self::Error> {
        Ok(value)
    }
}
struct CheckedConnection {
    socket: WebSocket,
    session: Arc<Session>,
    failure: Arc<Mutex<Option<String>>>,
    mask: Arc<dyn Fn(&str) -> String + Send + Sync>,
}
impl CheckedConnection {
    fn failed(&self, reason: String) -> Option<GqlMessage> {
        *self.failure.lock().unwrap() = Some(reason.clone());
        Some(GqlMessage::Close {
            code: Some(1011),
            reason: Some(reason),
        })
    }
}
impl Connection for CheckedConnection {
    fn on_event(&mut self, event: &Event) {
        // The dependency invokes this only after validating/routing an active operation.
        let event = match event {
            Event::Next { id, payload } => Some(EventMessage::GraphqlNext {
                operation_id: id.clone(),
                payload: payload.clone(),
            }),
            Event::Error { id, payload } => Some(EventMessage::GraphqlError {
                operation_id: id.clone(),
                payload: Value::Array(payload.clone()),
            }),
            Event::Complete { id } => Some(EventMessage::GraphqlComplete {
                operation_id: id.clone(),
                payload: Value::Null,
            }),
            _ => None,
        };
        if let Some(event) = event
            && let Err(error) = self.session.event("incoming", event)
        {
            *self.failure.lock().unwrap() = Some(error.to_string());
        }
    }

    async fn receive(&mut self) -> Option<GqlMessage> {
        let message = match self.socket.next().await {
            Some(Ok(message)) => message,
            Some(Err(error)) => {
                return self.failed(format!("GraphQL WebSocket read failed: {error}"));
            }
            None => {
                return self
                    .failed("GraphQL WebSocket ended without a complete/close event".into());
            }
        };
        let size = match &message {
            Message::Text(t) => t.len(),
            Message::Binary(b) | Message::Ping(b) | Message::Pong(b) => b.len(),
            Message::Close { reason, .. } => reason.len(),
        };
        if let Err(error) = self.session.received(size) {
            return self.failed(error.to_string());
        }
        match message {
            Message::Text(text) => Some(GqlMessage::Text(text)),
            Message::Binary(_) => self.failed("GraphQL WebSocket requires text messages".into()),
            Message::Ping(_) => {
                if let Err(error) = self.socket.flush().await {
                    return self.failed(format!("GraphQL WebSocket pong failed: {error}"));
                }
                Some(GqlMessage::Ping)
            }
            Message::Pong(_) => Some(GqlMessage::Pong),
            Message::Close { code, reason } => {
                let code = u16::from(code);
                let reason = (self.mask)(&reason);
                let _ = self.session.event(
                    "incoming",
                    EventMessage::Close {
                        code: Some(code),
                        reason: reason.clone(),
                    },
                );
                if code != 1000 && code != 1001 {
                    *self.failure.lock().unwrap() =
                        Some(format!("GraphQL WebSocket closed ({code}) {reason}"));
                }
                Some(GqlMessage::Close {
                    code: Some(code),
                    reason: Some(reason),
                })
            }
        }
    }
    async fn send(
        &mut self,
        message: GqlMessage,
    ) -> std::result::Result<(), graphql_ws_client::Error> {
        let message = match message {
            GqlMessage::Text(text) => {
                self.session.record.lock().unwrap().summary.sent_bytes += text.len() as u64;
                Message::Text(text) // connection_init/auth parameters are deliberately never recorded.
            }
            GqlMessage::Close { code, reason } => {
                let code = code.unwrap_or(1000);
                let reason = reason.unwrap_or_default();
                if code >= 4000 {
                    *self.failure.lock().unwrap() = Some((self.mask)(&format!(
                        "GraphQL protocol closed ({code}) {reason}"
                    )));
                }
                let _ = self.session.event(
                    "outgoing",
                    EventMessage::Close {
                        code: Some(code),
                        reason: (self.mask)(&reason),
                    },
                );
                Message::Close {
                    code: reqwest_websocket::CloseCode::from(code),
                    reason,
                }
            }
            GqlMessage::Ping => Message::Ping(Vec::new().into()),
            GqlMessage::Pong => Message::Pong(Vec::new().into()),
        };
        self.socket.send(message).await.map_err(|error| {
            let reason = format!("GraphQL WebSocket send failed: {error}");
            *self.failure.lock().unwrap() = Some(reason.clone());
            graphql_ws_client::Error::Send(reason)
        })
    }
}
pub(crate) async fn run(
    session: Arc<Session>,
    request: RequestSpec,
    policy: NetworkPolicy,
    mask: Arc<dyn Fn(&str) -> String + Send + Sync>,
) -> Result<String> {
    ensure!(
        graphql_is_subscription(&request)?,
        "GraphQL query/mutation requests use the execute API"
    );
    let Protocol::Graphql {
        connection_params,
        subscription_url,
        ..
    } = &request.protocol
    else {
        unreachable!()
    };
    let mut url = protocol_url(
        subscription_url.as_deref().unwrap_or(&request.url),
        subscription_url.is_some(),
    )?;
    url.set_scheme(if matches!(url.scheme(), "https" | "wss") {
        "https"
    } else {
        "http"
    })
    .map_err(|_| anyhow::anyhow!("Invalid GraphQL WebSocket URL"))?;
    for query in request.query.iter().filter(|p| p.enabled) {
        url.query_pairs_mut().append_pair(&query.key, &query.value);
    }
    let mut headers = request_headers(&request)?;
    let mut parameters = connection_params.clone();
    let connect = async {
        for redirect in 0..=10 {
            let client = checked_client(&url, policy, request.verify_tls).await?;
            let response = client
                .get(url.clone())
                .headers(headers.clone())
                .upgrade()
                .protocols(["graphql-transport-ws"])
                .web_socket_config(
                    tungstenite::protocol::WebSocketConfig::default()
                        .max_message_size(Some(MAX_MESSAGE))
                        .max_frame_size(Some(MAX_MESSAGE))
                        .write_buffer_size(0)
                        .max_write_buffer_size(MAX_MESSAGE * 2),
                )
                .send()
                .await?;
            let status = response.status().as_u16();
            if request.follow_redirects
                && matches!(status, 301 | 302 | 303 | 307 | 308)
                && let Some(location) = response.headers().get("location")
            {
                ensure!(redirect < 10, "Too many redirects");
                let next = url.join(
                    location
                        .to_str()
                        .context("Invalid GraphQL redirect location")?,
                )?;
                protocol_url(next.as_str(), false)?;
                ensure!(
                    url.scheme() != "https" || next.scheme() == "https",
                    "HTTPS downgrade redirect blocked"
                );
                if url.origin() != next.origin() {
                    headers.remove("authorization");
                    headers.remove("cookie");
                    // Connection parameters are auth-bearing and never forwarded across origins.
                    parameters = serde_json::json!({});
                }
                url = next;
                continue;
            }
            let handshake = Handshake {
                status,
                headers: crate::engine::headers(&response, &*mask),
            };
            session.record.lock().unwrap().summary.handshake = Some(handshake.clone());
            let socket = response.into_websocket().await.context("GraphQL requires negotiated graphql-transport-ws (legacy graphql-ws is unsupported)")?;
            ensure!(
                socket.protocol() == Some("graphql-transport-ws"),
                "Endpoint must negotiate graphql-transport-ws (legacy graphql-ws is unsupported)"
            );
            let failure = Arc::new(Mutex::new(None));
            let connection = CheckedConnection {
                socket,
                session: session.clone(),
                failure: failure.clone(),
                mask: mask.clone(),
            };
            let (client, actor) = graphql_ws_client::Client::build(connection)
                .payload(&parameters)?
                .subscription_buffer_size(8)
                .await?;
            return Ok::<_, anyhow::Error>((client, actor, failure, handshake));
        }
        bail!("Too many redirects")
    };
    let (client, actor, failure, handshake) = tokio::select! {
        biased;
        _ = session.cancel.cancelled() => return Ok("Session closed while connecting".into()),
        result = tokio::time::timeout(Duration::from_millis(request.timeout_ms),connect) => result.context("GraphQL WebSocket connection/ack timed out")??,
    };
    session.open(handshake)?;
    let actor = actor.into_future();
    futures_util::pin_mut!(actor);
    let mut subscription = client
        .subscribe(DynamicOperation(graphql_payload(&request)?))
        .await?;
    let id = subscription.id();
    let cancelled = loop {
        tokio::select! {
            biased;
            _ = session.cancel.cancelled() => break true,
            result = subscription.next() => if result.is_none() { break false; },
            _ = &mut actor => {
                if let Some(reason) = failure.lock().unwrap().take() { bail!("{reason}") }
                return Ok("GraphQL WebSocket peer closed".into());
            }
        }
    };
    if cancelled {
        client.stop(id).await?;
    }
    client.close(1000, "Client closed GraphQL session").await;
    let _ = tokio::time::timeout(Duration::from_secs(1), &mut actor).await;
    if let Some(reason) = failure.lock().unwrap().take() {
        bail!("{reason}")
    }
    Ok(if cancelled {
        "Session closed by client"
    } else {
        "GraphQL subscription completed"
    }
    .into())
}
