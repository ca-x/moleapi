use crate::{ApiError, history, privacy::HistoryPrivacy};
use moleapi_core::{HistoryEntry, RunnerSelection, VariableScopes, Workspace};
use sea_orm::ConnectionTrait;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeSet;

const MAX_UPLOAD_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Upload {
    position: usize,
    entry: HistoryEntry,
}

pub(super) async fn persist<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    workspace: &Workspace,
    selection: &RunnerSelection,
    scopes: &VariableScopes,
    report: &mut Value,
    allowed: &BTreeSet<&str>,
) -> Result<(), ApiError> {
    let object = report
        .as_object_mut()
        .ok_or_else(|| ApiError::bad("Runner report must be an object"))?;
    let uploads = object
        .remove("history_uploads")
        .unwrap_or(Value::Array(vec![]));
    object.remove("history_upload_omitted");
    if !uploads.is_array()
        || serde_json::to_vec(&uploads)
            .map_err(|_| ApiError::internal())?
            .len()
            > MAX_UPLOAD_BYTES
    {
        return Err(ApiError::bad("Runner response history exceeds 8 MiB"));
    }
    let uploads: Vec<Upload> = serde_json::from_value(uploads)
        .map_err(|_| ApiError::bad("Invalid runner response history"))?;
    if uploads.len() > 1000 {
        return Err(ApiError::bad(
            "Runner response history exceeds 1000 entries",
        ));
    }
    let results = report["results"]
        .as_array_mut()
        .ok_or_else(|| ApiError::bad("Report results must be an array"))?;
    let mut local_ids = BTreeSet::new();
    let mut positions = BTreeSet::new();
    for upload in &uploads {
        let item = results
            .get(upload.position)
            .ok_or_else(|| ApiError::bad("Response history position is outside the report"))?;
        if !positions.insert(upload.position)
            || !local_ids.insert(upload.entry.id.clone())
            || item["history_id"].as_str() != Some(upload.entry.id.as_str())
            || item["request_id"].as_str() != Some(upload.entry.request_id.as_str())
            || upload.entry.workspace_id != workspace.id
            || !allowed.contains(upload.entry.request_id.as_str())
            || upload.entry.response.skipped
            || item["status"]
                .as_u64()
                .is_some_and(|status| status != u64::from(upload.entry.response.status))
        {
            return Err(ApiError::bad(
                "Runner response history does not match its report step",
            ));
        }
    }
    for item in results.iter_mut() {
        item.as_object_mut()
            .ok_or_else(|| ApiError::bad("Report result must be an object"))?
            .remove("history_id");
    }
    let environment =
        crate::execution::environment(workspace, selection.environment_id.as_deref())?;
    for upload in uploads {
        let request = workspace
            .data
            .collections
            .iter()
            .flat_map(|collection| &collection.requests)
            .find(|request| request.id == upload.entry.request_id)
            .ok_or_else(|| ApiError::bad("Runner response request does not exist"))?;
        let uncertain = upload
            .entry
            .response
            .tests
            .iter()
            .any(|test| test.id == "post-script-error");
        let projected = history::project(
            workspace,
            request,
            environment,
            &upload.entry.response,
            HistoryPrivacy {
                values: &scopes.private_values,
                redact_failed_response: uncertain,
            },
        )?;
        history::insert_in(db, owner, &projected).await?;
        results[upload.position]["history_id"] = projected.id.into();
    }
    Ok(())
}
