use crate::{ApiError, AppState, auth::Identity, workspaces::owned};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use moleapi_formats::ci::{Config, Notifications, Preset, Source};
use serde::Deserialize;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Generate {
    expected_revision: i64,
    config: Config,
}
pub async fn generate(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
    Json(input): Json<Generate>,
) -> Result<Json<Preset>, ApiError> {
    let workspace = owned(&state, &owner.0, &id).await?;
    if workspace.revision != input.expected_revision {
        return Err(ApiError::conflict("Workspace revision changed"));
    }
    let config = input.config;
    config
        .arguments()
        .map_err(|error| ApiError::bad(error.to_string()))?;
    match &config.source {
        Source::Remote {
            workspace: target, ..
        } if target != &id => {
            return Err(ApiError::bad("CI workspace must match the owned source"));
        }
        Source::File { format, .. } if format != "moleapi" => {
            return Err(ApiError::bad(
                "Workspace file presets require the native export format",
            ));
        }
        _ => {}
    }
    let scenario = config
        .scenario
        .as_ref()
        .map(|id| {
            workspace
                .data
                .scenarios
                .iter()
                .find(|scenario| scenario.id == *id)
                .ok_or_else(ApiError::not_found)
        })
        .transpose()?;
    let collection = config
        .collection
        .as_deref()
        .or_else(|| scenario.map(|scenario| scenario.collection_id.as_str()))
        .ok_or_else(ApiError::not_found)?;
    let collection = workspace
        .data
        .collections
        .iter()
        .find(|value| value.id == collection)
        .ok_or_else(ApiError::not_found)?;
    let subtree = moleapi_core::collection_subtree(&workspace.data, collection)
        .map_err(|error| ApiError::bad(error.to_string()))?;
    let mut selected = std::collections::BTreeSet::new();
    for id in &config.requests {
        if !selected.insert(id)
            || !subtree
                .iter()
                .any(|collection| collection.requests.iter().any(|request| request.id == *id))
        {
            return Err(ApiError::bad(
                "Request filters must be distinct and within the selected subtree",
            ));
        }
    }
    if config.environment.as_ref().is_some_and(|id| {
        !workspace
            .data
            .environments
            .iter()
            .any(|value| value.id == *id)
    }) || config
        .dataset
        .as_ref()
        .is_some_and(|id| !workspace.data.datasets.iter().any(|value| value.id == *id))
    {
        return Err(ApiError::not_found());
    }
    if let Some(id) = &config.dataset {
        let source = workspace
            .data
            .datasets
            .iter()
            .find(|value| value.id == *id)
            .and_then(|value| value.source.as_ref())
            .ok_or_else(|| ApiError::bad("Saved dataset source is missing"))?;
        let parsed = source
            .parse()
            .map_err(|error| ApiError::bad(error.to_string()))?;
        if config
            .iterations
            .is_some_and(|count| usize::from(count) > parsed.rows.len())
        {
            return Err(ApiError::bad("Iterations exceed selected dataset rows"));
        }
        if matches!(config.source, Source::File { .. }) {
            let exported = moleapi_formats::export(&workspace, "moleapi", false)
                .map_err(|error| ApiError::bad(error.to_string()))?;
            let projected: moleapi_core::Workspace =
                serde_json::from_str(&exported.content).map_err(|_| ApiError::internal())?;
            if !projected
                .data
                .datasets
                .iter()
                .any(|value| value.id == *id && value.source.is_some())
            {
                return Err(ApiError::bad(
                    "Dataset source is withheld by private export; supply a prepared repository data file",
                ));
            }
        }
    }
    let notification_ids = match &config.notifications {
        Notifications::Silent => &[][..],
        Notifications::Defaults => {
            scenario.map_or(&[][..], |value| value.notification_ids.as_slice())
        }
        Notifications::Selected { ids } => ids.as_slice(),
    };
    crate::notifications::validate_targets(&state.db, &owner.0, &id, notification_ids).await?;
    Ok(Json(
        moleapi_formats::ci::generate(&config).map_err(|error| ApiError::bad(error.to_string()))?,
    ))
}
