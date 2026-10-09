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
    let dataset = c
        .dataset
        .as_ref()
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
    let steps = subtree
        .iter()
        .map(|collection| collection.requests.len())
        .sum::<usize>();
    if steps.saturating_mul(count) > 1000 {
        return Err(ApiError::bad("Run exceeds 1000 request executions"));
    }
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
    let start = std::time::Instant::now();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(300);
    let mut results = vec![];
    let mut summaries = vec![];
    let (mut passed, mut failed, mut report_bytes, mut omitted) = (0usize, 0usize, 0usize, 0usize);
    let mut stopped = None;
    let mut overlays = BTreeMap::<String, BTreeMap<String, Option<String>>>::new();
    for local in c.locals.iter().filter(|local| local.scope == "collection") {
        overlays
            .entry(collection.id.clone())
            .or_default()
            .insert(local.key.clone(), local.value.clone());
    }
    'iterations: for index in 0..count {
        let iteration_start = std::time::Instant::now();
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
        for selected in &subtree {
            let base = variables(&s, &w, Some(selected), e, &pairs, &c.variables, &[])?;
            scopes.private_values.extend(base.private_values);
            scopes.collection.clear();
            for ancestor in moleapi_core::collection_chain(&w.data, selected)
                .map_err(|error| ApiError::bad(error.to_string()))?
            {
                for pair in ancestor
                    .variables
                    .iter()
                    .filter(|pair| pair.enabled && ancestor.variables_enabled != Some(false))
                {
                    scopes.collection.insert(
                        pair.key.clone(),
                        if s.local {
                            pair.local_value.as_ref().unwrap_or(&pair.value)
                        } else {
                            &pair.value
                        }
                        .clone(),
                    );
                }
                if let Some(overlay) = overlays.get(&ancestor.id) {
                    for (key, value) in overlay {
                        if let Some(value) = value {
                            scopes.collection.insert(key.clone(), value.clone());
                        } else {
                            scopes.collection.remove(key);
                        }
                    }
                }
            }
            for request in &selected.requests {
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
                let result = tokio::select! {biased;_=lease.cancel.cancelled()=>{stopped=Some("cancelled");break 'iterations;},_=tokio::time::sleep_until(deadline)=>{stopped=Some("deadline");break 'iterations;},result=perform(&s,&owner.0,&w,request,Some(selected),&mut scopes)=>result};
                let mut item = match result {
                    Ok(response) => {
                        if response.tests.iter().all(|test| test.passed) {
                            passed += 1;
                        } else {
                            failed += 1;
                        }
                        json!({"request_id":request.id,"request_name":request.name,"collection_id":selected.id,"iteration":index,"status":response.status,"elapsed_ms":response.elapsed_ms,"response":response})
                    }
                    Err(error) => {
                        failed += 1;
                        json!({"request_id":request.id,"request_name":request.name,"collection_id":selected.id,"iteration":index,"error":error.message})
                    }
                };
                let size = serde_json::to_vec(&item)
                    .map_err(|_| ApiError::internal())?
                    .len();
                if report_bytes.saturating_add(size) > 8 * 1024 * 1024 {
                    item.as_object_mut().unwrap().remove("response");
                    item["response_omitted"] = true.into();
                    omitted += 1;
                }
                report_bytes += serde_json::to_vec(&item)
                    .map_err(|_| ApiError::internal())?
                    .len();
                results.push(item);
                for key in before
                    .keys()
                    .chain(scopes.collection.keys())
                    .collect::<std::collections::BTreeSet<_>>()
                {
                    if before.get(key) != scopes.collection.get(key) {
                        overlays
                            .entry(selected.id.clone())
                            .or_default()
                            .insert(key.clone(), scopes.collection.get(key).cloned());
                    }
                }
                if overlays
                    .values()
                    .flat_map(|values| values.iter())
                    .map(|(key, value)| key.len() + value.as_ref().map_or(0, String::len))
                    .sum::<usize>()
                    > moleapi_core::MAX_VARIABLE_BYTES
                {
                    stopped = Some("variable_limit");
                    break 'iterations;
                }
            }
        }
        summaries.push(json!({"iteration":index,"passed":passed-before_passed,"failed":failed-before_failed,"elapsed_ms":iteration_start.elapsed().as_millis()as u64}));
    }
    let mut report = json!({"results":results,"iterations":summaries,"iteration_count":count,"completed_iterations":summaries.len(),"passed":passed,"failed":failed,"elapsed_ms":start.elapsed().as_millis()as u64,"cancelled":matches!(stopped,Some("cancelled"|"owner_changed")),"stopped_reason":stopped,"omitted_responses":omitted,"job_id":job_id});
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
    Ok(Json(report))
}
