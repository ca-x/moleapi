use moleapi_core::Pair;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Reply {
    pub status: u16,
    pub headers: Vec<Pair>,
    pub body: String,
}
impl Default for Reply {
    fn default() -> Self {
        Self {
            status: 200,
            headers: vec![],
            body: "OK".into(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Inbox {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub token: String,
    pub active: bool,
    pub response: Reply,
    pub config_epoch: u64,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
    pub received: u64,
    pub dropped: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Header {
    pub name: String,
    pub value_base64: String,
    pub value_text: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Capture {
    pub id: String,
    pub inbox_id: String,
    pub workspace_id: String,
    pub cursor: u64,
    pub received_at: String,
    pub method: String,
    pub query: String,
    pub headers: Vec<Header>,
    pub body_base64: String,
    pub body_bytes: usize,
    pub response_status: u16,
}
#[derive(Serialize)]
pub struct InboxView {
    #[serde(flatten)]
    pub inbox: Inbox,
    pub receiver_path: String,
}
impl From<Inbox> for InboxView {
    fn from(inbox: Inbox) -> Self {
        let receiver_path = format!("/hooks/{}", inbox.token);
        Self {
            inbox,
            receiver_path,
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Create {
    pub name: String,
    #[serde(default)]
    pub response: Reply,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Update {
    pub name: String,
    pub active: bool,
    pub response: Reply,
    pub expected_revision: i64,
}
#[derive(Deserialize)]
pub struct Revision {
    pub expected_revision: i64,
}
#[derive(Deserialize, Default)]
pub struct Inspect {
    #[serde(default)]
    pub include_secrets: bool,
    #[serde(default)]
    pub after: u64,
    #[serde(default)]
    pub search: String,
}
