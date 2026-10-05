use crate::execution::{environment, perform, variables};
use crate::{ApiError, AppState, auth::Identity, workspaces::owned};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use serde::Deserialize;
#[derive(Deserialize)]
pub struct Run {
    collection_id: String,
    environment_id: Option<String>,
    #[serde(default)]
    variables: Vec<moleapi_core::Pair>,
    #[serde(default)]
    data: Vec<moleapi_core::Pair>,
    #[serde(default)]
    locals: Vec<moleapi_core::VariableUpdate>,
}
pub async fn run(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
    Json(c): Json<Run>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let w = owned(&s, &owner.0, &id).await?;
    let collection = w
        .data
        .collections
        .iter()
        .find(|v| v.id == c.collection_id)
        .ok_or_else(ApiError::not_found)?;
    let e = environment(&w, c.environment_id.as_deref())?;
    let mut scopes = variables(
        &s,
        &w,
        Some(collection),
        e,
        &c.data,
        &c.variables,
        &c.locals,
    )?;
    let start = std::time::Instant::now();
    let mut results = vec![];
    let mut passed = 0;
    let mut failed = 0;
    let subtree = moleapi_core::collection_subtree(&w.data, collection)
        .map_err(|e| ApiError::bad(e.to_string()))?;
    let mut overlays = std::collections::BTreeMap::<
        String,
        std::collections::BTreeMap<String, Option<String>>,
    >::new();
    for local in c.locals.iter().filter(|v| v.scope == "collection") {
        overlays
            .entry(collection.id.clone())
            .or_default()
            .insert(local.key.clone(), local.value.clone());
    }
    for selected in subtree {
        let base = variables(&s, &w, Some(selected), e, &c.data, &c.variables, &[])?;
        scopes.private_values.extend(base.private_values);
        scopes.collection.clear();
        for ancestor in moleapi_core::collection_chain(&w.data, selected)
            .map_err(|e| ApiError::bad(e.to_string()))?
        {
            for pair in ancestor
                .variables
                .iter()
                .filter(|p| p.enabled && ancestor.variables_enabled != Some(false))
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
        for r in &selected.requests {
            let before = scopes.collection.clone();
            match perform(&s, &owner.0, &w, r, Some(selected), &mut scopes).await {
                Ok(response) => {
                    if response.tests.iter().all(|t| t.passed) {
                        passed += 1;
                    } else {
                        failed += 1;
                    }
                    results.push(serde_json::json!({"request_id":r.id,"request_name":r.name,"response":response}));
                }
                Err(err) => {
                    failed += 1;
                    results.push(serde_json::json!({"request_id":r.id,"request_name":r.name,"error":err.message}));
                }
            }
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
        }
    }

    Ok(Json(
        serde_json::json!({"results":results,"passed":passed,"failed":failed,"elapsed_ms":start.elapsed().as_millis() as u64}),
    ))
}
