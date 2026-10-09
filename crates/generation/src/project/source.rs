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
    let mut pending = vec![(value, 0usize, false, false)];
    let mut count = 0usize;
    while let Some((node, depth, dictionary, literal)) = pending.pop() {
        count += 1;
        ensure!(
            count <= 50000 && depth <= 64,
            "Generation specification complexity limit exceeded"
        );
        match node {
            Value::Object(items) => {
                if !dictionary
                    && !literal
                    && let Some(mapping) = items
                        .get("discriminator")
                        .and_then(|value| value.get("mapping"))
                        .and_then(Value::as_object)
                {
                    for target in mapping.values() {
                        let reference = target.as_str().context("Invalid discriminator mapping")?;
                        let named = value
                            .pointer("/components/schemas")
                            .and_then(Value::as_object)
                            .is_some_and(|schemas| schemas.contains_key(reference));
                        ensure!(
                            named
                                || reference.starts_with("#/")
                                    && value.pointer(&reference[1..]).is_some(),
                            "External or unresolved discriminator mappings require an explicit source bundle"
                        );
                    }
                }
                for (key, child) in items {
                    if !dictionary
                        && !literal
                        && matches!(
                            key.as_str(),
                            "$ref" | "$dynamicRef" | "$recursiveRef" | "operationRef"
                        )
                    {
                        ensure!(
                            child.as_str().is_some_and(|v| v.starts_with("#/")),
                            "External or unresolved generation references require an explicit source bundle"
                        );
                        if key == "operationRef" {
                            let reference = child.as_str().unwrap();
                            ensure!(
                                value
                                    .pointer(&reference[1..])
                                    .and_then(|target| target.get("responses"))
                                    .is_some_and(Value::is_object),
                                "Operation reference target is missing or is not an operation"
                            );
                        }
                    }
                    let data =
                        literal || super::reference_policy::literal_member(key, child, dictionary);
                    pending.push((
                        child,
                        depth + 1,
                        !dictionary && super::reference_policy::dictionary_member(key),
                        data,
                    ));
                }
            }
            Value::Array(items) => {
                pending.extend(items.iter().map(|v| (v, depth + 1, false, literal)))
            }
            _ => {}
        }
    }
    Ok(())
}
