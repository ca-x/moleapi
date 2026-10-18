//! Resolve a saved read-only Data request into one run-local dataset snapshot.
use crate::{ApiError, AppState, execution, workspaces::owned};
use moleapi_core::{
    Dataset, Environment, NetworkPolicy, Pair, Protocol, SavedQueryDataset, VariableUpdate,
    Workspace,
};
use std::time::Duration;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

pub(crate) struct Refresh<'a> {
    pub state: &'a AppState,
    pub owner: &'a str,
    pub workspace: &'a Workspace,
    pub query: &'a SavedQueryDataset,
    pub environment: Option<&'a Environment>,
    pub data: &'a [Pair],
    pub temporary: &'a [Pair],
    pub locals: &'a [VariableUpdate],
    pub cancel: &'a CancellationToken,
    pub deadline: Instant,
    pub allow_empty: bool,
}

pub(crate) async fn refresh(input: Refresh<'_>) -> Result<Dataset, ApiError> {
    let Refresh {
        state,
        owner,
        workspace,
        query,
        environment,
        data,
        temporary,
        locals,
        cancel,
        deadline,
        allow_empty,
    } = input;
    let (collection, request) = workspace
        .data
        .collections
        .iter()
        .find_map(|collection| {
            collection
                .requests
                .iter()
                .find(|request| request.id == query.request_id)
                .map(|request| (collection, request))
        })
        .ok_or_else(ApiError::not_found)?;
    let Protocol::Data { config } = &request.protocol else {
        return Err(ApiError::bad(
            "Dataset query no longer references a Data request",
        ));
    };
    if !config.read_only || config.sql.trim().is_empty() {
        return Err(ApiError::bad("Dataset query requires saved read-only SQL"));
    }
    let mut scopes = execution::variables(
        state,
        workspace,
        Some(collection),
        environment,
        data,
        temporary,
        locals,
    )?;
    let (prepared, _, _, _) = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(ApiError::bad("Dataset refresh cancelled")),
        _ = tokio::time::sleep_until(deadline) => return Err(ApiError::bad("Dataset refresh deadline exceeded")),
        outcome = execution::prepare_live(state, owner, workspace, request, collection, &mut scopes) => outcome?,
    };
    let Protocol::Data { config } = &prepared.protocol else {
        return Err(ApiError::bad(
            "Dataset pre-script changed the Data protocol",
        ));
    };
    if !config.read_only {
        return Err(ApiError::bad(
            "Dataset pre-script changed read-only Data settings",
        ));
    }
    let selected_sql = config.sql.clone();
    let sql = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(ApiError::bad("Dataset refresh cancelled")),
        _ = tokio::time::sleep_until(deadline) => return Err(ApiError::bad("Dataset refresh deadline exceeded")),
        outcome = execution::prepare_data_query(state, owner, workspace, &prepared, &selected_sql, true, &mut scopes) => outcome?.sql.ok_or_else(|| ApiError::bad("Dataset query was skipped"))?,
    };
    if cancel.is_cancelled() || Instant::now() >= deadline {
        return Err(ApiError::bad("Dataset refresh cancelled or timed out"));
    }
    let policy = NetworkPolicy {
        allow_private_network: state.local || state.config.allow_private_network,
    };
    let duration = Duration::from_millis(prepared.timeout_ms)
        .min(deadline.saturating_duration_since(Instant::now()));
    let mut connection = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(ApiError::bad("Dataset refresh cancelled")),
        result = tokio::time::timeout(duration, moleapi_data::Connection::open(&prepared, config, policy, &state.script_worker)) => result.map_err(|_| ApiError::bad("Dataset connection timed out"))?.map_err(|_| ApiError::bad("Dataset connection failed"))?,
    };
    let cancellation = connection.cancellation();
    let duration = Duration::from_millis(prepared.timeout_ms)
        .min(deadline.saturating_duration_since(Instant::now()));
    let outcome = tokio::select! {
        biased;
        _ = cancel.cancelled() => None,
        outcome = tokio::time::timeout(duration, connection.query(&sql, true, duration)) => Some(outcome),
    };
    let result = match outcome {
        None => {
            let _ = tokio::time::timeout(Duration::from_secs(1), cancellation.request()).await;
            return Err(ApiError::bad("Dataset refresh cancelled"));
        }
        Some(Err(_)) => {
            let _ = tokio::time::timeout(Duration::from_secs(1), cancellation.request()).await;
            return Err(ApiError::bad("Dataset query timed out"));
        }
        Some(Ok(Err(_))) => return Err(ApiError::bad("Dataset query failed")),
        Some(Ok(Ok(result))) => result,
    };
    let dataset = if allow_empty && result.rows.is_empty() {
        let mut names = std::collections::BTreeSet::new();
        if result.truncated
            || !(1..=100).contains(&result.columns.len())
            || result.columns.iter().any(|column| {
                column.name.trim().is_empty()
                    || column.name.len() > 1024
                    || column.name.contains('\0')
                    || !names.insert(column.name.clone())
            })
        {
            return Err(ApiError::bad("Empty Data dataset has invalid columns"));
        }
        Dataset {
            rows: Vec::new(),
            columns: result
                .columns
                .iter()
                .map(|column| column.name.clone())
                .collect(),
        }
    } else {
        let source = moleapi_data::dataset_source(&result)
            .map_err(|error| ApiError::bad(error.to_string()))?;
        source
            .parse()
            .map_err(|error| ApiError::bad(error.to_string()))?
    };
    if cancel.is_cancelled() || Instant::now() >= deadline {
        return Err(ApiError::bad("Dataset refresh cancelled or timed out"));
    }
    let current = owned(state, owner, &workspace.id).await?;
    if current.revision != workspace.revision {
        return Err(ApiError::conflict(
            "Dataset source workspace changed during refresh",
        ));
    }
    Ok(dataset)
}
