use crate::{ApiError, AppState, auth::Identity, workspaces::owned};
use axum::{Extension, Json, extract::State};
use moleapi_core::{
    Collection, Environment, Pair, RequestSpec, Response, ScriptLog, TestResult, VariableScopes,
    VariableUpdate, Workspace,
};
use serde::Deserialize;
#[derive(Deserialize)]
pub struct Execute {
    workspace_id: String,
    request: RequestSpec,
    environment_id: Option<String>,
    #[serde(default)]
    variables: Vec<Pair>,
    #[serde(default)]
    data: Vec<Pair>,
    #[serde(default)]
    locals: Vec<VariableUpdate>,
}
pub(crate) fn environment<'a>(
    w: &'a Workspace,
    id: Option<&str>,
) -> Result<Option<&'a Environment>, ApiError> {
    let id = id.or(w.data.active_environment_id.as_deref());
    id.map(|id| {
        w.data
            .environments
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| ApiError::bad("Environment not found"))
    })
    .transpose()
}
pub(crate) fn variables(
    s: &AppState,
    w: &Workspace,
    collection: Option<&Collection>,
    environment: Option<&Environment>,
    data: &[Pair],
    temporary: &[Pair],
    locals: &[VariableUpdate],
) -> Result<VariableScopes, ApiError> {
    let mut scopes =
        VariableScopes::new(&w.data, collection, environment, data, temporary, s.local)
            .map_err(|e| ApiError::bad(e.to_string()))?;
    scopes
        .apply_locals(locals)
        .map_err(|e| ApiError::bad(e.to_string()))?;
    Ok(scopes)
}
struct PhaseFailure {
    error: ApiError,
    private_values: std::collections::BTreeSet<String>,
    privacy_complete: bool,
}
async fn script_phase(
    s: &AppState,
    scripts: Vec<String>,
    request: &RequestSpec,
    response: Option<&Response>,
    scopes: &VariableScopes,
) -> Result<moleapi_script_runtime::ScriptOutput, PhaseFailure> {
    let _permit = s
        .script_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| PhaseFailure {
            error: ApiError {
                status: axum::http::StatusCode::TOO_MANY_REQUESTS,
                message: "Script execution capacity reached; retry shortly".into(),
            },
            private_values: scopes.private_values.clone(),
            privacy_complete: true,
        })?;
    moleapi_script_runtime::run_worker(&s.script_worker, scripts, request, response, scopes)
        .await
        .map_err(|failure| PhaseFailure {
            error: ApiError::bad(failure.message),
            private_values: failure.private_values,
            privacy_complete: failure.privacy_complete,
        })
}
pub(crate) async fn perform(
    s: &AppState,
    owner: &str,
    w: &Workspace,
    r: &RequestSpec,
    collection: Option<&Collection>,
    scopes: &mut VariableScopes,
) -> Result<Response, ApiError> {
    moleapi_core::validate_request(r, true).map_err(|e| ApiError::bad(e.to_string()))?;
    let pre = vec![
        w.data.pre_request_script.clone(),
        collection
            .map(|c| c.pre_request_script.clone())
            .unwrap_or_default(),
        r.pre_request_script.clone(),
    ];
    let post = vec![
        r.post_response_script.clone(),
        collection
            .map(|c| c.post_response_script.clone())
            .unwrap_or_default(),
        w.data.post_response_script.clone(),
    ];
    let prepared = moleapi_core::prepare_graphql(r).map_err(|e| ApiError::bad(e.to_string()))?;
    let mut request = prepared.clone();
    let mut logs = vec![];
    let mut tests = vec![];
    let mut updates = vec![];
    let mut redact_failed_response = false;
    if pre.iter().any(|s| !s.trim().is_empty()) {
        let output = script_phase(s, pre, &request, None, scopes)
            .await
            .map_err(|failure| failure.error)?;
        scopes.private_values.extend(output.private_values);
        scopes
            .apply(&output.updates)
            .map_err(|e| ApiError::bad(e.to_string()))?;
        request = output.request;
        logs = output.logs;
        tests = output.tests;
        updates = output.updates;
        for test in &mut tests {
            test.id = format!("pre-{}", test.id);
        }
    }
    let mut request_updates = vec![];
    for (field, before, after) in [
        ("method", prepared.method.clone(), request.method.clone()),
        ("url", prepared.url.clone(), request.url.clone()),
        (
            "body_kind",
            prepared.body_kind.clone(),
            request.body_kind.clone(),
        ),
        ("body", prepared.body.clone(), request.body.clone()),
        (
            "headers",
            serde_json::to_string(&prepared.headers).map_err(|_| ApiError::internal())?,
            serde_json::to_string(&request.headers).map_err(|_| ApiError::internal())?,
        ),
    ] {
        if before != after {
            request_updates.push(moleapi_core::RequestUpdate {
                field: field.into(),
                value: after,
            });
        }
    }
    if request.protocol.is_graphql() != r.protocol.is_graphql() {
        return Err(ApiError::bad(
            "Pre scripts cannot change the GraphQL protocol",
        ));
    }
    let effective = scopes.effective();
    let resolved = moleapi_core::resolve_request(&request, Some(&effective))
        .map_err(|e| ApiError::bad(e.to_string()))?;
    let mut response = moleapi_core::execute(
        &resolved,
        None,
        moleapi_core::NetworkPolicy {
            allow_private_network: s.local || s.config.allow_private_network,
        },
    )
    .await
    .map_err(|e| ApiError::bad(e.to_string()))?;
    response.tests.extend(tests);
    if post.iter().any(|s| !s.trim().is_empty()) {
        match script_phase(s, post, &resolved, Some(&response), scopes).await {
            Ok(mut output) => {
                scopes.private_values.extend(output.private_values);
                scopes
                    .apply(&output.updates)
                    .map_err(|e| ApiError::bad(e.to_string()))?;
                logs.extend(output.logs);
                for test in &mut output.tests {
                    test.id = format!("post-{}", test.id);
                }
                response.tests.extend(output.tests);
                updates.extend(output.updates);
            }
            Err(failure) => {
                scopes.private_values.extend(failure.private_values);
                redact_failed_response = !failure.privacy_complete;
                let error = failure.error;
                logs.push(ScriptLog {
                    level: "error".into(),
                    message: error.message.clone(),
                });
                response.tests.push(TestResult {
                    id: "post-script-error".into(),
                    name: "Post-response script".into(),
                    passed: false,
                    actual: error.message,
                    expected: "Script completes successfully".into(),
                });
            }
        }
    }
    response.request_updates = request_updates;
    response.logs = logs;
    response.variable_updates = updates;
    // Ephemeral values and script-generated values are never written into workspace data.
    let history_environment = scopes.effective();
    let mut history_request = resolved.clone();
    history_request.id = r.id.clone();
    crate::history::record(
        s,
        owner,
        w,
        &history_request,
        Some(&history_environment),
        &response,
        crate::privacy::HistoryPrivacy {
            values: &scopes.private_values,
            redact_failed_response,
        },
    )
    .await?;
    Ok(response)
}
pub async fn execute(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Json(c): Json<Execute>,
) -> Result<Json<Response>, ApiError> {
    let w = owned(&s, &owner.0, &c.workspace_id).await?;
    if c.request.protocol.is_graphql()
        && let Some(id) = &c.request.specification_id
        && !w.data.specifications.iter().any(|s| &s.id == id)
    {
        return Err(ApiError::not_found());
    }
    let e = environment(&w, c.environment_id.as_deref())?;
    let collection = w.data.collections.iter().find(|collection| {
        collection
            .requests
            .iter()
            .any(|request| request.id == c.request.id)
    });
    let mut scopes = variables(&s, &w, collection, e, &c.data, &c.variables, &c.locals)?;
    Ok(Json(
        perform(&s, &owner.0, &w, &c.request, collection, &mut scopes).await?,
    ))
}

