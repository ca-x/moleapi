mod parallel;
mod report;
mod scenario_control;
mod scopes;
type PlanStep<'a> = (
    &'a moleapi_core::Collection,
    &'a moleapi_core::RequestSpec,
    Option<&'a moleapi_core::ScenarioStep>,
);
use crate::execution::{environment, perform, variables};
use crate::{ApiError, AppState, auth::Identity, workspaces::owned};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use moleapi_core::{DatasetSource, IterationInfo};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Run {
    collection_id: String,
    #[serde(default)]
    scenario_id: Option<String>,
    environment_id: Option<String>,
    #[serde(default)]
    variables: Vec<moleapi_core::Pair>,
    #[serde(default)]
    data: Vec<moleapi_core::Pair>,
    #[serde(default)]
    locals: Vec<moleapi_core::VariableUpdate>,
    #[serde(default)]
    dataset: Option<DatasetSource>,
    #[serde(default)]
    dataset_id: Option<String>,
    #[serde(default)]
    iterations: Option<usize>,
    #[serde(default)]
    job_id: Option<String>,
}
#[derive(Deserialize)]
pub struct Preview {
    workspace_id: String,
    dataset: DatasetSource,
}
pub async fn preview(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Json(c): Json<Preview>,
) -> Result<Json<moleapi_core::Dataset>, ApiError> {
    owned(&s, &owner.0, &c.workspace_id).await?;
    Ok(Json(
        c.dataset
            .parse()
            .map_err(|error| ApiError::bad(error.to_string()))?,
    ))
}
#[derive(Deserialize)]
pub struct Cancel {
    job_id: String,
}
pub async fn cancel(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(workspace): Path<String>,
    Json(c): Json<Cancel>,
) -> Result<Json<Value>, ApiError> {
    owned(&s, &owner.0, &workspace).await?;
    s.project_jobs
        .cancel_job(&owner.0, Some(&workspace), &c.job_id)?;
    Ok(Json(json!({"cancelled":true})))
}
fn iteration_pairs(
    c: &Run,
    row: Option<&BTreeMap<String, Value>>,
) -> Result<Vec<moleapi_core::Pair>, ApiError> {
    let mut data = c
        .data
        .iter()
        .filter(|pair| pair.enabled)
        .map(|pair| (pair.key.clone(), pair.clone()))
        .collect::<BTreeMap<_, _>>();
    if let Some(row) = row {
        for pair in
            moleapi_core::dataset_pairs(row).map_err(|error| ApiError::bad(error.to_string()))?
        {
            data.insert(pair.key.clone(), pair);
        }
    }
    let pairs = data
        .into_values()
        .enumerate()
        .map(|(id, mut pair)| {
            pair.id = format!("run-data-{id}");
            pair
        })
        .collect::<Vec<_>>();
    Ok(pairs)
}
pub async fn run(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
    Json(c): Json<Run>,
) -> Result<Json<Value>, ApiError> {
    let gate = s.protocol_admission.owner(&owner.0)?;
    let epoch = *gate.lock().await;
    let w = owned(&s, &owner.0, &id).await?;
    let collection = w
        .data
        .collections
        .iter()
        .find(|value| value.id == c.collection_id)
        .ok_or_else(ApiError::not_found)?;
    let e = environment(&w, c.environment_id.as_deref())?;
    if c.dataset.is_some() && c.dataset_id.is_some() {
        return Err(ApiError::bad(
            "Select a saved dataset or supply temporary data, not both",
        ));
    }
    let selected_source = if let Some(id) = &c.dataset_id {
        Some(
            w.data
                .datasets
                .iter()
                .find(|dataset| dataset.id == *id)
                .ok_or_else(ApiError::not_found)?
                .source
                .as_ref()
                .ok_or_else(|| {
                    ApiError::bad("Saved dataset source is missing; import its data again")
                })?,
        )
    } else {
        c.dataset.as_ref()
    };
    let dataset = selected_source
        .map(DatasetSource::parse)
        .transpose()
        .map_err(|error| ApiError::bad(error.to_string()))?;
    let count = c
        .iterations
        .unwrap_or_else(|| dataset.as_ref().map_or(1, |dataset| dataset.rows.len()));
    if count == 0
        || count > 100
        || dataset
            .as_ref()
            .is_some_and(|dataset| count > dataset.rows.len())
    {
        return Err(ApiError::bad(
            "Run requires 1 to 100 iterations, within supplied dataset rows",
        ));
    }
    let subtree = moleapi_core::collection_subtree(&w.data, collection)
        .map_err(|error| ApiError::bad(error.to_string()))?;
    let scenario = c
        .scenario_id
        .as_ref()
        .map(|id| {
            w.data
                .scenarios
                .iter()
                .find(|scenario| scenario.id == *id)
                .ok_or_else(ApiError::not_found)
        })
        .transpose()?;
    if scenario.is_some_and(|scenario| scenario.collection_id != c.collection_id) {
        return Err(ApiError::bad(
            "Scenario root does not match selected collection",
        ));
    }
    let plan = if let Some(scenario) = scenario {
        moleapi_core::scenario_plan(&w.data, scenario)
            .map_err(|error| ApiError::bad(error.to_string()))?
            .into_iter()
            .map(|(collection, request, step)| (collection, request, Some(step)))
            .collect::<Vec<_>>()
    } else {
        subtree
            .iter()
            .flat_map(|collection| {
                collection
                    .requests
                    .iter()
                    .map(move |request| (*collection, request, None))
            })
            .collect::<Vec<_>>()
    };
    if plan
        .iter()
        .map(|(_, _, step)| step.map_or(1, |step| step.repeat))
        .sum::<usize>()
        .saturating_mul(count)
        > 1000
    {
        return Err(ApiError::bad("Run exceeds 1000 request executions"));
    }
    let scenario_steps = plan.iter().map(|(_, _, step)| *step).collect::<Vec<_>>();
    let mut scopes = variables(
        &s,
        &w,
        Some(collection),
        e,
        &c.data,
        &c.variables,
        &c.locals,
    )?;
    if let Some(dataset) = &dataset {
        scopes.private_values.extend(
            moleapi_core::dataset_private_values(&dataset.rows)
                .map_err(|error| ApiError::bad(error.to_string()))?,
        );
        scopes
            .validate()
            .map_err(|error| ApiError::bad(error.to_string()))?;
    }
    if let Some(dataset) = &dataset {
        for row in dataset.rows.iter().take(count) {
            let pairs = iteration_pairs(&c, Some(row))?;
            for selected in subtree
                .iter()
                .filter(|collection| !collection.requests.is_empty())
            {
                variables(&s, &w, Some(selected), e, &pairs, &c.variables, &c.locals)?;
            }
        }
    }
    let _slot = s
        .project_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::bad("Run execution capacity reached"))?;
    let job_id = c
        .job_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let lease = s.project_jobs.start(&owner.0, &w.id, &job_id)?;
    let started_at = crate::storage::now();
    let start = std::time::Instant::now();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(300);
    let mut results = vec![];
    let mut summaries = vec![];
    let (mut passed, mut failed, mut report_bytes, mut omitted) = (0usize, 0usize, 0usize, 0usize);
    let mut stopped = None;
    let (mut executed, mut skipped) = (0usize, 0usize);
    let mut overlays = scopes::Overlays::new();
    let mut parallel_writes = parallel::Writes::new();
    for local in c.locals.iter().filter(|local| local.scope == "collection") {
        overlays
            .entry(collection.id.clone())
            .or_default()
            .insert(local.key.clone(), local.value.clone());
    }
    'iterations: for index in 0..count {
        let iteration_start = std::time::Instant::now();
        let mut script_stopped = false;
        let mut scenario_stopped = false;
        let (before_passed, before_failed) = (passed, failed);
        let pairs = iteration_pairs(&c, dataset.as_ref().map(|dataset| &dataset.rows[index]))?;
        let iteration_base = variables(&s, &w, Some(collection), e, &pairs, &c.variables, &[])?;
        scopes.data = iteration_base.data;
        scopes.private_values.extend(iteration_base.private_values);
        scopes.iteration = Some(IterationInfo { index, count });
        scopes.iteration_data = dataset.as_ref().map(|dataset| {
            let mut values = c
                .data
                .iter()
                .filter(|pair| pair.enabled)
                .map(|pair| {
                    (
                        pair.key.clone(),
                        Value::String(pair.local_value.as_ref().unwrap_or(&pair.value).clone()),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            values.extend(dataset.rows[index].clone());
            values
        });
        scopes
            .validate()
            .map_err(|error| ApiError::bad(error.to_string()))?;
        let mut cursor = 0usize;
        let mut repeat_index = 0usize;
        let mut previous_response = None;
        'steps: while cursor < plan.len() {
            if executed >= 1000 {
                stopped = Some("step_limit");
                break 'iterations;
            }
            let (selected, request, step) = plan[cursor];
            let mut condition_passed = true;
            let mut condition_failed = false;
            if let Some(block) = scenario.and_then(|scenario| {
                scenario
                    .parallel
                    .iter()
                    .find(|block| block.step_ids.first() == step.map(|step| &step.id))
            }) {
                let end = cursor + block.step_ids.len();
                let context = parallel::Context {
                    state: &s,
                    owner: &owner.0,
                    workspace: &w,
                    environment: e,
                    data: &pairs,
                    temporary: &c.variables,
                    gate: &gate,
                    epoch,
                    cancel: &lease.cancel,
                    deadline,
                    iteration: index,
                };
                let batch = parallel::run(
                    &context,
                    block,
                    plan.get(cursor..end).ok_or_else(ApiError::internal)?,
                    &mut scopes,
                    &mut overlays,
                    previous_response.as_ref(),
                    &mut executed,
                )
                .await?;
                passed += batch.passed;
                failed += batch.failed;
                skipped += batch.skipped;
                for item in batch.items {
                    report::append(&mut results, item, &mut report_bytes, &mut omitted)?;
                }
                parallel_writes.extend(batch.writes);
                if let Some(reason) = batch.stopped {
                    stopped = Some(reason);
                    break 'iterations;
                }
                previous_response = None;
                repeat_index = 0;
                cursor = end;
                continue 'steps;
            }
            if scenario.is_some_and(|scenario| {
                scenario.parallel.iter().any(|block| {
                    block
                        .step_ids
                        .iter()
                        .skip(1)
                        .any(|id| step.is_some_and(|step| step.id == *id))
                })
            }) {
                stopped = Some("parallel_entry");
                break 'iterations;
            }
            scopes::prepare(
                &scopes::Inputs {
                    state: &s,
                    workspace: &w,
                    environment: e,
                    data: &pairs,
                    temporary: &c.variables,
                    overlays: &overlays,
                },
                selected,
                &mut scopes,
            )?;
            {
                executed += 1;
                if lease.cancel.is_cancelled() {
                    stopped = Some("cancelled");
                    break 'iterations;
                }
                let admission = gate.lock().await;
                if *admission != epoch || owned(&s, &owner.0, &id).await.is_err() {
                    stopped = Some("owner_changed");
                    break 'iterations;
                }
                drop(admission);
                let before = scopes.collection.clone();
                scopes.execution = moleapi_core::ExecutionControl::default();
                if let Some(expression) = step.and_then(|step| step.condition.as_deref()) {
                    let evaluation = tokio::select! {biased;_=lease.cancel.cancelled()=>{stopped=Some("cancelled");break 'iterations;},_=tokio::time::sleep_until(deadline)=>{stopped=Some("deadline");break 'iterations;},result=scenario_control::condition(&s,expression,request,previous_response.as_ref(),&mut scopes)=>result};
                    match evaluation {
                        Ok(value) => condition_passed = value,
                        Err(()) => condition_failed = true,
                    }
                }
                let mut item = if condition_failed {
                    failed += 1;
                    json!({"request_id":request.id,"request_name":request.name,"method":request.method,"collection_id":selected.id,"iteration":index,"error":"Scenario condition failed; check its boolean result and execution limits"})
                } else if !condition_passed {
                    skipped += 1;
                    json!({"request_id":request.id,"request_name":request.name,"method":request.method,"collection_id":selected.id,"iteration":index,"condition_skipped":true})
                } else {
                    // A condition worker may have yielded while the owner or workspace changed.
                    if step.and_then(|step| step.condition.as_ref()).is_some() {
                        let admission = gate.lock().await;
                        if *admission != epoch || owned(&s, &owner.0, &id).await.is_err() {
                            stopped = Some("owner_changed");
                            break 'iterations;
                        }
                    }
                    let result = tokio::select! {biased;_=lease.cancel.cancelled()=>{stopped=Some("cancelled");break 'iterations;},_=tokio::time::sleep_until(deadline)=>{stopped=Some("deadline");break 'iterations;},result=perform(&s,&owner.0,&w,request,Some(selected),&mut scopes)=>result};
                    match result {
                        Ok(response) => {
                            if response.skipped {
                                skipped += 1;
                            } else {
                                previous_response = Some(response.clone());
                                if response.tests.iter().all(|test| test.passed) {
                                    passed += 1;
                                } else {
                                    failed += 1;
                                }
                            }
                            json!({"request_id":request.id,"request_name":request.name,"method":request.method,"collection_id":selected.id,"iteration":index,"status":response.status,"elapsed_ms":response.elapsed_ms,"response":response})
                        }
                        Err(error) => {
                            previous_response = None;
                            failed += 1;
                            json!({"request_id":request.id,"request_name":request.name,"method":request.method,"collection_id":selected.id,"iteration":index,"error":error.message})
                        }
                    }
                };
                if let Some(step) = step {
                    item["step_id"] = step.id.clone().into();
                    item["step_repeat_index"] = repeat_index.into();
                    let privacy = crate::privacy::Redactor::new(&scopes.private_values)
                        .map_err(|_| ApiError::internal())?;
                    item["step_name"] = step.name.clone().into();
                    item["step_group"] = step.group.clone().into();
                    privacy.scrub(&mut item["step_name"]);
                    privacy.scrub(&mut item["step_group"]);
                }
                report::append(&mut results, item, &mut report_bytes, &mut omitted)?;
                if condition_failed {
                    stopped = Some("condition_error");
                    break 'iterations;
                }
                scopes::capture(&before, &scopes.collection, &selected.id, &mut overlays);
                if !scopes::validate_overlays(&overlays) {
                    stopped = Some("variable_limit");
                    break 'iterations;
                }
            }
            if !condition_passed {
                scenario_stopped = matches!(
                    step.and_then(|step| step.on_false.as_ref()),
                    Some(moleapi_core::ScenarioTarget::Stop)
                );
                repeat_index = 0;
                cursor = scenario_control::next(
                    step.and_then(|step| step.on_false.as_ref()),
                    cursor,
                    &scenario_steps,
                )?;
                continue 'steps;
            }
            cursor = match &scopes.execution.next_request {
                None => {
                    if step.is_some_and(|step| repeat_index + 1 < step.repeat) {
                        repeat_index += 1;
                        cursor
                    } else {
                        scenario_stopped = matches!(
                            step.and_then(|step| step.on_true.as_ref()),
                            Some(moleapi_core::ScenarioTarget::Stop)
                        );
                        repeat_index = 0;
                        scenario_control::next(
                            step.and_then(|step| step.on_true.as_ref()),
                            cursor,
                            &scenario_steps,
                        )?
                    }
                }
                Some(moleapi_core::NextRequest::Stop) => {
                    script_stopped = true;
                    break 'steps;
                }
                Some(moleapi_core::NextRequest::Request { target }) => {
                    repeat_index = 0;
                    let by_id = plan
                        .iter()
                        .enumerate()
                        .filter(|(_, (_, request, _))| request.id == *target)
                        .map(|(index, _)| index)
                        .collect::<Vec<_>>();
                    let matches = if by_id.is_empty() {
                        plan.iter()
                            .enumerate()
                            .filter(|(_, (_, request, _))| request.name == *target)
                            .map(|(index, _)| index)
                            .collect::<Vec<_>>()
                    } else {
                        by_id
                    };
                    if matches.len() != 1 {
                        stopped = Some(if matches.is_empty() {
                            "next_request_missing"
                        } else {
                            "next_request_ambiguous"
                        });
                        break 'iterations;
                    }
                    matches[0]
                }
            };
        }
        summaries.push(json!({"iteration":index,"passed":passed-before_passed,"failed":failed-before_failed,"elapsed_ms":iteration_start.elapsed().as_millis()as u64,"script_stopped":script_stopped,"scenario_stopped":scenario_stopped}));
    }
    let mut report = json!({"results":results,"iterations":summaries,"iteration_count":count,"completed_iterations":summaries.len(),"passed":passed,"failed":failed,"elapsed_ms":start.elapsed().as_millis()as u64,"cancelled":matches!(stopped,Some("cancelled"|"owner_changed")),"stopped_reason":stopped,"omitted_responses":omitted,"job_id":job_id,"executed_steps":executed,"skipped":skipped});
    let mut final_parallel = vec![];
    for ((scope, collection, key), _) in parallel_writes {
        let value = match scope.as_str() {
            "project" => scopes.project.get(&key).cloned(),
            "environment" => scopes.environment.get(&key).cloned(),
            "temporary" => scopes.temporary.get(&key).cloned(),
            "collection" => overlays
                .get(&collection)
                .and_then(|values| values.get(&key))
                .cloned()
                .flatten(),
            _ => continue,
        };
        final_parallel.push(json!({"collection_id":if collection.is_empty(){c.collection_id.clone()}else{collection},"scope":scope,"key":key,"value":value}));
    }
    if !final_parallel.is_empty() {
        report["parallel_variable_updates"] = final_parallel.into();
    }
    if let Some(scenario) = scenario {
        report["scenario_id"] = scenario.id.clone().into();
    }
    if serde_json::to_vec(&report)
        .map_err(|_| ApiError::internal())?
        .len()
        > 8 * 1024 * 1024
    {
        let items = report["results"].as_array_mut().unwrap();
        for item in items.iter_mut() {
            let values = item.as_object_mut().unwrap();
            values.remove("response");
            values.remove("error");
            values.remove("request_name");
            item["response_omitted"] = true.into();
        }
        report["omitted_responses"] = items.len().into();
    }
    {
        let admission = gate.lock().await;
        if *admission == epoch {
            let source = crate::run_reports::Source {
                workspace: &w,
                collection,
                scenario,
                environment: e,
                dataset: c.dataset_id.as_deref(),
                started_at: &started_at,
                scopes: &scopes,
            };
            match crate::run_reports::record(&s, &owner.0, &source, &report).await {
                Ok(Some(id)) => {
                    report["report_id"] = id.into();
                }
                Ok(None) => {
                    report["report_save_error"] =
                        "Workspace was deleted before the report could be saved".into();
                }
                Err(_) => {
                    report["report_save_error"] = "Run report could not be persisted".into();
                }
            }
        }
    }
    Ok(Json(report))
}
