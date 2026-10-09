use crate::TestResult;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RunOutcome {
    Passed,
    Failed,
    Skipped,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunSummary {
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub elapsed_ms: u64,
    pub iteration_count: usize,
    pub completed_iterations: usize,
    pub executed_steps: usize,
    pub tests_passed: usize,
    pub tests_failed: usize,
    pub diagnostics_omitted: usize,
    pub stopped_reason: Option<String>,
    pub cancelled: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunIterationReport {
    pub iteration: usize,
    pub passed: usize,
    pub failed: usize,
    pub elapsed_ms: u64,
    pub script_stopped: bool,
    pub scenario_stopped: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunStepReport {
    pub position: usize,
    pub request_id: String,
    pub request_name: String,
    pub collection_id: String,
    pub method: String,
    pub iteration: usize,
    pub step_id: Option<String>,
    pub step_name: Option<String>,
    pub step_group: Option<String>,
    pub parallel_id: Option<String>,
    pub parallel_name: Option<String>,
    pub repeat_index: usize,
    pub outcome: RunOutcome,
    pub status: Option<u16>,
    pub elapsed_ms: u64,
    pub size_bytes: usize,
    pub tests: Vec<TestResult>,
    pub tests_passed: usize,
    pub tests_failed: usize,
    pub diagnostics_omitted: usize,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedRunReport {
    pub schema_version: u32,
    pub id: String,
    pub workspace_id: String,
    pub workspace_name: String,
    pub workspace_revision: i64,
    pub collection_id: String,
    pub collection_name: String,
    pub scenario_id: Option<String>,
    pub scenario_name: Option<String>,
    pub environment_id: Option<String>,
    pub environment_name: Option<String>,
    pub dataset_id: Option<String>,
    pub started_at: String,
    pub finished_at: String,
    pub summary: RunSummary,
    pub iterations: Vec<RunIterationReport>,
    pub results: Vec<RunStepReport>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunReportSummary {
    pub id: String,
    pub workspace_revision: i64,
    pub collection_name: String,
    pub scenario_name: Option<String>,
    pub environment_name: Option<String>,
    pub started_at: String,
    pub finished_at: String,
    pub summary: RunSummary,
}
impl SavedRunReport {
    pub fn brief(&self) -> RunReportSummary {
        RunReportSummary {
            id: self.id.clone(),
            workspace_revision: self.workspace_revision,
            collection_name: self.collection_name.clone(),
            scenario_name: self.scenario_name.clone(),
            environment_name: self.environment_name.clone(),
            started_at: self.started_at.clone(),
            finished_at: self.finished_at.clone(),
            summary: self.summary.clone(),
        }
    }
}
