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
    for r in &collection.requests {
        match perform(&s, &owner.0, &w, r, Some(collection), &mut scopes).await {
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
    }
    Ok(Json(
        serde_json::json!({"results":results,"passed":passed,"failed":failed,"elapsed_ms":start.elapsed().as_millis() as u64}),
    ))
}
