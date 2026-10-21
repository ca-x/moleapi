use super::*;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpConfigExport {
    request_id: String,
    expected_revision: i64,
    #[serde(default)]
    include_secrets: bool,
    host_name: Option<String>,
}
pub async fn shared_mcp_config_export(
    State(state): State<AppState>,
    Extension(actor): Extension<Identity>,
    headers: HeaderMap,
    Path((owner, workspace)): Path<(String, String)>,
    Json(input): Json<McpConfigExport>,
) -> Result<Json<moleapi_core::RequestSpec>, ApiError> {
    if state.local {
        return Err(ApiError::not_found());
    }
    let tx = state.db.begin().await?;
    let canonical = lock_workspace(&tx, &owner, &workspace).await?;
    let (_, admission) = access(&tx, &actor.0, &owner, &workspace).await?;
    if admission.role == Role::Viewer {
        return Err(ApiError::forbidden(
            "Viewer cannot export MCP configurations",
        ));
    }
    if canonical.revision != input.expected_revision {
        return Err(ApiError::conflict(
            "Workspace changed before MCP config export",
        ));
    }
    if !canonical
        .data
        .collections
        .iter()
        .flat_map(|c| &c.requests)
        .any(|r| r.id == input.request_id && r.protocol.is_mcp())
    {
        return Err(ApiError::not_found());
    }
    record_event_result(
        &tx,
        &owner,
        &workspace,
        &actor.0,
        "mcp.config.export",
        Some(&input.request_id),
        "started",
    )
    .await?;
    tx.commit().await?;
    let revision = canonical.revision;
    let operation = async {
        tokio::task::spawn_blocking(move || {
            crate::mcp_config::export_request(
                canonical,
                crate::mcp_config::McpConfigExport {
                    request_id: input.request_id,
                    include_secrets: input.include_secrets,
                    host_name: input.host_name,
                },
            )
        })
        .await
        .map_err(|_| ApiError::internal())?
        .map(Json)
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
