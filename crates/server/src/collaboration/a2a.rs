use super::*;
use std::collections::BTreeSet;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardImport {
    request_id: String,
    expected_revision: i64,
    source: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardRead {
    request_id: String,
    expected_revision: i64,
    specification_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardDiscover {
    request_id: String,
    expected_revision: i64,
    discovery_id: String,
    url: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardCancel {
    request_id: String,
    expected_revision: i64,
    discovery_id: String,
}
async fn admit(
    state: &AppState,
    actor: &str,
    owner: &str,
    workspace: &str,
    request_id: &str,
    revision: i64,
    action: &str,
) -> Result<
    (
        Workspace,
        moleapi_core::RequestSpec,
        Access,
        BTreeSet<String>,
    ),
    ApiError,
> {
    if state.local {
        return Err(ApiError::not_found());
    }
    let tx = state.db.begin().await?;
    let canonical = lock_workspace(&tx, owner, workspace).await?;
    let (_, admission) = access(&tx, actor, owner, workspace).await?;
    if admission.role == Role::Viewer {
        return Err(ApiError::forbidden("Viewer cannot inspect Agent Cards"));
    }
    if canonical.revision != revision {
        return Err(ApiError::conflict(
            "Workspace changed before Agent Card operation",
        ));
    }
    let collection = canonical
        .data
        .collections
        .iter()
        .find(|c| {
            c.requests
                .iter()
                .any(|r| r.id == request_id && r.protocol.is_a2a())
        })
        .ok_or_else(ApiError::not_found)?;
    let request = collection
        .requests
        .iter()
        .find(|r| r.id == request_id)
        .cloned()
        .ok_or_else(ApiError::not_found)?;
    let environment =
        crate::execution::environment(&canonical, canonical.data.active_environment_id.as_deref())?;
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
    for parent in moleapi_core::collection_chain(&canonical.data, collection)
        .map_err(|e| ApiError::bad(e.to_string()))?
    {
        if let Some(auth) = &parent.auth {
            let mut inherited = request.clone();
            inherited.auth = auth.clone();
            crate::privacy::request_values(&inherited, &mut scopes)?;
        }
    }
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
    Ok((canonical, request, admission, scopes.private_values))
}
async fn finish<F>(
    state: &AppState,
    actor: &str,
    owner: &str,
    workspace: &str,
    headers: &HeaderMap,
    context: (Access, i64),
    operation: F,
) -> Result<Json<crate::a2a::Candidate>, ApiError>
where
    F: Future<Output = Result<Json<crate::a2a::Candidate>, ApiError>>,
{
    let (admission, revision) = context;
    let watch =
        watch_authenticated_revision(state, actor, owner, workspace, admission, revision, headers);
    let check = finish_specification_preview(
        &state.db,
        actor,
        owner,
        workspace,
        (admission, revision),
        crate::auth::still_authenticated(state, actor, headers),
    );
    with_access_watch(operation, watch, check).await
}
fn dialect(request: &moleapi_core::RequestSpec) -> Result<&str, ApiError> {
    match &request.protocol {
        moleapi_core::Protocol::A2a { config } => Ok(&config.dialect),
        _ => Err(ApiError::not_found()),
    }
}
pub async fn shared_a2a_import(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    headers: HeaderMap,
    Path((owner, workspace)): Path<(String, String)>,
    Json(input): Json<CardImport>,
) -> Result<Json<crate::a2a::Candidate>, ApiError> {
    let (canonical, request, admission, private) = admit(
        &state,
        &actor.0,
        &owner,
        &workspace,
        &input.request_id,
        input.expected_revision,
        "a2a.card.import",
    )
    .await?;
    finish(
        &state,
        &actor.0,
        &owner,
        &workspace,
        &headers,
        (admission, canonical.revision),
        async { crate::a2a::candidate(input.source, dialect(&request)?, &private).map(Json) },
    )
    .await
}
pub async fn shared_a2a_read(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    headers: HeaderMap,
    Path((owner, workspace)): Path<(String, String)>,
    Json(input): Json<CardRead>,
) -> Result<Json<crate::a2a::Candidate>, ApiError> {
    let (canonical, request, admission, private) = admit(
        &state,
        &actor.0,
        &owner,
        &workspace,
        &input.request_id,
        input.expected_revision,
        "a2a.card.read",
    )
    .await?;
    if request.specification_id.as_deref() != Some(&input.specification_id) {
        return Err(ApiError::not_found());
    }
    let spec = canonical
        .data
        .specifications
        .iter()
        .find(|s| {
            s.id == input.specification_id
                && s.kind == "a2a-agent-card"
                && s.dialect == format!("a2a-{}", dialect(&request).unwrap_or_default())
        })
        .ok_or_else(ApiError::not_found)?;
    finish(
        &state,
        &actor.0,
        &owner,
        &workspace,
        &headers,
        (admission, canonical.revision),
        async {
            let mut value =
                crate::a2a::candidate(spec.source.clone(), dialect(&request)?, &private)?;
            value.specification = spec.clone();
            Ok(Json(value))
        },
    )
    .await
}
pub async fn shared_a2a_discover(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    headers: HeaderMap,
    Path((owner, workspace)): Path<(String, String)>,
    Json(input): Json<CardDiscover>,
) -> Result<Json<crate::a2a::Candidate>, ApiError> {
    let (canonical, mut request, admission, private) = admit(
        &state,
        &actor.0,
        &owner,
        &workspace,
        &input.request_id,
        input.expected_revision,
        "a2a.card.discover",
    )
    .await?;
    let lease = state.a2a_sources.start(&actor.0, &input.discovery_id)?;
    let operation = async {
        let collection = canonical
            .data
            .collections
            .iter()
            .find(|c| c.requests.iter().any(|r| r.id == input.request_id))
            .ok_or_else(ApiError::not_found)?;
        let environment = crate::execution::environment(
            &canonical,
            canonical.data.active_environment_id.as_deref(),
        )?;
        let mut scopes = crate::execution::variables(
            &state,
            &canonical,
            Some(collection),
            environment,
            &[],
            &[],
            &[],
        )?;
        scopes.private_values.extend(private);
        request.url = input.url;
        let version = dialect(&request)?.to_owned();
        let (request, _, _, _) = crate::execution::prepare_live(
            &state,
            &owner,
            &canonical,
            &request,
            collection,
            &mut scopes,
        )
        .await?;
        let source = crate::a2a::fetch_card(&state, request).await?;
        crate::a2a::candidate(source, &version, &scopes.private_values).map(Json)
    };
    let operation = async {
        tokio::select! {biased;_=lease.token.cancelled()=>Err(ApiError::bad("Agent Card discovery stopped locally")),result=operation=>result}
    };
    lease
        .finish(finish(
            &state,
            &actor.0,
            &owner,
            &workspace,
            &headers,
            (admission, canonical.revision),
            operation,
        ))
        .await
}
pub async fn shared_a2a_cancel(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    headers: HeaderMap,
    Path((owner, workspace)): Path<(String, String)>,
    Json(input): Json<CardCancel>,
) -> Result<Json<Value>, ApiError> {
    let (canonical, _, admission, _) = admit(
        &state,
        &actor.0,
        &owner,
        &workspace,
        &input.request_id,
        input.expected_revision,
        "a2a.card.stop",
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
    state.a2a_sources.cancel(&actor.0, &input.discovery_id);
    Ok(Json(json!({"stopped":true})))
}
