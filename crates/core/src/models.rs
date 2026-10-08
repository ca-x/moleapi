use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Pair {
    pub id: String,
    pub key: String,
    pub value: String,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_value: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Auth {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hawk: Option<Box<crate::HawkAuth>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aws: Option<Box<crate::AwsAuth>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oauth2: Option<Box<crate::OAuth2Auth>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<Box<crate::ApiKeyAuth>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jwt: Option<Box<crate::JwtAuth>>,
    pub kind: String,
    pub token: String,
    pub username: String,
    pub password: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Assertion {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub target: String,
    pub expected: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Example {
    pub id: String,
    pub name: String,
    pub status: u16,
    pub headers: Vec<Pair>,
    pub body: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Protocol {
    #[default]
    Http,
    Data {
        #[serde(flatten)]
        config: Box<crate::DataConfig>,
    },
    Tcp {
        #[serde(flatten)]
        config: Box<crate::TcpConfig>,
    },
    Sse,
    Websocket,
    Socketio {
        #[serde(default = "socketio_namespace")]
        namespace: String,
        #[serde(default = "socketio_path")]
        path: String,
        #[serde(default = "empty_message_source")]
        auth_source: String,
        #[serde(default)]
        listeners: Vec<String>,
        #[serde(default = "socketio_event")]
        event: String,
        #[serde(default = "socketio_arguments")]
        arguments_source: String,
        #[serde(default)]
        attachments_base64: Vec<String>,
        #[serde(default)]
        request_ack: bool,
        #[serde(default = "socketio_ack_timeout")]
        ack_timeout_ms: u64,
    },
    Mqtt {
        #[serde(flatten)]
        config: Box<crate::MqttConfig>,
    },
    A2a {
        #[serde(flatten)]
        config: Box<crate::A2aConfig>,
    },
    Mcp {
        #[serde(flatten)]
        config: Box<crate::McpConfig>,
    },
    Soap {
        #[serde(flatten)]
        config: Box<crate::SoapConfig>,
    },
    Grpc {
        #[serde(default)]
        service: String,
        #[serde(default)]
        method: String,
        #[serde(default = "empty_message_source")]
        message_source: String,
    },
    Graphql {
        #[serde(default)]
        document: String,
        #[serde(default = "empty_boxed_object")]
        variables: Box<serde_json::Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        variables_source: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        operation_name: Option<String>,
        #[serde(default = "empty_object")]
        connection_params: serde_json::Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subscription_url: Option<String>,
    },
}
fn socketio_event() -> String {
    "message".into()
}
fn socketio_arguments() -> String {
    "[]".into()
}
fn socketio_ack_timeout() -> u64 {
    5000
}
fn socketio_namespace() -> String {
    "/".into()
}
fn socketio_path() -> String {
    "/socket.io/".into()
}
fn empty_message_source() -> String {
    "{}".into()
}
fn empty_boxed_object() -> Box<serde_json::Value> {
    Box::new(empty_object())
}
fn empty_object() -> serde_json::Value {
    serde_json::json!({})
}
impl Protocol {
    pub fn is_data(&self) -> bool {
        matches!(self, Self::Data { .. })
    }

    pub fn is_tcp(&self) -> bool {
        matches!(self, Self::Tcp { .. })
    }

    pub fn is_a2a(&self) -> bool {
        matches!(self, Self::A2a { .. })
    }
    pub fn is_mcp(&self) -> bool {
        matches!(self, Self::Mcp { .. })
    }
    pub fn is_soap(&self) -> bool {
        matches!(self, Self::Soap { .. })
    }
    pub fn is_mqtt(&self) -> bool {
        matches!(self, Self::Mqtt { .. })
    }
    pub fn is_socketio(&self) -> bool {
        matches!(self, Self::Socketio { .. })
    }
    pub fn is_grpc(&self) -> bool {
        matches!(self, Self::Grpc { .. })
    }
    pub fn is_graphql(&self) -> bool {
        matches!(self, Self::Graphql { .. })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RequestSpec {
    #[serde(default)]
    pub protocol: Protocol,
    #[serde(default)]
    pub pre_request_script: String,
    #[serde(default)]
    pub post_response_script: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specification_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    pub id: String,
    pub name: String,
    pub method: String,
    pub url: String,
    pub description: String,
    pub query: Vec<Pair>,
    pub headers: Vec<Pair>,
    pub body_kind: String,
    pub body: String,
    pub auth: Auth,
    pub timeout_ms: u64,
    pub follow_redirects: bool,
    pub verify_tls: bool,
    pub assertions: Vec<Assertion>,
    pub examples: Vec<Example>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Collection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variables_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<Auth>,
    #[serde(default)]
    pub variables: Vec<Pair>,
    #[serde(default)]
    pub pre_request_script: String,
    #[serde(default)]
    pub post_response_script: String,
    pub id: String,
    pub name: String,
    pub description: String,
    pub requests: Vec<RequestSpec>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Environment {
    pub id: String,
    pub name: String,
    pub variables: Vec<Pair>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<Auth>,
    #[serde(default)]
    pub global_variables: Vec<Pair>,
    #[serde(default)]
    pub pre_request_script: String,
    #[serde(default)]
    pub post_response_script: String,
    #[serde(default)]
    pub specifications: Vec<Specification>,
    pub schema_version: u32,
    pub collections: Vec<Collection>,
    pub environments: Vec<Environment>,
    pub active_environment_id: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub revision: i64,
    pub updated_at: String,
    pub data: WorkspaceData,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TestResult {
    pub id: String,
    pub name: String,
    pub passed: bool,
    pub actual: String,
    pub expected: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Response {
    #[serde(skip)]
    pub private_auth_values: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub soap_fault: Option<crate::SoapFault>,
    #[serde(default)]
    pub request_updates: Vec<RequestUpdate>,
    #[serde(default)]
    pub logs: Vec<ScriptLog>,
    #[serde(default)]
    pub variable_updates: Vec<VariableUpdate>,
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<Pair>,
    pub body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_base64: Option<String>,
    pub elapsed_ms: u64,
    pub size_bytes: usize,
    pub truncated: bool,
    pub url: String,
    pub tests: Vec<TestResult>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub workspace_id: String,
    pub request_id: String,
    pub request_name: String,
    pub method: String,
    pub url: String,
    pub status: u16,
    pub elapsed_ms: u64,
    pub size_bytes: usize,
    pub created_at: String,
    pub response: Response,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Specification {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub source: String,
    pub dialect: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ScriptLog {
    pub level: String,
    pub message: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct VariableUpdate {
    pub scope: String,
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RequestUpdate {
    pub field: String,
    pub value: String,
}
