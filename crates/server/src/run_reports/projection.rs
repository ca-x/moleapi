use crate::{ApiError, privacy::Redactor, storage};
use moleapi_core::{
    Collection, Environment, RunIterationReport, RunOutcome, RunStepReport, RunSummary,
    SavedRunReport, Scenario, TestResult, VariableScopes, Workspace,
};
use serde_json::{Value, json};
pub(crate) struct Source<'a> {
    pub workspace: &'a Workspace,
    pub collection: &'a Collection,
    pub scenario: Option<&'a Scenario>,
    pub environment: Option<&'a Environment>,
    pub dataset: Option<&'a str>,
    pub started_at: &'a str,
    pub scopes: &'a VariableScopes,
}
fn number(value: &Value, key: &str) -> usize {
    value[key].as_u64().unwrap_or(0) as usize
}
fn clipped(value: String, limit: usize) -> String {
    if value.len() <= limit {
        return value;
    }
    let mut end = limit.saturating_sub(3).min(value.len());
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &value[..end])
}
fn text(
    redactor: &Redactor,
    known: &moleapi_formats::RunReportPrivacy,
    value: &str,
    limit: usize,
    withhold: bool,
) -> String {
    if withhold {
        return "[WITHHELD: report privacy]".into();
    }
    let screened = known.screen(value, limit);
    let screened = if !value.is_empty() && screened.is_empty() {
        "[WITHHELD: report privacy or field limit]".into()
    } else {
        screened
    };
    let mut value = json!(screened);
    redactor.scrub(&mut value);
    clipped(value.as_str().unwrap_or("").into(), limit)
}
pub(super) fn project(source: &Source<'_>, live: &Value) -> Result<SavedRunReport, ApiError> {
    let redactor = Redactor::new(&source.scopes.private_values)?;
    let known = moleapi_formats::RunReportPrivacy::new(source.workspace);
    let withhold = false;
    let screen = |value: &str, limit| text(&redactor, &known, value, limit, withhold);
    let mut results = vec![];
    let (mut budget, mut passed_tests, mut failed_tests, mut omitted) =
        (0usize, 0usize, 0usize, 0usize);
    for (position, item) in live["results"]
        .as_array()
        .ok_or_else(ApiError::internal)?
        .iter()
        .take(1000)
        .enumerate()
    {
        let source_tests = item["response"]["tests"].as_array();
        let count = number(item, "tests_passed") + number(item, "tests_failed");
        passed_tests += number(item, "tests_passed");
        failed_tests += number(item, "tests_failed");
        let uncertain = item.get("error").is_some()
            || source_tests
                .is_some_and(|tests| tests.iter().any(|test| test["id"] == "post-script-error"));
        let mut tests = vec![];
        if let Some(source_tests) = source_tests {
            for test in source_tests
                .iter()
                .filter(|test| test["passed"] == false)
                .chain(source_tests.iter().filter(|test| test["passed"] == true))
            {
                if tests.len() >= 100 {
                    break;
                }
                let test: TestResult =
                    serde_json::from_value(test.clone()).map_err(|_| ApiError::internal())?;
                let projected = TestResult {
                    id: screen(&test.id, 128),
                    name: text(&redactor, &known, &test.name, 256, withhold || uncertain),
                    passed: test.passed,
                    actual: text(&redactor, &known, &test.actual, 512, withhold || uncertain),
                    expected: text(
                        &redactor,
                        &known,
                        &test.expected,
                        512,
                        withhold || uncertain,
                    ),
                };
                let size = serde_json::to_vec(&projected)
                    .map_err(|_| ApiError::internal())?
                    .len();
                if budget.saturating_add(size) > 1024 * 1024 {
                    break;
                }
                budget += size;
                tests.push(projected);
            }
        }
        let diagnostics_omitted = count.saturating_sub(tests.len());
        omitted += diagnostics_omitted;
        let request_id = item["request_id"].as_str().unwrap_or("");
        let request = source
            .workspace
            .data
            .collections
            .iter()
            .flat_map(|collection| &collection.requests)
            .find(|request| request.id == request_id);
        let optional = |key: &str, limit| item[key].as_str().map(|value| screen(value, limit));
        results.push(RunStepReport {
            history_id: item["history_id"].as_str().map(str::to_string),
            position,
            request_id: screen(request_id, 128),
            request_name: screen(item["request_name"].as_str().unwrap_or(""), 256),
            collection_id: screen(
                item["collection_id"]
                    .as_str()
                    .unwrap_or(&source.collection.id),
                128,
            ),
            method: screen(
                item["method"]
                    .as_str()
                    .unwrap_or_else(|| request.map_or("", |request| request.method.as_str())),
                48,
            ),
            iteration: number(item, "iteration"),
            step_id: optional("step_id", 128),
            step_name: optional("step_name", 256),
            step_group: optional("step_group", 256),
            parallel_id: optional("parallel_id", 128),
            parallel_name: optional("parallel_name", 256),
            repeat_index: number(item, "step_repeat_index"),
            outcome: match item["outcome"].as_str() {
                Some("skipped") => RunOutcome::Skipped,
                Some("failed") => RunOutcome::Failed,
                _ => RunOutcome::Passed,
            },
            status: item["status"]
                .as_u64()
                .and_then(|status| u16::try_from(status).ok()),
            elapsed_ms: item["elapsed_ms"].as_u64().unwrap_or(0),
            size_bytes: number(item, "size_bytes"),
            tests,
            tests_passed: number(item, "tests_passed"),
            tests_failed: number(item, "tests_failed"),
            diagnostics_omitted,
            error: item
                .get("error")
                .map(|_| "Request execution failed; inspect its live result".into()),
        });
    }
    let iterations = live["iterations"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .take(100)
                .map(|item| RunIterationReport {
                    iteration: number(item, "iteration"),
                    passed: number(item, "passed"),
                    failed: number(item, "failed"),
                    elapsed_ms: item["elapsed_ms"].as_u64().unwrap_or(0),
                    script_stopped: item["script_stopped"].as_bool().unwrap_or(false),
                    scenario_stopped: item["scenario_stopped"].as_bool().unwrap_or(false),
                })
                .collect()
        })
        .unwrap_or_default();
    let report = SavedRunReport {
        schema_version: 1,
        id: uuid::Uuid::new_v4().to_string(),
        workspace_id: source.workspace.id.clone(),
        workspace_name: screen(&source.workspace.name, 256),
        workspace_revision: source.workspace.revision,
        collection_id: screen(&source.collection.id, 128),
        collection_name: screen(&source.collection.name, 256),
        scenario_id: source.scenario.map(|scenario| screen(&scenario.id, 128)),
        scenario_name: source.scenario.map(|scenario| screen(&scenario.name, 256)),
        environment_id: source
            .environment
            .map(|environment| screen(&environment.id, 128)),
        environment_name: source
            .environment
            .map(|environment| screen(&environment.name, 256)),
        dataset_id: source.dataset.map(|id| screen(id, 128)),
        started_at: source.started_at.into(),
        finished_at: storage::now(),
        summary: RunSummary {
            passed: number(live, "passed"),
            failed: number(live, "failed"),
            skipped: number(live, "skipped"),
            elapsed_ms: live["elapsed_ms"].as_u64().unwrap_or(0),
            iteration_count: number(live, "iteration_count"),
            completed_iterations: number(live, "completed_iterations"),
            executed_steps: number(live, "executed_steps"),
            tests_passed: passed_tests,
            tests_failed: failed_tests,
            diagnostics_omitted: omitted,
            stopped_reason: live["stopped_reason"].as_str().map(str::to_string),
            cancelled: live["cancelled"].as_bool().unwrap_or(false),
        },
        iterations,
        results,
    };
    if serde_json::to_vec(&report)
        .map_err(|_| ApiError::internal())?
        .len()
        > 4 * 1024 * 1024
    {
        return Err(ApiError::bad("Saved report exceeds 4 MiB"));
    }
    Ok(report)
}
