use moleapi_core::{Pair, RequestUpdate, ScriptLog, TestResult, VariableUpdate};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionState {
    Connecting,
    Open,
    Closed,
    Error,
}
impl SessionState {
    pub fn live(self) -> bool {
        matches!(self, Self::Connecting | Self::Open)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Handshake {
    pub status: u16,
    pub headers: Vec<Pair>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: String,
    pub workspace_id: String,
    pub request_id: String,
    pub url: String,
    pub protocol: String,
    pub state: SessionState,
    pub reason: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub received_bytes: u64,
    pub sent_bytes: u64,
    pub event_count: u64,
    pub handshake: Option<Handshake>,
    #[serde(default)]
    pub client_half_closed: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variable_updates: Vec<VariableUpdate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub request_updates: Vec<RequestUpdate>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventMessage {
    SocketioEvent {
        event: String,
        arguments: Vec<serde_json::Value>,
        attachments_base64: Vec<String>,
        ack_id: Option<String>,
    },
    SocketioAck {
        ack_id: String,
        status: String,
        arguments: Vec<serde_json::Value>,
        attachments_base64: Vec<String>,
        error: Option<String>,
    },
    GrpcMessage {
        message: serde_json::Value,
    },
    GrpcMetadata {
        phase: String,
        metadata: Vec<Pair>,
    },
    GrpcStatus {
        code: u32,
        name: String,
        message: String,
        details_base64: String,
        metadata: Vec<Pair>,
    },
    Sse {
        event: String,
        data: String,
        id: String,
        retry: Option<u64>,
    },
    GraphqlNext {
        operation_id: String,
        payload: serde_json::Value,
    },
    GraphqlError {
        operation_id: String,
        payload: serde_json::Value,
    },
    GraphqlComplete {
        operation_id: String,
        payload: serde_json::Value,
    },
    Text {
        text: String,
    },
    Binary {
        base64: String,
    },
    Ping {
        base64: String,
    },
    Pong {
        base64: String,
    },
    Close {
        code: Option<u16>,
        reason: String,
    },
    State {
        state: SessionState,
        reason: Option<String>,
    },
    ScriptLog {
        level: String,
        message: String,
    },
    ScriptTest {
        test: TestResult,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionEvent {
    pub cursor: u64,
    pub received_at: String,
    pub direction: String,
    pub message: EventMessage,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventBatch {
    pub events: Vec<SessionEvent>,
    pub next_cursor: u64,
    pub earliest_cursor: u64,
    pub dropped_count: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SendMessage {
    SocketioEmit {
        event: String,
        arguments_source: String,
        #[serde(default)]
        attachments_base64: Vec<String>,
        #[serde(default)]
        ack_id: Option<String>,
        #[serde(default = "ack_timeout")]
        ack_timeout_ms: u64,
    },
    SocketioListen {
        event: String,
        enabled: bool,
    },
    SocketioAck {
        ack_id: String,
        arguments_source: String,
        #[serde(default)]
        attachments_base64: Vec<String>,
    },
    GrpcMessage {
        message_source: String,
    },
    GrpcHalfClose,
    Text {
        text: String,
    },
    Binary {
        base64: String,
    },
    Ping {
        base64: String,
    },
}
#[derive(Default)]
pub struct PreparedFeedback {
    pub logs: Vec<ScriptLog>,
    pub tests: Vec<TestResult>,
}

fn ack_timeout() -> u64 {
    5000
}
