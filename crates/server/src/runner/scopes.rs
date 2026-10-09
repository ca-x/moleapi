use crate::{ApiError, AppState, execution::variables};
use moleapi_core::{Collection, Environment, Pair, VariableScopes, Workspace};
use std::collections::{BTreeMap, BTreeSet};
pub(super) type Overlays = BTreeMap<String, BTreeMap<String, Option<String>>>;
pub(super) struct Inputs<'a> {
    pub state: &'a AppState,
    pub workspace: &'a Workspace,
    pub environment: Option<&'a Environment>,
    pub data: &'a [Pair],
    pub temporary: &'a [Pair],
    pub overlays: &'a Overlays,
}
pub(super) fn prepare(
    inputs: &Inputs<'_>,
    selected: &Collection,
    scopes: &mut VariableScopes,
) -> Result<(), ApiError> {
    let Inputs {
        state,
        workspace,
        environment,
        data,
        temporary,
        overlays,
    } = inputs;
    let base = variables(
        state,
        workspace,
        Some(selected),
        *environment,
        data,
        temporary,
        &[],
    )?;
    scopes.private_values.extend(base.private_values);
    scopes.collection.clear();
    for ancestor in moleapi_core::collection_chain(&workspace.data, selected)
        .map_err(|error| ApiError::bad(error.to_string()))?
    {
        for pair in ancestor
            .variables
            .iter()
            .filter(|pair| pair.enabled && ancestor.variables_enabled != Some(false))
        {
            scopes.collection.insert(
                pair.key.clone(),
                if state.local {
                    pair.local_value.as_ref().unwrap_or(&pair.value)
                } else {
                    &pair.value
                }
                .clone(),
            );
        }
        if let Some(overlay) = overlays.get(&ancestor.id) {
            for (key, value) in overlay {
                if let Some(value) = value {
                    scopes.collection.insert(key.clone(), value.clone());
                } else {
                    scopes.collection.remove(key);
                }
            }
        }
    }
    scopes
        .validate()
        .map_err(|error| ApiError::bad(error.to_string()))
}
pub(super) fn capture(
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
    collection: &str,
    overlays: &mut Overlays,
) {
    for key in before.keys().chain(after.keys()).collect::<BTreeSet<_>>() {
        if before.get(key) != after.get(key) {
            overlays
                .entry(collection.into())
                .or_default()
                .insert(key.clone(), after.get(key).cloned());
        }
    }
}
pub(super) fn validate_overlays(overlays: &Overlays) -> bool {
    overlays
        .values()
        .flat_map(|values| values.iter())
        .map(|(key, value)| key.len() + value.as_ref().map_or(0, String::len))
        .sum::<usize>()
        <= moleapi_core::MAX_VARIABLE_BYTES
}
