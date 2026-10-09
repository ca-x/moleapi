use crate::{Collection, Environment, MAX_BODY, Pair, VariableUpdate, WorkspaceData};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_VARIABLE_BYTES: usize = 1024 * 1024;
pub const MAX_SCRIPT_BYTES: usize = 256 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IterationInfo {
    pub index: usize,
    pub count: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VariableScopes {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iteration: Option<IterationInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iteration_data: Option<BTreeMap<String, serde_json::Value>>,
    #[serde(skip)]
    pub environment_id: Option<String>,
    pub project: BTreeMap<String, String>,
    pub collection: BTreeMap<String, String>,
    pub environment: BTreeMap<String, String>,
    pub data: BTreeMap<String, String>,
    pub temporary: BTreeMap<String, String>,
    #[serde(skip)]
    pub private_values: BTreeSet<String>,
}
impl VariableScopes {
    pub fn new(
        workspace: &WorkspaceData,
        collection: Option<&Collection>,
        environment: Option<&Environment>,
        data: &[Pair],
        temporary: &[Pair],
        native: bool,
    ) -> Result<Self> {
        let mut result = Self {
            environment_id: environment.map(|e| e.id.clone()),
            ..Self::default()
        };
        let mut inputs = vec![("project", workspace.global_variables.as_slice())];
        if let Some(collection) = collection {
            for parent in crate::collection_chain(workspace, collection)?
                .into_iter()
                .filter(|c| c.variables_enabled != Some(false))
            {
                inputs.push(("collection", parent.variables.as_slice()));
            }
        }
        inputs.extend([
            (
                "environment",
                environment
                    .map(|e| e.variables.as_slice())
                    .unwrap_or_default(),
            ),
            ("data", data),
            ("temporary", temporary),
        ]);
        for (name, pairs) in inputs {
            validate_variables(pairs)?;
            for pair in pairs.iter().filter(|p| p.enabled) {
                let value = if native || matches!(name, "data" | "temporary") {
                    pair.local_value.as_ref().unwrap_or(&pair.value)
                } else {
                    &pair.value
                };
                if pair.secret == Some(true)
                    || (native && pair.local_value.is_some())
                    || matches!(name, "data" | "temporary")
                {
                    result.private_values.insert(value.clone());
                }
                result
                    .map_mut(name)?
                    .insert(pair.key.clone(), value.clone());
            }
        }
        result.validate()?;
        Ok(result)
    }
    pub fn map_mut(&mut self, scope: &str) -> Result<&mut BTreeMap<String, String>> {
        Ok(match scope {
            "project" => &mut self.project,
            "collection" => &mut self.collection,
            "environment" => &mut self.environment,
            "data" => &mut self.data,
            "temporary" => &mut self.temporary,
            _ => anyhow::bail!("Unsupported variable scope"),
        })
    }
    pub fn effective(&self) -> Environment {
        let mut values = BTreeMap::new();
        for scope in [
            &self.project,
            &self.collection,
            &self.environment,
            &self.data,
            &self.temporary,
        ] {
            values.extend(scope.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
        Environment {
            id: "execution".into(),
            name: "Execution".into(),
            variables: values
                .into_iter()
                .map(|(key, value)| Pair {
                    id: key.clone(),
                    key,
                    secret: Some(self.private_values.contains(&value)),
                    value,
                    enabled: true,
                    local_value: None,
                })
                .collect(),
        }
    }
    pub fn validate(&self) -> Result<()> {
        if let Some(data) = &self.iteration_data {
            ensure!(
                data.len() <= 1000 && serde_json::to_vec(data)?.len() <= MAX_VARIABLE_BYTES,
                "Typed iteration data exceeds execution limit"
            );
        }
        ensure!(
            self.private_values.len() <= 5000
                && self.private_values.iter().map(String::len).sum::<usize>()
                    <= 4 * MAX_VARIABLE_BYTES,
            "Private variable history exceeds execution limit"
        );
        let mut bytes = 0usize;
        for map in [
            &self.project,
            &self.collection,
            &self.environment,
            &self.data,
            &self.temporary,
        ] {
            ensure!(map.len() <= 1000, "Variable scope exceeds 1000 entries");
            for (key, value) in map {
                ensure!(!key.is_empty() && key.len() <= 1024, "Invalid variable key");
                bytes = bytes.saturating_add(key.len()).saturating_add(value.len());
                ensure!(
                    bytes <= MAX_VARIABLE_BYTES,
                    "Execution variables exceed 1 MiB"
                );
            }
        }
        Ok(())
    }
    pub fn apply_locals(&mut self, updates: &[VariableUpdate]) -> Result<()> {
        ensure!(updates.len() <= 1000, "Local overrides exceed 1000 entries");
        ensure!(
            updates.iter().all(|update| matches!(
                update.scope.as_str(),
                "project" | "collection" | "environment"
            )),
            "Local overrides support only project, collection and environment scopes"
        );
        self.apply(updates)
    }
    pub fn apply(&mut self, updates: &[VariableUpdate]) -> Result<()> {
        ensure!(
            updates.len() <= 1000,
            "Variable updates exceed 1000 entries"
        );
        let mut next = self.clone();
        for update in updates {
            ensure!(
                !update.key.is_empty() && update.key.len() <= 1024,
                "Invalid variable key"
            );
            ensure!(update.scope != "data", "Execution data is read-only");
            let map = next.map_mut(&update.scope)?;
            match &update.value {
                Some(value) => {
                    map.insert(update.key.clone(), value.clone());
                }
                None => {
                    map.remove(&update.key);
                }
            }
            if let Some(value) = &update.value {
                next.private_values.insert(value.clone());
            }
        }
        next.validate()?;
        *self = next;
        Ok(())
    }
}
pub fn validate_variables(pairs: &[Pair]) -> Result<()> {
    ensure!(pairs.len() <= 1000, "Variable scope exceeds 1000 entries");
    let mut ids = BTreeSet::new();
    let mut keys = BTreeSet::new();
    let mut bytes = 0usize;
    for pair in pairs {
        ensure!(
            !pair.id.is_empty() && ids.insert(&pair.id),
            "Variable IDs must be unique and nonempty"
        );
        ensure!(
            !pair.key.is_empty() && pair.key.len() <= 1024 && keys.insert(&pair.key),
            "Variable keys must be unique and 1–1024 bytes"
        );
        bytes = bytes
            .saturating_add(pair.key.len())
            .saturating_add(pair.value.len())
            .saturating_add(pair.local_value.as_ref().map_or(0, String::len));
        ensure!(bytes <= MAX_VARIABLE_BYTES, "Variable scope exceeds 1 MiB");
    }
    Ok(())
}
pub fn scrub_local_values(data: &mut WorkspaceData) {
    fn auth(auth: &mut crate::Auth) {
        if let Some(c) = &mut auth.oauth1
            && let Some(grant) = &mut c.grant
        {
            for row in grant
                .request_params
                .iter_mut()
                .chain(&mut grant.access_params)
            {
                row.local_value = None;
            }
        }
    }
    if let Some(a) = &mut data.auth {
        auth(a);
    }

    for pair in &mut data.global_variables {
        pair.local_value = None;
    }
    for collection in &mut data.collections {
        if let Some(a) = &mut collection.auth {
            auth(a);
        }
        for pair in &mut collection.variables {
            pair.local_value = None;
        }
        for request in &mut collection.requests {
            auth(&mut request.auth);
            for pair in request.query.iter_mut().chain(&mut request.headers) {
                pair.local_value = None;
            }
            for example in &mut request.examples {
                for pair in &mut example.headers {
                    pair.local_value = None;
                }
            }
        }
    }
    for env in &mut data.environments {
        for pair in &mut env.variables {
            pair.local_value = None;
        }
    }
}
pub fn preserve_local_values(current: &WorkspaceData, next: &mut WorkspaceData) {
    fn preserve(previous: &[Pair], next: &mut [Pair]) {
        for pair in next {
            if let Some(old) = previous
                .iter()
                .find(|old| old.id == pair.id && old.key == pair.key)
            {
                pair.local_value = old.local_value.clone();
            }
        }
    }
    preserve(&current.global_variables, &mut next.global_variables);
    for env in &mut next.environments {
        if let Some(old) = current.environments.iter().find(|old| old.id == env.id) {
            preserve(&old.variables, &mut env.variables);
        }
    }
    for collection in &mut next.collections {
        if let Some(old) = current
            .collections
            .iter()
            .find(|old| old.id == collection.id)
        {
            preserve(&old.variables, &mut collection.variables);
        }
    }
}
pub fn validate_script(script: &str) -> Result<()> {
    ensure!(
        script.len() <= MAX_SCRIPT_BYTES && script.len() <= MAX_BODY,
        "Script exceeds 256 KiB"
    );
    Ok(())
}
