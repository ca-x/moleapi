//! Pin each SDK request through the shared destination policy. The SDK owns SSE/JSON-RPC.
use futures_util::stream::BoxStream;
use http::{HeaderName, HeaderValue};
use moleapi_core::NetworkPolicy;
use rmcp::{
    model::ClientJsonRpcMessage,
    transport::streamable_http_client::{
        SseError, StreamableHttpClient, StreamableHttpError, StreamableHttpPostResponse,
    },
};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

#[derive(Clone)]
pub(super) struct CheckedHttp {
    pub endpoint: String,
    pub policy: NetworkPolicy,
    pub verify_tls: bool,
    pub timeout: Duration,
    pub headers: reqwest::header::HeaderMap,
    pub received: Arc<AtomicUsize>,
}
type Error = StreamableHttpError<std::io::Error>;
fn failure(text: &'static str) -> Error {
    Error::Client(std::io::Error::other(text))
}
impl CheckedHttp {
    async fn client(&self, uri: &str) -> Result<reqwest_mcp::Client, Error> {
        // Streamable HTTP has one endpoint. Never follow server-controlled URLs or redirects.
        if uri != self.endpoint {
            return Err(failure("MCP transport endpoint changed"));
        }
        let url =
            moleapi_core::protocol_url(uri, false).map_err(|_| failure("Invalid MCP endpoint"))?;
        let addresses = moleapi_core::checked_destination(&url, self.policy)
            .await
            .map_err(|_| failure("MCP endpoint blocked or DNS failed"))?;
        let mut headers = http::HeaderMap::new();
        for (key, value) in &self.headers {
            headers.insert(key.clone(), value.clone());
        }
        // reqwest 0.13's provider-free feature requires an explicit process default.
        // Reuse the workspace's ring provider without overriding a caller-installed provider.
        if rustls::crypto::CryptoProvider::get_default().is_none() {
            let _ = rustls::crypto::ring::default_provider().install_default();
        }
        reqwest_mcp::Client::builder()
            .no_proxy()
            .redirect(reqwest_mcp::redirect::Policy::none())
            .danger_accept_invalid_certs(!self.verify_tls)
            .resolve_to_addrs(url.host_str().unwrap().trim_matches(['[', ']']), &addresses)
            .connect_timeout(self.timeout.min(Duration::from_secs(15)))
            .timeout(self.timeout)
            .default_headers(headers)
            .build()
            .map_err(|_| failure("MCP HTTP client configuration failed"))
    }
    fn convert(error: StreamableHttpError<reqwest_mcp::Error>) -> Error {
        match error {
            StreamableHttpError::ServerDoesNotSupportSse => Error::ServerDoesNotSupportSse,
            StreamableHttpError::SessionExpired => Error::SessionExpired,
            // Do not let SDK error labels expose endpoint credentials or response body previews.
            _ => failure(
                "MCP HTTP transport failed (check endpoint, authentication, TLS and server response)",
            ),
        }
    }
    fn stream(
        &self,
        stream: BoxStream<'static, Result<sse_stream::Sse, SseError>>,
    ) -> BoxStream<'static, Result<sse_stream::Sse, SseError>> {
        use futures_util::StreamExt;
        let budget = self.received.clone();
        Box::pin(stream.map(move |item| {
            if let Ok(event) = &item {
                let size = event.data.as_ref().map_or(0, String::len);
                if budget
                    .fetch_add(size, Ordering::Relaxed)
                    .saturating_add(size)
                    > crate::MAX_WIRE
                {
                    return Err(SseError::Body(Box::new(std::io::Error::other(
                        "MCP wire budget exceeded",
                    ))));
                }
            }
            item
        }))
    }
}
impl StreamableHttpClient for CheckedHttp {
    type Error = std::io::Error;
    async fn post_message(
        &self,
        uri: Arc<str>,
        message: ClientJsonRpcMessage,
        session_id: Option<Arc<str>>,
        auth_header: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<StreamableHttpPostResponse, Error> {
        self.post_message_with_max_sse_event_size(
            uri,
            message,
            session_id,
            auth_header,
            custom_headers,
            crate::MAX_MESSAGE,
        )
        .await
    }
    async fn post_message_with_max_sse_event_size(
        &self,
        uri: Arc<str>,
        message: ClientJsonRpcMessage,
        session_id: Option<Arc<str>>,
        auth_header: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
        max: usize,
    ) -> Result<StreamableHttpPostResponse, Error> {
        let client = self.client(&uri).await?;
        let result = client
            .post_message_with_max_sse_event_size(
                uri,
                message,
                session_id,
                auth_header,
                custom_headers,
                max.min(crate::MAX_MESSAGE),
            )
            .await
            .map_err(Self::convert)?;
        match result {
            StreamableHttpPostResponse::Json(message, id) => {
                let size = serde_json::to_vec(&message)
                    .map_err(|_| failure("Invalid MCP response"))?
                    .len();
                if size > crate::MAX_MESSAGE
                    || self
                        .received
                        .fetch_add(size, Ordering::Relaxed)
                        .saturating_add(size)
                        > crate::MAX_WIRE
                {
                    return Err(failure("MCP response budget exceeded"));
                }
                Ok(StreamableHttpPostResponse::Json(message, id))
            }
            StreamableHttpPostResponse::Sse(stream, id) => {
                Ok(StreamableHttpPostResponse::Sse(self.stream(stream), id))
            }
            StreamableHttpPostResponse::Accepted => Ok(StreamableHttpPostResponse::Accepted),
            _ => Err(failure("Unsupported MCP response")),
        }
    }
    async fn get_stream(
        &self,
        uri: Arc<str>,
        session_id: Option<Arc<str>>,
        last_event_id: Option<String>,
        auth_header: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<BoxStream<'static, Result<sse_stream::Sse, SseError>>, Error> {
        self.get_stream_with_max_sse_event_size(
            uri,
            session_id,
            last_event_id,
            auth_header,
            custom_headers,
            crate::MAX_MESSAGE,
        )
        .await
    }
    async fn get_stream_with_max_sse_event_size(
        &self,
        uri: Arc<str>,
        session_id: Option<Arc<str>>,
        last_event_id: Option<String>,
        auth_header: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
        max: usize,
    ) -> Result<BoxStream<'static, Result<sse_stream::Sse, SseError>>, Error> {
        let stream = self
            .client(&uri)
            .await?
            .get_stream_with_max_sse_event_size(
                uri,
                session_id,
                last_event_id,
                auth_header,
                custom_headers,
                max.min(crate::MAX_MESSAGE),
            )
            .await
            .map_err(Self::convert)?;
        Ok(self.stream(stream))
    }
    async fn delete_session(
        &self,
        uri: Arc<str>,
        session_id: Arc<str>,
        auth_header: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<(), Error> {
        self.client(&uri)
            .await?
            .delete_session(uri, session_id, auth_header, custom_headers)
            .await
            .map_err(Self::convert)
    }
}
