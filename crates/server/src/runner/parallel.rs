use super::{
    PlanStep, report, scenario_control,
    scopes::{self, Overlays},
};
use crate::{
    ApiError, AppState,
    execution::{DeferredExecution, perform_deferred},
    workspaces::owned,
};
use futures_util::{StreamExt, stream};
use moleapi_core::{
    Environment, Pair, Response, ScenarioParallel, VariableScopes, VariableUpdate, Workspace,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use tokio_util::sync::CancellationToken;
pub(super) type Writes = BTreeMap<(String, String, String), Option<String>>;
pub(super) struct Context<'a> {
    pub state: &'a AppState,
    pub owner: &'a str,
    pub workspace: &'a Workspace,
    pub environment: Option<&'a Environment>,
    pub data: &'a [Pair],
    pub temporary: &'a [Pair],
    pub gate: &'a tokio::sync::Mutex<u64>,
    pub epoch: u64,
    pub cancel: &'a CancellationToken,
    pub deadline: tokio::time::Instant,
    pub iteration: usize,
}
pub(super) struct Batch {
    pub items: Vec<Value>,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub stopped: Option<&'static str>,
    pub writes: Writes,
}
struct Branch<'a> {
    step: PlanStep<'a>,
    initial: VariableScopes,
    scopes: VariableScopes,
    response: Option<Response>,
    repeat: usize,
    active: bool,
    writes: Writes,
}
struct Visit {
    outcome: Option<DeferredExecution>,
    error: Option<String>,
    condition_skipped: bool,
    stopped: Option<&'static str>,
}
async fn admitted(context: &Context<'_>) -> bool {
    let admission = context.gate.lock().await;
    *admission == context.epoch
        && owned(context.state, context.owner, &context.workspace.id)
            .await
            .is_ok()
}
async fn visit(context: &Context<'_>, branch: &mut Branch<'_>) -> Visit {
    let (collection, request, step) = branch.step;
    let mut result = Visit {
        outcome: None,
        error: None,
        condition_skipped: false,
        stopped: None,
    };
    if !admitted(context).await {
        result.stopped = Some("owner_changed");
        return result;
    }
    if let Some(expression) = step.and_then(|step| step.condition.as_deref()) {
        match scenario_control::condition(
            context.state,
            expression,
            request,
            branch.response.as_ref(),
            &mut branch.scopes,
        )
        .await
        {
            Ok(true) => {}
            Ok(false) => {
                result.condition_skipped = true;
                branch.active = false;
                return result;
            }
            Err(()) => {
                result.error = Some(
                    "Scenario condition failed; check its boolean result and execution limits"
                        .into(),
                );
                result.stopped = Some("condition_error");
                return result;
            }
        }
        if !admitted(context).await {
            result.stopped = Some("owner_changed");
            return result;
        }
    }
    match perform_deferred(
        context.state,
        context.owner,
        context.workspace,
        request,
        Some(collection),
        &mut branch.scopes,
    )
    .await
    {
        Ok(outcome) => {
            for update in &outcome.response.variable_updates {
                branch
                    .writes
                    .insert(write_key(update, &collection.id), update.value.clone());
            }
            if !outcome.response.skipped {
                branch.response = Some(outcome.response.clone());
            }
            if branch.scopes.execution.next_request.is_some() {
                result.error = Some("Parallel branch cannot redirect shared flow".into());
                result.stopped = Some("parallel_control");
            }
            result.outcome = Some(outcome);
        }
        Err(error) => {
            branch.response = None;
            result.error = Some(error.message);
        }
    }
    result
}
fn write_key(update: &VariableUpdate, collection: &str) -> (String, String, String) {
    (
        update.scope.clone(),
        if update.scope == "collection" {
            collection.into()
        } else {
            String::new()
        },
        update.key.clone(),
    )
}
fn final_changes(branch: &Branch<'_>) -> Writes {
    let mut writes = branch.writes.clone();
    for (scope, before, after) in [
        ("project", &branch.initial.project, &branch.scopes.project),
        (
            "environment",
            &branch.initial.environment,
            &branch.scopes.environment,
        ),
        (
            "temporary",
            &branch.initial.temporary,
            &branch.scopes.temporary,
        ),
        (
            "collection",
            &branch.initial.collection,
            &branch.scopes.collection,
        ),
    ] {
        for key in before.keys().chain(after.keys()).collect::<BTreeSet<_>>() {
            if before.get(key) != after.get(key) {
                writes.insert(
                    (
                        scope.into(),
                        if scope == "collection" {
                            branch.step.0.id.clone()
                        } else {
                            String::new()
                        },
                        key.clone(),
                    ),
                    after.get(key).cloned(),
                );
            }
        }
    }
    writes
}
pub(super) async fn run(
    context: &Context<'_>,
    block: &ScenarioParallel,
    members: &[PlanStep<'_>],
    shared: &mut VariableScopes,
    overlays: &mut Overlays,
    previous: Option<&Response>,
    executed: &mut usize,
) -> Result<Batch, ApiError> {
    let mut branches = Vec::new();
    for &step in members {
        let mut fork = shared.clone();
        scopes::prepare(
            &scopes::Inputs {
                state: context.state,
                workspace: context.workspace,
                environment: context.environment,
                data: context.data,
                temporary: context.temporary,
                overlays,
            },
            step.0,
            &mut fork,
        )?;
        branches.push(Branch {
            step,
            initial: fork.clone(),
            scopes: fork,
            response: previous.cloned(),
            repeat: 0,
            active: true,
            writes: Writes::new(),
        });
    }
    let mut batch = Batch {
        items: vec![],
        passed: 0,
        failed: 0,
        skipped: 0,
        stopped: None,
        writes: Writes::new(),
    };
    let (mut bytes, mut omitted) = (0, 0);
    while branches.iter().any(|branch| branch.active) {
        let active = branches.iter().filter(|branch| branch.active).count();
        if executed.saturating_add(active) > 1000 {
            batch.stopped = Some("step_limit");
            break;
        }
        *executed += active;
        let mut work = vec![];
        for branch in branches.iter_mut() {
            if branch.active {
                work.push(async move {
                    let result = visit(context, branch).await;
                    (branch, result)
                });
            }
        }
        let visits = stream::iter(work)
            .buffered(block.concurrency)
            .collect::<Vec<_>>();
        let visits = tokio::select! {biased;_=context.cancel.cancelled()=>{batch.stopped=Some("cancelled");break;},_=tokio::time::sleep_until(context.deadline)=>{batch.stopped=Some("deadline");break;},visits=visits=>visits};
        for (branch, _) in &visits {
            shared
                .private_values
                .extend(branch.scopes.private_values.clone());
        }
        let uncertain = visits.iter().any(|(_, visit)| visit.error.is_some());
        for (branch, visit) in visits {
            if let Some(reason) = visit.stopped {
                batch.stopped.get_or_insert(reason);
            }
            if matches!(visit.stopped, Some("owner_changed")) {
                continue;
            }
            let (collection, request, step) = branch.step;
            let step = step.ok_or_else(ApiError::internal)?;
            let mut item = json!({"request_id":request.id,"request_name":request.name,"method":request.method,"collection_id":collection.id,"iteration":context.iteration,"step_id":step.id,"step_name":step.name,"step_group":step.group,"step_repeat_index":branch.repeat,"parallel_id":block.id,"parallel_name":block.name,"variable_updates_applied":false});
            if visit.condition_skipped {
                batch.skipped += 1;
                item["condition_skipped"] = true.into();
            }
            if let Some(outcome) = visit.outcome {
                if let Some(history) = outcome.history {
                    let admission = context.gate.lock().await;
                    if *admission != context.epoch
                        || owned(context.state, context.owner, &context.workspace.id)
                            .await
                            .is_err()
                    {
                        batch.stopped = Some("owner_changed");
                        break;
                    }
                    crate::history::record(
                        context.state,
                        context.owner,
                        context.workspace,
                        &history.request,
                        Some(&history.environment),
                        &outcome.response,
                        crate::privacy::HistoryPrivacy {
                            values: &shared.private_values,
                            redact_failed_response: uncertain || history.redact_failed_response,
                        },
                    )
                    .await?;
                }
                if visit.error.is_none() {
                    if outcome.response.skipped {
                        batch.skipped += 1;
                    } else if outcome.response.tests.iter().all(|test| test.passed) {
                        batch.passed += 1;
                    } else {
                        batch.failed += 1;
                    }
                }
                item["status"] = outcome.response.status.into();
                item["elapsed_ms"] = outcome.response.elapsed_ms.into();
                item["response"] =
                    serde_json::to_value(outcome.response).map_err(|_| ApiError::internal())?;
            }
            if let Some(error) = visit.error {
                batch.failed += 1;
                item["error"] = error.into();
            }
            let privacy = crate::privacy::Redactor::new(&shared.private_values)?;
            for field in ["step_name", "step_group", "parallel_name", "error"] {
                if let Some(value) = item.get_mut(field) {
                    privacy.scrub(value);
                }
            }
            report::append(&mut batch.items, item, &mut bytes, &mut omitted)?;
            branch.repeat += 1;
            if branch.repeat >= step.repeat {
                branch.active = false;
            }
        }
        if batch.stopped.is_some() {
            break;
        }
        if shared.validate().is_err() {
            batch.stopped = Some("variable_limit");
            break;
        }
    }
    if batch.stopped.is_none() {
        if context.cancel.is_cancelled() {
            batch.stopped = Some("cancelled");
        } else if tokio::time::Instant::now() >= context.deadline {
            batch.stopped = Some("deadline");
        } else if !admitted(context).await {
            batch.stopped = Some("owner_changed");
        }
    }
    if batch.stopped.is_none() {
        let mut merged = Writes::new();
        for branch in &branches {
            for (key, value) in final_changes(branch) {
                if merged.get(&key).is_some_and(|previous| *previous != value) {
                    batch.stopped = Some("parallel_variable_conflict");
                    batch.failed += 1;
                    break;
                }
                merged.insert(key, value);
            }
            if batch.stopped.is_some() {
                break;
            }
        }
        if batch.stopped.is_none() {
            let mut proposed = shared.clone();
            let mut proposed_overlays = overlays.clone();
            let mut updates = vec![];
            for ((scope, collection, key), value) in &merged {
                if scope == "collection" {
                    proposed_overlays
                        .entry(collection.clone())
                        .or_default()
                        .insert(key.clone(), value.clone());
                } else {
                    updates.push(VariableUpdate {
                        scope: scope.clone(),
                        key: key.clone(),
                        value: value.clone(),
                    });
                }
            }
            let mut valid =
                proposed.apply(&updates).is_ok() && scopes::validate_overlays(&proposed_overlays);
            if valid {
                for member in members {
                    let mut probe = proposed.clone();
                    if scopes::prepare(
                        &scopes::Inputs {
                            state: context.state,
                            workspace: context.workspace,
                            environment: context.environment,
                            data: context.data,
                            temporary: context.temporary,
                            overlays: &proposed_overlays,
                        },
                        member.0,
                        &mut probe,
                    )
                    .is_err()
                    {
                        valid = false;
                        break;
                    }
                }
            }
            if !valid {
                batch.stopped = Some("variable_limit");
                batch.failed += 1;
            } else {
                *shared = proposed;
                *overlays = proposed_overlays;
                batch.writes = merged;
            }
        }
    }
    Ok(batch)
}