/// Prepare a live connection with the same isolated pre-script worker and scoped values.
pub(crate) async fn prepare_live(
    s: &AppState,
    w: &Workspace,
    r: &RequestSpec,
    collection: &Collection,
    scopes: &mut VariableScopes,
) -> Result<
    (
        RequestSpec,
        moleapi_protocols::PreparedFeedback,
        Vec<VariableUpdate>,
        Vec<moleapi_core::RequestUpdate>,
    ),
    ApiError,
> {
    if [
        &w.data.post_response_script,
        &collection.post_response_script,
        &r.post_response_script,
    ]
    .iter()
    .any(|s| !s.trim().is_empty())
    {
        return Err(ApiError::bad(
            "Post-response scripts are unavailable for live protocols until a per-event script contract exists",
        ));
    }
    moleapi_core::validate_request(r, true).map_err(|e| ApiError::bad(e.to_string()))?;
    crate::privacy::request_values(r, scopes)?;
    let scripts = vec![
        w.data.pre_request_script.clone(),
        collection.pre_request_script.clone(),
        r.pre_request_script.clone(),
    ];
    let prepared = moleapi_core::prepare_graphql(r).map_err(|e| ApiError::bad(e.to_string()))?;
    let prepared = moleapi_core::prepare_grpc(&prepared);
    let mut request = prepared.clone();
    let mut feedback = moleapi_protocols::PreparedFeedback::default();
    let mut updates = vec![];
    if scripts.iter().any(|s| !s.trim().is_empty()) {
        let output = script_phase(s, scripts, &request, None, scopes).await.map_err(|failure| {
            scopes.private_values.extend(failure.private_values);
            let mut message = serde_json::json!(failure.error.message);
            if failure.privacy_complete {
                match crate::privacy::Redactor::new(&scopes.private_values) {
                    Ok(redactor) => redactor.scrub(&mut message),
                    Err(_) => message=serde_json::json!("Live pre-script failed; privacy redaction unavailable"),
                }
            } else {
                message=serde_json::json!("Live pre-script failed; details withheld because privacy metadata is incomplete");
            }
            ApiError::bad(message.as_str().unwrap_or("Live pre-script failed"))
        })?;
        scopes.private_values.extend(output.private_values);
        scopes
            .apply(&output.updates)
            .map_err(|e| ApiError::bad(e.to_string()))?;
        request = output.request;
        crate::privacy::request_values(&request, scopes)?;
        feedback.logs = output.logs;
        feedback.tests = output.tests;
        updates = output.updates;
    }
    moleapi_core::reconcile_grpc(&mut request).map_err(|e| ApiError::bad(e.to_string()))?;
    let mut request_updates = vec![];
    for (field, before, after) in [
        ("method", prepared.method.clone(), request.method.clone()),
        ("url", prepared.url.clone(), request.url.clone()),
        (
            "body_kind",
            prepared.body_kind.clone(),
            request.body_kind.clone(),
        ),
        ("body", prepared.body.clone(), request.body.clone()),
        (
            "headers",
            serde_json::to_string(&prepared.headers).map_err(|_| ApiError::internal())?,
            serde_json::to_string(&request.headers).map_err(|_| ApiError::internal())?,
        ),
    ] {
        if before != after {
            request_updates.push(moleapi_core::RequestUpdate {
                field: field.into(),
                value: after,
            });
        }
    }
    // None is the execution mode, regardless of retained editor draft text. Scripts
    // still see that draft above; ignore it only in the execution clone so unrelated
    // draft templates neither resolve nor become request updates or wire bytes.
    let effective = scopes.effective();
    if moleapi_core::resolve_value(&request.body_kind, &effective).is_ok_and(|kind| kind == "none")
    {
        request.body.clear();
    }
    let resolved = moleapi_core::resolve_request(&request, Some(&effective))
        .map_err(|e| ApiError::bad(e.to_string()))?;
    crate::privacy::request_values(&resolved, scopes)?;
    Ok((resolved, feedback, updates, request_updates))
}
