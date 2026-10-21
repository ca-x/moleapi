use super::*;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrpcSource {
    expected_revision: i64,
    request_id: String,
    specification_id: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrpcImportSource {
    expected_revision: i64,
    request_id: String,
    name: String,
    source: String,
}
async fn admit(
    state: &AppState,
    actor: &str,
    owner: &str,
    workspace: &str,
    request_id: &str,
    revision: i64,
    action: &str,
) -> Result<(Workspace, moleapi_core::RequestSpec, Access), ApiError> {
    if state.local {
        return Err(ApiError::not_found());
    }
    let tx = state.db.begin().await?;
    let canonical = lock_workspace(&tx, owner, workspace).await?;
    let (_, admission) = access(&tx, actor, owner, workspace).await?;
    if admission.role == Role::Viewer {
        return Err(ApiError::forbidden(
            "Viewer cannot inspect gRPC definitions",
        ));
    }
    if canonical.revision != revision {
        return Err(ApiError::conflict(
            "Workspace changed before gRPC definition operation",
        ));
    }
    let request = canonical
        .data
        .collections
        .iter()
        .flat_map(|c| &c.requests)
        .find(|r| r.id == request_id && matches!(r.protocol, moleapi_core::Protocol::Grpc { .. }))
        .cloned()
        .ok_or_else(ApiError::not_found)?;
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
    Ok((canonical, request, admission))
}
async fn watch(
    state: &AppState,
    actor: &str,
    owner: &str,
    workspace: &str,
    admission: Access,
    revision: i64,
    headers: &HeaderMap,
) -> ApiError {
    let period = std::time::Duration::from_millis(200);
    let mut ticks = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        ticks.tick().await;
        if let Err(error) = crate::auth::still_authenticated(state, actor, headers).await {
            return error;
        }
        match access(&state.db, actor, owner, workspace).await {
            Ok((current, member)) if member == admission => {
                if current.revision != revision {
                    return ApiError::conflict(
                        "Workspace changed during gRPC definition operation",
                    );
                }
            }
            _ => return ApiError::not_found(),
        }
    }
}
pub async fn shared_grpc_schema(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    headers: HeaderMap,
    Path((owner, workspace)): Path<(String, String)>,
    Json(input): Json<GrpcSource>,
) -> Result<Json<crate::grpc::SchemaResult>, ApiError> {
    let (canonical, request, admission) = admit(
        &state,
        &actor.0,
        &owner,
        &workspace,
        &input.request_id,
        input.expected_revision,
        "grpc.schema.read",
    )
    .await?;
    let specification_id = input
        .specification_id
        .filter(|id| request.specification_id.as_ref() == Some(id))
        .ok_or_else(ApiError::not_found)?;
    let operation = crate::grpc::schema(
        State(state.clone()),
        Extension(Identity(owner.clone())),
        Json(crate::grpc::SchemaSource {
            workspace_id: workspace.clone(),
            specification_id,
        }),
    );
    let watcher = watch(
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
    with_access_watch(operation, watcher, check).await
}
pub async fn shared_grpc_import(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    headers: HeaderMap,
    Path((owner, workspace)): Path<(String, String)>,
    Json(input): Json<GrpcImportSource>,
) -> Result<Json<crate::grpc::SchemaResult>, ApiError> {
    let (canonical, _, admission) = admit(
        &state,
        &actor.0,
        &owner,
        &workspace,
        &input.request_id,
        input.expected_revision,
        "grpc.import",
    )
    .await?;
    let operation = crate::grpc::import(
        State(state.clone()),
        Extension(Identity(owner.clone())),
        Json(crate::grpc::Import {
            workspace_id: workspace.clone(),
            name: input.name,
            source: input.source,
        }),
    );
    let watcher = watch(
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
    with_access_watch(operation, watcher, check).await
}
pub async fn shared_grpc_reflect(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    headers: HeaderMap,
    Path((owner, workspace)): Path<(String, String)>,
    Json(input): Json<GrpcSource>,
) -> Result<Json<crate::grpc::ReflectResult>, ApiError> {
    let (canonical, request, admission) = admit(
        &state,
        &actor.0,
        &owner,
        &workspace,
        &input.request_id,
        input.expected_revision,
        "grpc.reflect",
    )
    .await?;
    let operation = crate::grpc::reflect_as(
        state.clone(),
        Identity(owner.clone()),
        &actor.0,
        headers.clone(),
        crate::grpc::Reflect {
            workspace_id: workspace.clone(),
            request,
            environment_id: canonical.data.active_environment_id.clone(),
            locals: Vec::new(),
        },
    );
    let watcher = watch(
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
    with_access_watch(operation, watcher, check).await
}
