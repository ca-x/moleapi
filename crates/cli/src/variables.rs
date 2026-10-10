//! Private run overlays compile to the existing core models; no separate execution engine.
use crate::io;
use anyhow::{Context, Result, ensure};
use moleapi_core::{Pair, VariableScopes, VariableUpdate};
use serde::Deserialize;
use std::{collections::BTreeMap, path::Path};
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Inputs {
    temporary: BTreeMap<String, String>,
    project: BTreeMap<String, Option<String>>,
    collection: BTreeMap<String, Option<String>>,
    environment: BTreeMap<String, Option<String>>,
}
#[derive(Default)]
pub struct Overrides {
    pub temporary: Vec<Pair>,
    pub locals: Vec<VariableUpdate>,
}
pub fn read(path: Option<&Path>, environment: Option<&str>) -> Result<Overrides> {
    let raw = if let Some(path) = path {
        Some(io::read(path, moleapi_core::MAX_VARIABLE_BYTES)?)
    } else if let Some(name) = environment {
        Some(std::env::var(name).context("Run variable environment input is unavailable")?)
    } else {
        None
    };
    let Some(raw) = raw else {
        return Ok(Overrides::default());
    };
    ensure!(
        raw.len() <= moleapi_core::MAX_VARIABLE_BYTES,
        "Run variable input exceeds 1 MiB"
    );
    let input: Inputs =
        serde_json::from_str(&raw).context("Cannot parse private run variable input")?;
    let temporary = input
        .temporary
        .into_iter()
        .enumerate()
        .map(|(index, (key, value))| Pair {
            id: format!("cli-variable-{index}"),
            key,
            value,
            enabled: true,
            secret: Some(true),
            local_value: None,
        })
        .collect::<Vec<_>>();
    let mut locals = vec![];
    for (scope, values) in [
        ("project", input.project),
        ("collection", input.collection),
        ("environment", input.environment),
    ] {
        locals.extend(values.into_iter().map(|(key, value)| VariableUpdate {
            scope: scope.into(),
            key,
            value,
        }));
    }
    // Shared count/key/value/privacy limits apply before any run/import API request.
    moleapi_core::validate_variables(&temporary).context("Invalid private temporary variables")?;
    let mut scopes = VariableScopes::default();
    scopes.temporary = temporary
        .iter()
        .map(|pair| (pair.key.clone(), pair.value.clone()))
        .collect();
    scopes
        .private_values
        .extend(temporary.iter().map(|pair| pair.value.clone()));
    scopes
        .apply_locals(&locals)
        .context("Invalid private scoped variables")?;
    Ok(Overrides { temporary, locals })
}
