use super::*;
use anyhow::{Context, ensure};
pub fn parse_project_specification(source: &str) -> Result<Value> {
    ensure!(
        source.len() <= SPEC_LIMIT,
        "Generation specification exceeds 1 MiB"
    );
    let value: Value = serde_yaml_ng::from_str(source)
        .map_err(|_| anyhow::anyhow!("Invalid generation specification JSON/YAML"))?;
    validate_project_specification(&value)?;
    Ok(value)
}
pub fn validate_project_specification(value: &Value) -> Result<()> {
    ensure!(
        serde_json::to_vec(value)?.len() <= SPEC_LIMIT,
        "Generation specification exceeds 1 MiB"
    );
    let version = value["openapi"]
        .as_str()
        .context("OpenAPI version missing")?;
    if version.starts_with("3.0.") {
        let _: openapiv3::OpenAPI = serde_json::from_value(value.clone())
            .map_err(|_| anyhow::anyhow!("Invalid OpenAPI3.0 structure"))?;
    } else {
        ensure!(
            version.starts_with("3.1."),
            "Project generation requires OpenAPI3.0/3.1"
        );
        let spec: oas3::Spec = serde_json::from_value(value.clone())
            .map_err(|_| anyhow::anyhow!("Invalid OpenAPI3.1 structure"))?;
        spec.validate_version()
            .map_err(|_| anyhow::anyhow!("Invalid OpenAPI3.1 version"))?;
    }
    let mut pending = vec![(value, 0usize)];
    let mut count = 0usize;
    while let Some((node, depth)) = pending.pop() {
        count += 1;
        ensure!(
            count <= 50000 && depth <= 64,
            "Generation specification complexity limit exceeded"
        );
        match node {
            Value::Object(items) => {
                for (key, value) in items {
                    if matches!(
                        key.as_str(),
                        "$ref" | "$dynamicRef" | "$recursiveRef" | "operationRef"
                    ) {
                        ensure!(
                            value.as_str().is_some_and(|v| v.starts_with("#/")),
                            "External or unresolved generation references require an explicit source bundle"
                        );
                    }
                    pending.push((value, depth + 1));
                }
            }
            Value::Array(items) => pending.extend(items.iter().map(|v| (v, depth + 1))),
            _ => {}
        }
    }
    Ok(())
}
