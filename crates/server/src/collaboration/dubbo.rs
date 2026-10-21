use super::*;
use moleapi_core::{DubboConfig, RequestSpec, VariableScopes};
use moleapi_protocols::dubbo::DubboMetadataPreview;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetadataSource {
    request_id: String,
    expected_revision: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetadataImport {
    request_id: String,
    expected_revision: i64,
    document_sha256: String,
    decisions: Vec<crate::dubbo::MethodChoice>,
}
async fn admit(
    state: &AppState,
    actor: &str,
    owner: &str,
    workspace: &str,
    request_id: &str,
    revision: i64,
    action: &str,
) -> Result<(Workspace, RequestSpec, String, Access, VariableScopes), ApiError> {
    if state.local {
        return Err(ApiError::not_found());
    }
    let tx = state.db.begin().await?;
    let canonical = lock_workspace(&tx, owner, workspace).await?;
    let (_, access) = access(&tx, actor, owner, workspace).await?;
    if access.role == Role::Viewer {
        return Err(ApiError::forbidden("Viewer cannot inspect Dubbo metadata"));
    }
    if canonical.revision != revision {
        return Err(ApiError::conflict(
            "Workspace changed before Dubbo metadata operation",
        ));
    }
    let collection = canonical
        .data
        .collections
        .iter()
        .find(|c| {
            c.requests
                .iter()
                .any(|r| r.id == request_id && r.protocol.is_dubbo())
        })
        .ok_or_else(ApiError::not_found)?;
    let request = collection
        .requests
        .iter()
        .find(|r| r.id == request_id)
        .cloned()
        .ok_or_else(ApiError::not_found)?;
    moleapi_core::validate_request(&request, true).map_err(|e| ApiError::bad(e.to_string()))?;
    let environment = crate::execution::environment(&canonical, None)?;
    let mut scopes = crate::execution::variables(
        state,
        &canonical,
        Some(collection),
        environment,
        &[],
        &[],
        &[],
    )?;
    crate::privacy::request_values(&request, &mut scopes)?;
    let chain = moleapi_core::collection_chain(&canonical.data, collection)
        .map_err(|e| ApiError::bad(e.to_string()))?;
    for auth in canonical
        .data
        .auth
        .iter()
        .chain(chain.iter().filter_map(|c| c.auth.as_ref()))
    {
        let mut inherited = request.clone();
        inherited.auth = auth.clone();
        crate::privacy::request_values(&inherited, &mut scopes)?;
    }
    let collection_id = collection.id.clone();
    record_event_result(
        &tx,
        owner,
        workspace,
        actor,
        action,
        Some(request_id),
        "started",
    )
    .await?;
    tx.commit().await?;
    Ok((canonical, request, collection_id, access, scopes))
}
fn resolved_config(
    request: &RequestSpec,
    scopes: &VariableScopes,
) -> Result<DubboConfig, ApiError> {
    let moleapi_core::Protocol::Dubbo { config } = &request.protocol else {
        return Err(ApiError::not_found());
    };
    let mut config = config.as_ref().clone();
    let environment = scopes.effective();
    for value in [
        &mut config.service,
        &mut config.version,
        &mut config.group,
        &mut config.metadata_center_url,
        &mut config.metadata_application,
        &mut config.metadata_namespace,
        &mut config.metadata_group,
    ] {
        *value = moleapi_core::resolve_value(value, &environment)
            .map_err(|_| ApiError::bad("Dubbo metadata scope resolution failed"))?;
    }
    if let Some(auth) = &mut config.metadata_auth {
        auth.username = moleapi_core::resolve_value(&auth.username, &environment)
            .map_err(|_| ApiError::bad("Dubbo metadata scope resolution failed"))?;
        auth.password = moleapi_core::resolve_value(&auth.password, &environment)
            .map_err(|_| ApiError::bad("Dubbo metadata scope resolution failed"))?;
    }
    Ok(config)
}

async fn read(
    state: &AppState,
    request: &RequestSpec,
    scopes: &VariableScopes,
) -> Result<(DubboMetadataPreview, DubboConfig), ApiError> {
    let config = resolved_config(request, scopes)?;
    let result = crate::dubbo::read_metadata(
        &config,
        moleapi_core::NetworkPolicy {
            allow_private_network: state.local || state.config.allow_private_network,
        },
        std::time::Duration::from_millis(request.timeout_ms.min(15_000)),
    )
    .await;
    let preview = result.map_err(|error| {
        let mut message = json!(error.to_string());
        match crate::privacy::Redactor::new(&scopes.private_values) {
            Ok(redactor) => redactor.scrub(&mut message),
            Err(_) => return ApiError::bad("Dubbo metadata read failed"),
        };
        ApiError::bad(message.as_str().unwrap_or("Dubbo metadata read failed"))
    })?;
    let mut private = scopes.private_values.clone();
    private.extend(preview.private_auth_values.iter().cloned());
    crate::dubbo::screen_metadata(&preview, &private)?;
    Ok((preview, config))
}
pub async fn shared_dubbo_metadata_preview(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    headers: HeaderMap,
    Path((owner, workspace)): Path<(String, String)>,
    Json(input): Json<MetadataSource>,
) -> Result<Json<DubboMetadataPreview>, ApiError> {
    let (canonical, request, _, admission, scopes) = admit(
        &state,
        &actor.0,
        &owner,
        &workspace,
        &input.request_id,
        input.expected_revision,
        "dubbo.metadata.preview",
    )
    .await?;
    let operation = async {
        read(&state, &request, &scopes)
            .await
            .map(|(preview, _)| Json(preview))
    };
    let watch = watch_authenticated_revision(
        &state,
        &actor.0,
        &owner,
        &workspace,
        admission,
        canonical.revision,
        &headers,
    );
    let check = finish_specification_preview(
        &state.db,
        &actor.0,
        &owner,
        &workspace,
        (admission, canonical.revision),
        crate::auth::still_authenticated(&state, &actor.0, &headers),
    );
    with_access_watch(operation, watch, check).await
}
pub async fn shared_dubbo_metadata_import(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    headers: HeaderMap,
    Path((owner, workspace)): Path<(String, String)>,
    Json(input): Json<MetadataImport>,
) -> Result<Json<Workspace>, ApiError> {
    let (canonical, request, collection_id, admission, scopes) = admit(
        &state,
        &actor.0,
        &owner,
        &workspace,
        &input.request_id,
        input.expected_revision,
        "dubbo.metadata.import_candidate",
    )
    .await?;
    if input.document_sha256.len() != 64
        || !input.document_sha256.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(ApiError::bad("Invalid metadata document digest"));
    }
    let revision = canonical.revision;
    let operation = async {
        let (preview, config) = read(&state, &request, &scopes).await?;
        let (w, _) = crate::dubbo::build_metadata_import(
            canonical,
            crate::dubbo::MetadataImportInput {
                request,
                collection_id,
                expected_revision: revision,
                document_sha256: input.document_sha256,
                decisions: input.decisions,
            },
            preview,
            &config,
        )?;
        Ok(Json(w))
    };
    let watch = watch_authenticated_revision(
        &state, &actor.0, &owner, &workspace, admission, revision, &headers,
    );
    let check = finish_specification_preview(
        &state.db,
        &actor.0,
        &owner,
        &workspace,
        (admission, revision),
        crate::auth::still_authenticated(&state, &actor.0, &headers),
    );
    with_access_watch(operation, watch, check).await
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DubboCancel {
    request_id: String,
    expected_revision: i64,
    job_id: String,
}
fn execution_key(
    actor: &str,
    owner: &str,
    workspace: &str,
    request_id: &str,
    admission: Access,
    job_id: &str,
) -> Result<String, ApiError> {
    if job_id.is_empty()
        || job_id.len() > 128
        || !job_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return Err(ApiError::bad("Invalid Dubbo job ID"));
    }
    let identity = serde_json::to_vec(&(request_id, job_id)).map_err(|_| ApiError::internal())?;
    let id = hex::encode(Sha256::digest(identity));
    Ok(format!(
        "dubbo-{}",
        member_run_job_id(owner, workspace, actor, admission, &id)?
    ))
}
pub(super) fn execution_lease(
    state: &AppState,
    actor: &str,
    owner: &str,
    workspace: &str,
    request_id: &str,
    admission: Access,
    job_id: Option<&str>,
) -> Result<Option<crate::generation_projects::Lease>, ApiError> {
    job_id
        .map(|id| {
            let key = execution_key(actor, owner, workspace, request_id, admission, id)?;
            state.project_jobs.start(actor, workspace, &key)
        })
        .transpose()
}
pub(super) async fn cancelable<T>(
    lease: Option<crate::generation_projects::Lease>,
    work: impl Future<Output = Result<T, ApiError>>,
    cancelled_check: impl Future<Output = Result<(), ApiError>>,
) -> Result<T, ApiError> {
    let Some(lease) = lease else {
        return work.await;
    };
    let result = tokio::select! {biased;_=lease.cancel.cancelled()=>Err(ApiError::bad("Dubbo operation stopped locally; remote outcome may be unknown")),result=work=>result};
    if lease.cancel.is_cancelled() {
        cancelled_check.await?;
        return Err(ApiError::bad(
            "Dubbo operation stopped locally; remote outcome may be unknown",
        ));
    }
    result
}
pub async fn shared_dubbo_cancel(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    headers: HeaderMap,
    Path((owner, workspace)): Path<(String, String)>,
    Json(input): Json<DubboCancel>,
) -> Result<Json<Value>, ApiError> {
    let (canonical, _, _, admission, _) = admit(
        &state,
        &actor.0,
        &owner,
        &workspace,
        &input.request_id,
        input.expected_revision,
        "dubbo.cancel",
    )
    .await?;
    finish_specification_preview(
        &state.db,
        &actor.0,
        &owner,
        &workspace,
        (admission, canonical.revision),
        crate::auth::still_authenticated(&state, &actor.0, &headers),
    )
    .await?;
    let id = execution_key(
        &actor.0,
        &owner,
        &workspace,
        &input.request_id,
        admission,
        &input.job_id,
    )?;
    state
        .project_jobs
        .cancel_job(&actor.0, Some(&workspace), &id)?;
    Ok(Json(json!({"cancelled":true})))
}
