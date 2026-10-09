use crate::{ApiError, AppState};
use moleapi_core::{RequestSpec, Response, ScenarioStep, ScenarioTarget, VariableScopes};
pub(super) async fn condition(
    state: &AppState,
    expression: &str,
    request: &RequestSpec,
    response: Option<&Response>,
    scopes: &mut VariableScopes,
) -> Result<bool, ()> {
    let _permit = state
        .script_slots
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| ())?;
    match moleapi_script_runtime::condition_worker(
        &state.script_worker,
        expression,
        request,
        response,
        scopes,
    )
    .await
    {
        Ok(output) => {
            scopes.private_values.extend(output.private_values);
            output.condition_result.ok_or(())
        }
        Err(failure) => {
            scopes.private_values.extend(failure.private_values);
            Err(())
        }
    }
}
pub(super) fn next(
    target: Option<&ScenarioTarget>,
    current: usize,
    steps: &[Option<&ScenarioStep>],
) -> Result<usize, ApiError> {
    match target {
        None => Ok(current + 1),
        Some(ScenarioTarget::Stop) => Ok(steps.len()),
        Some(ScenarioTarget::Step { step_id }) => steps
            .iter()
            .position(|step| step.is_some_and(|step| step.id == *step_id))
            .ok_or_else(|| ApiError::bad("Scenario branch target is unavailable")),
    }
}
