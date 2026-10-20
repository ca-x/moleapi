use super::*;
use std::collections::BTreeSet;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SoapSource {
    expected_revision: i64,
    request_id: String,
    specification_id: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SoapImport {
    expected_revision: i64,
    request_id: String,
    name: String,
    source: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SoapUrl {
    expected_revision: i64,
    request_id: String,
    name: String,
    url: String,
    verify_tls: bool,
    timeout_ms: u64,
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
        return Err(ApiError::forbidden(
            "Viewer cannot inspect SOAP definitions",
        ));
    }
    if canonical.revision != revision {
        return Err(ApiError::conflict(
            "Workspace changed before SOAP definition operation",
        ));
    }
    let collection = canonical
        .data
        .collections
        .iter()
        .find(|c| {
            c.requests
                .iter()
                .any(|r| r.id == request_id && r.protocol.is_soap())
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
fn candidate(
    name: String,
    source: String,
    private: &BTreeSet<String>,
) -> Result<Json<crate::soap::SchemaResult>, ApiError> {
    let specification = moleapi_core::Specification {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        source,
        kind: "wsdl".into(),
        dialect: "wsdl1.1".into(),
    };
    crate::soap::screen(&specification, private)?;
    Ok(Json(crate::soap::compile(specification)?))
}
pub async fn shared_soap_import(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    headers: HeaderMap,
    Path((owner, workspace)): Path<(String, String)>,
    Json(input): Json<SoapImport>,
) -> Result<Json<crate::soap::SchemaResult>, ApiError> {
    let (canonical, _, admission, private) = admit(
        &state,
        &actor.0,
        &owner,
        &workspace,
        &input.request_id,
        input.expected_revision,
        "soap.import",
    )
    .await?;
    let operation = async { candidate(input.name, input.source, &private) };
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
pub async fn shared_soap_import_url(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    headers: HeaderMap,
    Path((owner, workspace)): Path<(String, String)>,
    Json(input): Json<SoapUrl>,
) -> Result<Json<crate::soap::SchemaResult>, ApiError> {
    let (canonical, _, admission, private) = admit(
        &state,
        &actor.0,
        &owner,
        &workspace,
        &input.request_id,
        input.expected_revision,
        "soap.import_url",
    )
    .await?;
    let operation = async {
        let source =
            crate::soap::fetch_source(&state, &input.url, input.verify_tls, input.timeout_ms)
                .await?;
        candidate(input.name, source, &private)
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
pub async fn shared_soap_schema(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    headers: HeaderMap,
    Path((owner, workspace)): Path<(String, String)>,
    Json(input): Json<SoapSource>,
) -> Result<Json<crate::soap::SchemaResult>, ApiError> {
    let (canonical, request, admission, private) = admit(
        &state,
        &actor.0,
        &owner,
        &workspace,
        &input.request_id,
        input.expected_revision,
        "soap.schema.read",
    )
    .await?;
    let id = input
        .specification_id
        .filter(|id| request.specification_id.as_ref() == Some(id))
        .ok_or_else(ApiError::not_found)?;
    let specification = canonical
        .data
        .specifications
        .iter()
        .find(|s| s.id == id && s.kind == "wsdl")
        .cloned()
        .ok_or_else(ApiError::not_found)?;
    let operation = async {
        crate::soap::screen(&specification, &private)?;
        Ok(Json(crate::soap::compile(specification)?))
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
