use super::*;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphqlSource {
    expected_revision: i64,
    request_id: String,
    specification_id: Option<String>,
}
async fn admit(
    state: &AppState,
    actor: &str,
    owner: &str,
    workspace: &str,
    input: &GraphqlSource,
    action: &str,
) -> Result<(Workspace, moleapi_core::RequestSpec, Access), ApiError> {
    if state.local {
        return Err(ApiError::not_found());
    }
    let tx = state.db.begin().await?;
    let canonical = lock_workspace(&tx, owner, workspace).await?;
    let (_, admission) = access(&tx, actor, owner, workspace).await?;
    if admission.role == Role::Viewer {
        return Err(ApiError::forbidden("Viewer cannot inspect GraphQL schemas"));
    }
    if canonical.revision != input.expected_revision {
        return Err(ApiError::conflict(
            "Workspace changed before GraphQL schema operation",
        ));
    }
    let request = canonical
        .data
        .collections
        .iter()
        .flat_map(|collection| &collection.requests)
        .find(|request| request.id == input.request_id && request.protocol.is_graphql())
        .cloned()
        .ok_or_else(ApiError::not_found)?;
    record_event_result(
        &tx,
        owner,
        workspace,
        actor,
        action,
        Some(&request.id),
        "started",
    )
    .await?;
    tx.commit().await?;
    Ok((canonical, request, admission))
}
pub async fn shared_graphql_introspect(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    headers: HeaderMap,
    Path((owner, workspace)): Path<(String, String)>,
    Json(input): Json<GraphqlSource>,
) -> Result<Json<crate::graphql::IntrospectionResult>, ApiError> {
    let (canonical, request, admission) = admit(
        &state,
        &actor.0,
        &owner,
        &workspace,
        &input,
        "graphql.introspect",
    )
    .await?;
    let operation = crate::graphql::introspect(
        State(state.clone()),
        Extension(Identity(owner.clone())),
        Json(crate::graphql::Introspect {
            workspace_id: workspace.clone(),
            request,
            environment_id: canonical.data.active_environment_id.clone(),
            variables: vec![],
            data: vec![],
            locals: vec![],
        }),
    );
    let watch = watch_access_revision(
        &state,
        &actor.0,
        &owner,
        &workspace,
        admission,
        canonical.revision,
        "Workspace changed during GraphQL introspection",
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
pub async fn shared_graphql_schema(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    headers: HeaderMap,
    Path((owner, workspace)): Path<(String, String)>,
    Json(input): Json<GraphqlSource>,
) -> Result<Json<crate::graphql::SchemaResult>, ApiError> {
    let (canonical, request, admission) = admit(
        &state,
        &actor.0,
        &owner,
        &workspace,
        &input,
        "graphql.schema.read",
    )
    .await?;
    let specification_id = input
        .specification_id
        .filter(|id| request.specification_id.as_ref() == Some(id))
        .ok_or_else(ApiError::not_found)?;
    let operation = crate::graphql::schema(
        State(state.clone()),
        Extension(Identity(owner.clone())),
        Json(crate::graphql::SchemaSource {
            workspace_id: workspace.clone(),
            specification_id,
        }),
    );
    let watch = watch_access_revision(
        &state,
        &actor.0,
        &owner,
        &workspace,
        admission,
        canonical.revision,
        "Workspace changed during GraphQL schema read",
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
