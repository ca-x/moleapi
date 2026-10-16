//! Postman environment/globals files mapped to existing workspace variable scopes.
use crate::{ExportResult, redact, uid};
use anyhow::{Context, Result, ensure};
use moleapi_core::{Pair, Workspace, validate_variables};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;

const MAX_FILE: usize = 1024 * 1024;

#[derive(Deserialize, Serialize)]
pub struct VariableImport {
    pub scope: String,
    pub name: String,
    pub variables: Vec<Pair>,
    pub warnings: Vec<String>,
}

pub fn import_postman_variables(content: &str) -> Result<VariableImport> {
    ensure!(
        content.len() <= MAX_FILE,
        "Postman variable file exceeds 1 MiB"
    );
    let source: Value = serde_json::from_str(content).context("Invalid Postman variable JSON")?;
    let object = source
        .as_object()
        .context("Postman variable file must be an object")?;
    let scope = object
        .get("_postman_variable_scope")
        .and_then(Value::as_str)
        .context("Postman variable scope is missing")?;
    ensure!(
        matches!(scope, "environment" | "globals"),
        "Unsupported Postman variable scope"
    );
    let name = object
        .get("name")
        .and_then(Value::as_str)
        .context("Postman variable file name missing")?;
    ensure!(
        !name.trim().is_empty() && name.len() <= 128 && !name.chars().any(char::is_control),
        "Postman variable file name must be 1–128 bytes without control characters"
    );
    let rows = object
        .get("values")
        .and_then(Value::as_array)
        .context("Postman variable values must be an array")?;
    ensure!(
        rows.len() <= 1000,
        "Postman variable scope exceeds 1000 entries"
    );
    let mut variables = Vec::with_capacity(rows.len());
    let mut warnings = BTreeSet::new();
    for (index, row) in rows.iter().enumerate() {
        let row = row
            .as_object()
            .with_context(|| format!("Postman variable row {} is not an object", index + 1))?;
        let key = row
            .get("key")
            .and_then(Value::as_str)
            .with_context(|| format!("Postman variable row {} has no string key", index + 1))?;
        let value = row
            .get("value")
            .and_then(Value::as_str)
            .with_context(|| format!("Postman variable row {} has no string value", index + 1))?;
        let enabled = match row.get("enabled") {
            Some(Value::Bool(value)) => *value,
            None => true,
            _ => anyhow::bail!(
                "Postman variable row {} has invalid enabled flag",
                index + 1
            ),
        };
        let kind = match row.get("type") {
            Some(Value::String(value)) => value.as_str(),
            None => "default",
            _ => anyhow::bail!("Postman variable row {} has invalid type", index + 1),
        };
        ensure!(
            matches!(kind, "default" | "secret"),
            "Postman variable row {} has unsupported type",
            index + 1
        );
        if row
            .keys()
            .any(|field| !matches!(field.as_str(), "key" | "value" | "enabled" | "type"))
        {
            warnings.insert("postman_variable_row_metadata_omitted".into());
        }
        variables.push(Pair {
            id: uid(),
            key: key.into(),
            value: value.into(),
            enabled,
            secret: Some(kind == "secret"),
            local_value: None,
        });
    }
    validate_variables(&variables)?;
    Ok(VariableImport {
        scope: scope.into(),
        name: name.into(),
        variables,
        warnings: warnings.into_iter().collect(),
    })
}

pub fn export_postman_variables(
    workspace: &Workspace,
    scope: &str,
    environment_id: Option<&str>,
    include_secrets: bool,
) -> Result<ExportResult> {
    let projected = if include_secrets {
        workspace.clone()
    } else {
        redact::workspace(workspace)
    };
    let (id, name, variables, suffix) = match scope {
        "environment" => {
            let target = environment_id.context("Select an environment to export")?;
            let environment = projected
                .data
                .environments
                .iter()
                .find(|item| item.id == target)
                .context("Environment not found")?;
            (
                environment.id.as_str(),
                environment.name.as_str(),
                environment.variables.as_slice(),
                "postman_environment.json",
            )
        }
        "globals" => {
            ensure!(
                environment_id.is_none(),
                "Globals export does not accept an environment ID"
            );
            (
                projected.id.as_str(),
                projected.name.as_str(),
                projected.data.global_variables.as_slice(),
                "postman_globals.json",
            )
        }
        _ => anyhow::bail!("Unsupported Postman variable scope"),
    };
    validate_variables(variables)?;
    let values = variables
        .iter()
        .map(|item| {
            json!({
                "key":item.key,"value":item.value,"enabled":item.enabled,
                "type":if item.secret==Some(true){"secret"}else{"default"}
            })
        })
        .collect::<Vec<_>>();
    let content = serde_json::to_string_pretty(&json!({
        "id":id,"name":name,"values":values,"_postman_variable_scope":scope
    }))?;
    ensure!(
        content.len() <= MAX_FILE,
        "Postman variable export exceeds 1 MiB"
    );
    let stem = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(64)
        .collect::<String>();
    let stem = if stem.is_empty() { "variables" } else { &stem };
    Ok(ExportResult {
        warnings: vec![],
        filename: format!("{stem}.{suffix}"),
        content,
        mime: "application/json".into(),
    })
}
