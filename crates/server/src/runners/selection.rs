use crate::ApiError;
use moleapi_core::{RunnerSelection, Workspace};
pub(super) fn validate(workspace: &Workspace, selection: &RunnerSelection) -> Result<(), ApiError> {
    let collection = workspace
        .data
        .collections
        .iter()
        .find(|c| c.id == selection.collection_id)
        .ok_or_else(ApiError::not_found)?;
    let subtree = moleapi_core::collection_subtree(&workspace.data, collection)
        .map_err(|e| ApiError::bad(e.to_string()))?;
    let mut executions = subtree.iter().map(|c| c.requests.len()).sum::<usize>();
    if let Some(id) = &selection.scenario_id {
        if selection.request_ids.is_some() {
            return Err(ApiError::bad("Scenario tasks cannot filter request IDs"));
        }
        let scenario = workspace
            .data
            .scenarios
            .iter()
            .find(|s| s.id == *id && s.collection_id == collection.id)
            .ok_or_else(ApiError::not_found)?;
        let plan = moleapi_core::scenario_plan(&workspace.data, scenario)
            .map_err(|e| ApiError::bad(e.to_string()))?;
        executions = plan.iter().map(|(_, _, step)| step.repeat).sum();
    }
    if let Some(ids) = &selection.request_ids {
        let unique = ids.iter().collect::<std::collections::BTreeSet<_>>();
        if ids.is_empty()
            || ids.len() > 1000
            || ids.len() != unique.len()
            || unique.iter().any(|id| {
                !subtree
                    .iter()
                    .any(|c| c.requests.iter().any(|r| r.id.as_str() == id.as_str()))
            })
        {
            return Err(ApiError::bad(
                "Request filters must be distinct IDs within the selected subtree",
            ));
        }
        executions = ids.len();
    }
    crate::execution::environment(workspace, selection.environment_id.as_deref())?;
    let dataset = selection
        .dataset_id
        .as_ref()
        .map(|id| {
            workspace
                .data
                .datasets
                .iter()
                .find(|d| d.id == *id)
                .and_then(|d| d.source.as_ref())
                .ok_or_else(ApiError::not_found)?
                .parse()
                .map_err(|e| ApiError::bad(e.to_string()))
        })
        .transpose()?;
    let iterations = selection
        .iterations
        .unwrap_or_else(|| dataset.as_ref().map_or(1, |d| d.rows.len()));
    if !(1..=100).contains(&iterations)
        || dataset.as_ref().is_some_and(|d| iterations > d.rows.len())
        || executions.saturating_mul(iterations) > 1000
    {
        return Err(ApiError::bad("Task exceeds iteration or execution limits"));
    }
    Ok(())
}
