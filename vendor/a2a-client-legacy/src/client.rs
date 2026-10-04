//! A2A Client for calling remote A2A agents
//!
//! This module provides a client for making A2A protocol calls to remote agents.
//! It supports both streaming and non-streaming interactions.

use crate::constants::{AGENT_CARD_PATH, JSONRPC_VERSION};
use crate::error::{A2AError, A2AResult};
use a2a_types::{
    AgentCard, DeleteTaskPushNotificationConfigParams, JSONRPCErrorResponse, JSONRPCId,
    ListTaskPushNotificationConfigParams, MessageSendParams, SendMessageResult,
    SendStreamingMessageResult, Task, TaskIdParams, TaskPushNotificationConfig, TaskQueryParams,
};
use futures_core::Stream;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// A2A client for communicating with remote agents
#[derive(Clone)]
pub struct A2AClient {
    /// HTTP client for making requests
    client: Client,
    /// Service endpoint URL from agent card
    service_endpoint_url: String,
    /// Optional authentication token
    auth_token: Option<String>,
    /// Request ID counter for JSON-RPC requests
    request_id_counter: Arc<AtomicU64>,
    /// Cached agent card
    agent_card: Arc<AgentCard>,
}

/// JSON-RPC 2.0 request structure
#[derive(Debug, Serialize)]
struct JsonRpcRequest<T> {
    jsonrpc: String,
    id: JSONRPCId,
    method: String,
    params: T,
}

/// JSON-RPC 2.0 response structure
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum JsonRpcResponse<T> {
    Success {
        jsonrpc: String,
        id: Option<JSONRPCId>,
        result: T,
    },
    Error(JSONRPCErrorResponse),
}

