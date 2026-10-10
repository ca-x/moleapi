use crate::Workspace;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunnerSelection {
    pub collection_id: String,
    pub scenario_id: Option<String>,
    pub environment_id: Option<String>,
    pub dataset_id: Option<String>,
    pub iterations: Option<usize>,
    pub request_ids: Option<Vec<String>>,
    #[serde(default)]
    pub notification_ids: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionRunner {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub revision: i64,
    pub last_seen_at: Option<String>,
    pub active_task_id: Option<String>,
    pub created_at: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunnerTaskStatus {
    Queued,
    Leased,
    Completed,
    Failed,
    Cancelled,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunnerTaskSummary {
    pub id: String,
    pub workspace_id: String,
    pub runner_id: String,
    pub selection: RunnerSelection,
    pub source_revision: i64,
    pub status: RunnerTaskStatus,
    pub attempt: usize,
    pub max_attempts: usize,
    pub queued_at: String,
    pub started_at: Option<String>,
    pub lease_until: Option<String>,
    pub finished_at: Option<String>,
    pub report_id: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunnerClaim {
    pub task: RunnerTaskSummary,
    pub lease_token: String,
    pub workspace: Workspace,
}