impl A2AClient {
    /// Create a new A2A client from an agent card URL
    ///
    /// This will fetch the agent card from the specified URL and use the
    /// service endpoint URL from the card for all subsequent requests.
    ///
    /// Uses a default `reqwest::Client` for HTTP requests. For custom HTTP
    /// configuration, use `from_card_url_with_client()`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use a2a_client::A2AClient;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = A2AClient::from_card_url("https://agent.example.com").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn from_card_url(base_url: impl AsRef<str>) -> A2AResult<Self> {
        Self::from_card_url_with_client(base_url, Client::new()).await
    }

    /// Create a new A2A client from an agent card URL with a custom HTTP client
    ///
    /// This allows you to provide a pre-configured `reqwest::Client` with
    /// custom settings like timeouts, proxies, TLS config, default headers, etc.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use a2a_client::A2AClient;
    /// use reqwest::Client;
    /// use std::time::Duration;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let http_client = Client::builder()
    ///     .timeout(Duration::from_secs(30))
    ///     .build()?;
    ///
    /// let client = A2AClient::from_card_url_with_client(
    ///     "https://agent.example.com",
    ///     http_client
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn from_card_url_with_client(
        base_url: impl AsRef<str>,
        http_client: Client,
    ) -> A2AResult<Self> {
        let base_url = base_url.as_ref().trim_end_matches('/');
        let card_url = format!("{}/{}", base_url, AGENT_CARD_PATH);

        let response = http_client
            .get(&card_url)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| A2AError::NetworkError {
                message: format!("Failed to fetch agent card from {}: {}", card_url, e),
            })?;

        if !response.status().is_success() {
            return Err(A2AError::NetworkError {
                message: format!("Failed to fetch agent card: HTTP {}", response.status()),
            });
        }

        let agent_card: AgentCard = serde_json::from_slice(&bounded_bytes(response).await?)
            .map_err(|e| A2AError::SerializationError {
                message: format!("Failed to parse agent card: {}", e),
            })?;

        if agent_card.url.is_empty() {
            return Err(A2AError::InvalidParameter {
                message: "Agent card does not contain a valid 'url' for the service endpoint"
                    .to_string(),
            });
        }

        Ok(Self {
            client: http_client,
            service_endpoint_url: agent_card.url.clone(),
            auth_token: None,
            request_id_counter: Arc::new(AtomicU64::new(1)),
            agent_card: Arc::new(agent_card),
        })
    }

    /// Create a new A2A client directly from an agent card
    ///
    /// This is useful when you already have an agent card and don't need to fetch it.
    /// Uses a default `reqwest::Client`. For custom HTTP configuration, use `from_card_with_client()`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use a2a_client::A2AClient;
    /// use a2a_types::AgentCard;
    ///
    /// # fn example(agent_card: AgentCard) -> Result<(), Box<dyn std::error::Error>> {
    /// let client = A2AClient::from_card(agent_card)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_card(agent_card: AgentCard) -> A2AResult<Self> {
        Self::from_card_with_client(agent_card, Client::new())
    }

    /// Create a new A2A client from an agent card with a custom HTTP client
    ///
    /// This allows you to provide a pre-configured `reqwest::Client` with
    /// custom settings like timeouts, proxies, TLS config, default headers, etc.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use a2a_client::A2AClient;
    /// use a2a_types::AgentCard;
    /// use reqwest::Client;
    /// use std::time::Duration;
    ///
    /// # fn example(agent_card: AgentCard) -> Result<(), Box<dyn std::error::Error>> {
    /// let http_client = Client::builder()
    ///     .timeout(Duration::from_secs(30))
    ///     .default_headers({
    ///         let mut headers = reqwest::header::HeaderMap::new();
    ///         headers.insert("X-Custom-Header", "value".parse()?);
    ///         headers
    ///     })
    ///     .build()?;
    ///
    /// let client = A2AClient::from_card_with_client(agent_card, http_client)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_card_with_client(agent_card: AgentCard, http_client: Client) -> A2AResult<Self> {
        if agent_card.url.is_empty() {
            return Err(A2AError::InvalidParameter {
                message: "Agent card does not contain a valid 'url' for the service endpoint"
                    .to_string(),
            });
        }

        Ok(Self {
            client: http_client,
            service_endpoint_url: agent_card.url.clone(),
            auth_token: None,
            request_id_counter: Arc::new(AtomicU64::new(1)),
            agent_card: Arc::new(agent_card),
        })
    }

    /// Set authentication token (builder pattern)
    pub fn with_auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    /// Get the cached agent card
    pub fn agent_card(&self) -> &AgentCard {
        &self.agent_card
    }

    /// Fetch a fresh agent card from the base URL
    pub async fn fetch_agent_card(&self, base_url: impl AsRef<str>) -> A2AResult<AgentCard> {
        let base_url = base_url.as_ref().trim_end_matches('/');
        let card_url = format!("{}/{}", base_url, AGENT_CARD_PATH);

        let mut req = self
            .client
            .get(&card_url)
            .header("Accept", "application/json");

        if let Some(token) = &self.auth_token {
            req = req.bearer_auth(token);
        }

        let response = req.send().await.map_err(|e| A2AError::NetworkError {
            message: format!("Failed to fetch agent card from {}: {}", card_url, e),
        })?;

        if !response.status().is_success() {
            return Err(A2AError::NetworkError {
                message: format!("Failed to fetch agent card: HTTP {}", response.status()),
            });
        }

        serde_json::from_slice(&bounded_bytes(response).await?).map_err(|e| {
            A2AError::SerializationError {
                message: format!("Failed to parse agent card: {}", e),
            }
        })
    }

    /// Get the next request ID
    fn next_request_id(&self) -> JSONRPCId {
        let id = self.request_id_counter.fetch_add(1, Ordering::SeqCst);
        JSONRPCId::Integer(id as i64)
    }

    /// Helper method to make a generic JSON-RPC POST request
    async fn post_rpc_request<TParams, TResponse>(
        &self,
        method: &str,
        params: TParams,
    ) -> A2AResult<JsonRpcResponse<TResponse>>
    where
        TParams: Serialize,
        TResponse: for<'de> Deserialize<'de>,
    {
        let request_id = self.next_request_id();
        let rpc_request = JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.to_string(),
            params,
            id: request_id.clone(),
        };

        let mut req = self
            .client
            .post(&self.service_endpoint_url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&rpc_request);

        if let Some(token) = &self.auth_token {
            req = req.bearer_auth(token);
        }

        let response = req.send().await.map_err(|e| A2AError::NetworkError {
            message: format!("Failed to send {} request: {}", method, e),
        })?;

        if !response.status().is_success() {
            // Try to parse error response
            let status = response.status();
            let error_text = bounded_text(response).await?;
            if let Ok(error_json) = serde_json::from_str::<JSONRPCErrorResponse>(&error_text) {
                return Ok(JsonRpcResponse::Error(error_json));
            }
            return Err(A2AError::NetworkError {
                message: format!("HTTP error {}: {}", status, error_text),
            });
        }

        let json_response: JsonRpcResponse<TResponse> =
            serde_json::from_slice(&bounded_bytes(response).await?).map_err(|e| {
                A2AError::SerializationError {
                    message: format!("Failed to parse {} response: {}", method, e),
                }
            })?;

        // Validate response ID matches request ID
        if let JsonRpcResponse::Success {
            id: Some(resp_id), ..
        } = &json_response
        {
            if resp_id != &request_id {
                eprintln!(
                    "WARNING: RPC response ID mismatch for method {}. Expected {:?}, got {:?}",
                    method, request_id, resp_id
                );
            }
        }

        Ok(json_response)
    }

    /// Send a message to the remote agent (non-streaming)
    pub async fn send_message(&self, params: MessageSendParams) -> A2AResult<SendMessageResult> {
        match self.post_rpc_request("message/send", params).await? {
            JsonRpcResponse::Success { result, .. } => Ok(result),
            JsonRpcResponse::Error(err) => Err(A2AError::RemoteAgentError {
                message: format!("Remote agent error: {}", err.error.message),
                code: Some(err.error.code),
            }),
        }
    }

    /// Send a streaming message to the remote agent
    ///
    /// Returns a stream of events (Task, Message, TaskStatusUpdateEvent, TaskArtifactUpdateEvent)
    pub async fn send_streaming_message(
        &self,
        params: MessageSendParams,
    ) -> A2AResult<Pin<Box<dyn Stream<Item = A2AResult<SendStreamingMessageResult>> + Send>>> {
        // Check if agent supports streaming
        if !self.agent_card.capabilities.streaming.unwrap_or(false) {
            return Err(A2AError::InvalidParameter {
                message: "Agent does not support streaming (capabilities.streaming is not true)"
                    .to_string(),
            });
        }

        let request_id = self.next_request_id();
        let rpc_request = JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: "message/stream".to_string(),
            params,
            id: request_id.clone(),
        };

        let mut req = self
            .client
            .post(&self.service_endpoint_url)
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .json(&rpc_request);

        if let Some(token) = &self.auth_token {
            req = req.bearer_auth(token);
        }

        let response = req.send().await.map_err(|e| A2AError::NetworkError {
            message: format!("Failed to send streaming message request: {}", e),
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = bounded_text(response).await?;
            return Err(A2AError::NetworkError {
                message: format!("HTTP error {}: {}", status, error_text),
            });
        }

        // Verify content type
        let content_type = response
            .headers()
            .get("Content-Type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();

        if !content_type.starts_with("text/event-stream") {
            let body = bounded_text(response).await?;
            if let Ok(error) = serde_json::from_str::<JSONRPCErrorResponse>(&body) {
                return Err(A2AError::RemoteAgentError {
                    message: error.error.message,
                    code: Some(error.error.code),
                });
            }

            return Err(A2AError::NetworkError {
                message: format!(
                    "Invalid response Content-Type for SSE stream. Expected 'text/event-stream', got '{}'",
                    content_type
                ),
            });
        }

        // Parse SSE stream
        Ok(Box::pin(Self::parse_sse_stream(
            response.bytes_stream(),
            request_id,
        )))
    }

    /// Parse Server-Sent Events (SSE) stream
    fn parse_sse_stream(
        byte_stream: impl Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
        _original_request_id: JSONRPCId,
    ) -> impl Stream<Item = A2AResult<SendStreamingMessageResult>> + Send {
        use futures_core::stream::Stream;
        use std::pin::Pin;
        use std::task::{Context, Poll};

        struct SseParser {
            inner: Pin<Box<dyn Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send>>,
            received: usize,
            utf8_pending: Vec<u8>,
            buffer: String,
            event_data_buffer: String,
            pending_results: Vec<A2AResult<SendStreamingMessageResult>>,
        }

        impl SseParser {
            fn new(
                inner: impl Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
            ) -> Self {
                Self {
                    inner: Box::pin(inner),
                    received: 0,
                    utf8_pending: Vec::new(),
                    buffer: String::new(),
                    event_data_buffer: String::new(),
                    pending_results: Vec::new(),
                }
            }

            fn process_chunk(
                &mut self,
                chunk: bytes::Bytes,
            ) -> Vec<A2AResult<SendStreamingMessageResult>> {
                // Append chunk to buffer
                if self
                    .buffer
                    .len()
                    .saturating_add(self.event_data_buffer.len())
                    .saturating_add(chunk.len())
                    > 1024 * 1024
                {
                    return vec![Err(A2AError::SerializationError {
                        message: "A2A SSE frame exceeds 1 MiB".into(),
                    })];
                }
                self.received = self.received.saturating_add(chunk.len());
                if self.received > 8 * 1024 * 1024 {
                    return vec![Err(A2AError::SerializationError {
                        message: "A2A stream wire exceeds 8 MiB".into(),
                    })];
                }
                let decoded = match decode_utf8_chunk(&mut self.utf8_pending, &chunk) {
                    Ok(decoded) => decoded,
                    Err(error) => return vec![Err(error)],
                };
                self.buffer.push_str(&decoded);

                let mut results = Vec::new();

                // Process complete lines
                let mut consumed = 0;
                let mut lines = 0;
                while let Some(relative_pos) = self.buffer[consumed..].find('\n') {
                    lines += 1;
                    if lines > 4096 {
                        return vec![Err(A2AError::SerializationError {
                            message: "A2A SSE line work limit exceeded".into(),
                        })];
                    }
                    let newline_pos = consumed + relative_pos;
                    let line = self.buffer[consumed..newline_pos]
                        .trim_end_matches('\r')
                        .to_string();
                    consumed = newline_pos + 1;

                    if line.is_empty() {
                        // Empty line signals end of event
                        if !self.event_data_buffer.is_empty() {
                            match A2AClient::process_sse_event(&self.event_data_buffer) {
                                Ok(result) => results.push(Ok(result)),
                                Err(e) => results.push(Err(e)),
                            }
                            self.event_data_buffer.clear();
                        }
                    } else if let Some(data) = line.strip_prefix("data:") {
                        // Accumulate data lines
                        if !self.event_data_buffer.is_empty() {
                            self.event_data_buffer.push('\n');
                        }
                        self.event_data_buffer.push_str(data.trim_start());
                    } else if line.starts_with(':') {
                        // Comment line, ignore
                    }
                    // Ignore other SSE fields (event:, id:, retry:)
                }

                self.buffer.drain(..consumed);
                results
            }
        }

        impl Stream for SseParser {
            type Item = A2AResult<SendStreamingMessageResult>;

            fn poll_next(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<Option<Self::Item>> {
                // First, return any pending results
                if let Some(result) = self.pending_results.pop() {
                    return Poll::Ready(Some(result));
                }

                // Poll the inner stream for more data
                match self.inner.as_mut().poll_next(cx) {
                    Poll::Ready(Some(Ok(chunk))) => {
                        // Process the chunk and get results
                        let mut results = self.process_chunk(chunk);

                        if results.is_empty() {
                            // No complete events yet, wake up and try again
                            cx.waker().wake_by_ref();
                            Poll::Pending
                        } else {
                            // Store results in reverse order (we pop from the end)
                            results.reverse();
                            self.pending_results = results;

                            // Return first result
                            Poll::Ready(self.pending_results.pop())
                        }
                    }
                    Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(A2AError::NetworkError {
                        message: format!("Stream error: {}", e),
                    }))),
                    Poll::Ready(None) => Poll::Ready(None),
                    Poll::Pending => Poll::Pending,
                }
            }
        }

        SseParser::new(byte_stream)
    }

    /// Process a single SSE event's data
    fn process_sse_event(json_data: &str) -> A2AResult<SendStreamingMessageResult> {
        if json_data.trim().is_empty() {
            return Err(A2AError::SerializationError {
                message: "Empty SSE event data".to_string(),
            });
        }

        validate_json_budget(json_data.as_bytes())?;
        // Parse JSON-RPC response
        let json_response: JsonRpcResponse<SendStreamingMessageResult> =
            serde_json::from_str(json_data).map_err(|e| A2AError::SerializationError {
                message: format!("Failed to parse SSE event data: {}", e),
            })?;

        match json_response {
            JsonRpcResponse::Success { result, .. } => Ok(result),
            JsonRpcResponse::Error(err) => Err(A2AError::RemoteAgentError {
                message: format!("SSE event contained an error: {}", err.error.message),
                code: Some(err.error.code),
            }),
        }
    }

    /// Get a specific task from the remote agent
    pub async fn get_task(&self, params: TaskQueryParams) -> A2AResult<Task> {
        match self.post_rpc_request("tasks/get", params).await? {
            JsonRpcResponse::Success { result, .. } => Ok(result),
            JsonRpcResponse::Error(err) => Err(A2AError::RemoteAgentError {
                message: format!("Remote agent error: {}", err.error.message),
                code: Some(err.error.code),
            }),
        }
    }

    /// Cancel a task by its ID
    pub async fn cancel_task(&self, params: TaskIdParams) -> A2AResult<Task> {
        match self.post_rpc_request("tasks/cancel", params).await? {
            JsonRpcResponse::Success { result, .. } => Ok(result),
            JsonRpcResponse::Error(err) => Err(A2AError::RemoteAgentError {
                message: format!("Remote agent error: {}", err.error.message),
                code: Some(err.error.code),
            }),
        }
    }

    /// Resubscribe to a task's event stream
    ///
    /// This is used if a previous SSE connection for an active task was broken.
    pub async fn resubscribe_task(
        &self,
        params: TaskIdParams,
    ) -> A2AResult<Pin<Box<dyn Stream<Item = A2AResult<SendStreamingMessageResult>> + Send>>> {
        // Check if agent supports streaming
        if !self.agent_card.capabilities.streaming.unwrap_or(false) {
            return Err(A2AError::InvalidParameter {
                message: "Agent does not support streaming (required for tasks/resubscribe)"
                    .to_string(),
            });
        }

        let request_id = self.next_request_id();
        let rpc_request = JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: "tasks/resubscribe".to_string(),
            params,
            id: request_id.clone(),
        };

        let mut req = self
            .client
            .post(&self.service_endpoint_url)
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .json(&rpc_request);

        if let Some(token) = &self.auth_token {
            req = req.bearer_auth(token);
        }

        let response = req.send().await.map_err(|e| A2AError::NetworkError {
            message: format!("Failed to send resubscribe request: {}", e),
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = bounded_text(response).await?;
            return Err(A2AError::NetworkError {
                message: format!("HTTP error {}: {}", status, error_text),
            });
        }

        // Verify content type
        let content_type = response
            .headers()
            .get("Content-Type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();

        if !content_type.starts_with("text/event-stream") {
            let body = bounded_text(response).await?;
            if let Ok(error) = serde_json::from_str::<JSONRPCErrorResponse>(&body) {
                return Err(A2AError::RemoteAgentError {
                    message: error.error.message,
                    code: Some(error.error.code),
                });
            }

            return Err(A2AError::NetworkError {
                message: format!(
                    "Invalid response Content-Type for SSE stream on resubscribe. Expected 'text/event-stream', got '{}'",
                    content_type
                ),
            });
        }

        Ok(Box::pin(Self::parse_sse_stream(
            response.bytes_stream(),
            request_id,
        )))
    }

    /// Set or update the push notification configuration for a given task
    pub async fn set_task_push_notification_config(
        &self,
        params: TaskPushNotificationConfig,
    ) -> A2AResult<TaskPushNotificationConfig> {
        // Check if agent supports push notifications
        if !self
            .agent_card
            .capabilities
            .push_notifications
            .unwrap_or(false)
        {
            return Err(A2AError::InvalidParameter {
                message: "Agent does not support push notifications (capabilities.pushNotifications is not true)"
                    .to_string(),
            });
        }

        match self
            .post_rpc_request("tasks/pushNotificationConfig/set", params)
            .await?
        {
            JsonRpcResponse::Success { result, .. } => Ok(result),
            JsonRpcResponse::Error(err) => Err(A2AError::RemoteAgentError {
                message: format!("Remote agent error: {}", err.error.message),
                code: Some(err.error.code),
            }),
        }
    }

    /// Get the push notification configuration for a given task
    pub async fn get_task_push_notification_config(
        &self,
        params: TaskIdParams,
    ) -> A2AResult<TaskPushNotificationConfig> {
        match self
            .post_rpc_request("tasks/pushNotificationConfig/get", params)
            .await?
        {
            JsonRpcResponse::Success { result, .. } => Ok(result),
            JsonRpcResponse::Error(err) => Err(A2AError::RemoteAgentError {
                message: format!("Remote agent error: {}", err.error.message),
                code: Some(err.error.code),
            }),
        }
    }

    /// List push notification configurations for a given task
    pub async fn list_task_push_notification_config(
        &self,
        params: ListTaskPushNotificationConfigParams,
    ) -> A2AResult<Vec<TaskPushNotificationConfig>> {
        match self
            .post_rpc_request("tasks/pushNotificationConfig/list", params)
            .await?
        {
            JsonRpcResponse::Success { result, .. } => Ok(result),
            JsonRpcResponse::Error(err) => Err(A2AError::RemoteAgentError {
                message: format!("Remote agent error: {}", err.error.message),
                code: Some(err.error.code),
            }),
        }
    }

    /// Delete a push notification configuration for a given task
    pub async fn delete_task_push_notification_config(
        &self,
        params: DeleteTaskPushNotificationConfigParams,
    ) -> A2AResult<()> {
        match self
            .post_rpc_request::<_, serde_json::Value>("tasks/pushNotificationConfig/delete", params)
            .await?
        {
            JsonRpcResponse::Success { .. } => Ok(()),
            JsonRpcResponse::Error(err) => Err(A2AError::RemoteAgentError {
                message: format!("Remote agent error: {}", err.error.message),
                code: Some(err.error.code),
            }),
        }
    }

    /// Call a custom extension method
    ///
    /// This allows calling custom JSON-RPC methods defined by agent extensions.
    pub async fn call_extension_method<TParams, TResponse>(
        &self,
        method: &str,
        params: TParams,
    ) -> A2AResult<TResponse>
    where
        TParams: Serialize,
        TResponse: for<'de> Deserialize<'de>,
    {
        match self.post_rpc_request(method, params).await? {
            JsonRpcResponse::Success { result, .. } => Ok(result),
            JsonRpcResponse::Error(err) => Err(A2AError::RemoteAgentError {
                message: format!("Remote agent error: {}", err.error.message),
                code: Some(err.error.code),
            }),
        }
    }

    /// List tasks from the remote agent
    ///
    /// Note: This method is not part of the official A2A spec but is commonly implemented.
    pub async fn list_tasks(&self, context_id: Option<String>) -> A2AResult<Vec<Task>> {
        #[derive(Serialize)]
        struct ListTasksParams {
            #[serde(skip_serializing_if = "Option::is_none")]
            context_id: Option<String>,
        }

        match self
            .post_rpc_request("tasks/list", ListTasksParams { context_id })
            .await?
        {
            JsonRpcResponse::Success { result, .. } => Ok(result),
            JsonRpcResponse::Error(err) => Err(A2AError::RemoteAgentError {
                message: format!("Remote agent error: {}", err.error.message),
                code: Some(err.error.code),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_requires_valid_card_url() {
        let card_without_url = AgentCard {
            name: "Test".to_string(),
            description: "Test".to_string(),
            version: "1.0.0".to_string(),
            protocol_version: "0.3.0".to_string(),
            url: "".to_string(), // Empty URL
            preferred_transport: a2a_types::TransportProtocol::JsonRpc,
            capabilities: a2a_types::AgentCapabilities::default(),
            default_input_modes: vec![],
            default_output_modes: vec![],
            skills: vec![],
            provider: None,
            additional_interfaces: vec![],
            documentation_url: None,
            icon_url: None,
            security: vec![],
            security_schemes: None,
            signatures: vec![],
            supports_authenticated_extended_card: None,
        };

        assert!(A2AClient::from_card(card_without_url).is_err());
    }
}

async fn bounded_bytes(mut response: reqwest::Response) -> A2AResult<Vec<u8>> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if bytes.len().saturating_add(chunk.len()) > 1024 * 1024 {
            return Err(A2AError::SerializationError {
                message: "A2A response exceeds 1 MiB".into(),
            });
        }
        bytes.extend_from_slice(&chunk);
    }
    validate_json_budget(&bytes)?;
    Ok(bytes)
}
async fn bounded_text(response: reqwest::Response) -> A2AResult<String> {
    Ok(String::from_utf8_lossy(&bounded_bytes(response).await?).into_owned())
}

fn validate_json_budget(bytes: &[u8]) -> A2AResult<()> {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return Ok(());
    };
    let mut stack = vec![(&value, 0usize)];
    let mut count = 0;
    while let Some((value, depth)) = stack.pop() {
        count += 1;
        if count > 10000 || depth > 32 {
            return Err(A2AError::SerializationError {
                message: "A2A JSON structure exceeds limits".into(),
            });
        }
        match value {
            serde_json::Value::Object(items) => {
                stack.extend(items.values().map(|value| (value, depth + 1)))
            }
            serde_json::Value::Array(items) => {
                stack.extend(items.iter().map(|value| (value, depth + 1)))
            }
            _ => {}
        }
    }
    Ok(())
}

fn decode_utf8_chunk(pending: &mut Vec<u8>, chunk: &[u8]) -> A2AResult<String> {
    pending.extend_from_slice(chunk);
    let length = match std::str::from_utf8(pending) {
        Ok(_) => pending.len(),
        Err(error) if error.error_len().is_none() => error.valid_up_to(),
        Err(_) => {
            return Err(A2AError::SerializationError {
                message: "A2A SSE is not UTF-8".into(),
            });
        }
    };
    let decoded = std::str::from_utf8(&pending[..length])
        .expect("validated UTF-8")
        .to_owned();
    pending.drain(..length);
    Ok(decoded)
}
