//! Deterministic, bounded generation for the explicit saved Mock schema profile.
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Map, Number, Value};
use std::collections::{HashMap, hash_map::Entry};

const MAX_SCHEMA: usize = 256 * 1024;
const MAX_OUTPUT: usize = 1024 * 1024;
const MAX_DEPTH: usize = 64;
const MAX_NODES: usize = 10_000;
const MAX_CANDIDATES: usize = 256;

#[derive(Default)]
struct GenerateContext {
    nodes: usize,
    candidates: usize,
    validators: HashMap<usize, jsonschema::Validator>,
    values: HashMap<usize, Value>,
}

struct NoResources;
impl jsonschema::Retrieve for NoResources {
    fn retrieve(
        &self,
        _: &jsonschema::Uri<String>,
    ) -> std::result::Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Err("External Mock schema resources are disabled".into())
    }
}

fn validator(schema: &Value) -> Result<jsonschema::Validator> {
    jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .with_retriever(NoResources)
        .build(schema)
        .context("Invalid Mock schema")
}

fn scan(schema: &Value, path: &str, depth: usize, nodes: &mut usize) -> Result<()> {
    *nodes += 1;
    ensure!(
        depth <= MAX_DEPTH && *nodes <= MAX_NODES,
        "Mock schema complexity limit exceeded at {path}"
    );
    let object = schema
        .as_object()
        .with_context(|| format!("Mock schema must be an object at {path}"))?;
    for (key, value) in object {
        let field = format!("{path}/{key}");
        match key.as_str() {
            "$defs" | "properties" => {
                ensure!(
                    key != "$defs" || path == "#",
                    "Nested Mock schema $defs are unsupported at {field}"
                );
                let values = value
                    .as_object()
                    .with_context(|| format!("Mock schema {field} must be an object"))?;
                for (name, child) in values {
                    scan(child, &format!("{field}/{name}"), depth + 1, nodes)?;
                }
            }
            "items" => scan(value, &field, depth + 1, nodes)?,
            "$ref" => ensure!(
                value
                    .as_str()
                    .is_some_and(|s| s.starts_with("#/$defs/") && !s.contains(' ')),
                "Mock schema {field} must reference #/$defs"
            ),
            "type" => ensure!(
                matches!(
                    value.as_str(),
                    Some("object" | "array" | "string" | "integer" | "number" | "boolean" | "null")
                ),
                "Unsupported Mock schema type at {field}"
            ),
            "required" => ensure!(
                value
                    .as_array()
                    .is_some_and(|a| a.iter().all(Value::is_string)),
                "Mock schema {field} must be a string array"
            ),
            "enum" | "examples" => ensure!(
                value
                    .as_array()
                    .is_some_and(|a| !a.is_empty() && a.len() <= 128),
                "Mock schema {field} must contain 1–128 values"
            ),
            "minimum" | "maximum" => ensure!(
                value.as_f64().is_some_and(f64::is_finite),
                "Mock schema {field} must be a finite number"
            ),
            "minLength" | "maxLength" | "minItems" | "maxItems" => ensure!(
                value.as_u64().is_some_and(|n| n <= MAX_OUTPUT as u64),
                "Mock schema {field} exceeds the output budget"
            ),
            "const" | "default" | "title" | "description" => {}
            _ => bail!("Unsupported Mock schema keyword at {field}"),
        }
    }
    Ok(())
}

fn parse(source: &str) -> Result<Value> {
    ensure!(source.len() <= MAX_SCHEMA, "Mock schema exceeds 256 KiB");
    let root: Value = serde_json::from_str(source).context("Invalid Mock schema JSON")?;
    let mut nodes = 0;
    scan(&root, "#", 0, &mut nodes)?;
    validator(&root)?;
    Ok(root)
}

fn valid_candidate(
    root: &Value,
    schema: &Value,
    candidate: &Value,
    context: &mut GenerateContext,
) -> Result<bool> {
    context.candidates += 1;
    ensure!(
        context.candidates <= MAX_CANDIDATES,
        "Mock schema candidate budget exceeded"
    );
    let key = schema as *const Value as usize;
    let check = match context.validators.entry(key) {
        Entry::Occupied(entry) => entry.into_mut(),
        Entry::Vacant(entry) => {
            let mut scoped = schema.clone();
            if let Some(defs) = root.get("$defs")
                && let Some(object) = scoped.as_object_mut()
            {
                object.entry("$defs").or_insert_with(|| defs.clone());
            }
            entry.insert(validator(&scoped)?)
        }
    };
    Ok(check.is_valid(candidate))
}

fn has_authored_value(root: &Value, schema: &Value, depth: usize) -> Result<bool> {
    ensure!(depth <= MAX_DEPTH, "Mock schema reference depth exceeded");
    let object = schema
        .as_object()
        .context("Mock schema node is not an object")?;
    if let Some(pointer) = object.get("$ref").and_then(Value::as_str) {
        let target = root
            .pointer(&pointer[1..])
            .context("Unresolved Mock schema reference")?;
        return has_authored_value(root, target, depth + 1);
    }
    Ok(["const", "examples", "default"]
        .iter()
        .any(|key| object.contains_key(*key)))
}

fn insert_bounded(
    values: &mut Map<String, Value>,
    name: &str,
    value: Value,
    size: &mut usize,
) -> Result<()> {
    *size = size
        .saturating_add(serde_json::to_string(name)?.len() + serde_json::to_vec(&value)?.len() + 2);
    ensure!(*size <= MAX_OUTPUT, "Generated Mock body exceeds 1 MiB");
    values.insert(name.to_owned(), value);
    Ok(())
}

fn synthesize(
    root: &Value,
    schema: &Value,
    depth: usize,
    refs: &mut Vec<String>,
    context: &mut GenerateContext,
) -> Result<Value> {
    context.nodes += 1;
    ensure!(
        depth <= MAX_DEPTH && context.nodes <= MAX_NODES,
        "Mock schema generation budget exceeded"
    );
    let schema_key = schema as *const Value as usize;
    if let Some(value) = context.values.get(&schema_key) {
        return Ok(value.clone());
    }
    let object = schema
        .as_object()
        .context("Mock schema node is not an object")?;
    if let Some(pointer) = object.get("$ref").and_then(Value::as_str) {
        ensure!(
            object
                .keys()
                .all(|key| matches!(key.as_str(), "$ref" | "title" | "description")),
            "Mock schema reference siblings are unsupported"
        );
        ensure!(
            !refs.iter().any(|seen| seen == pointer),
            "Cyclic Mock schema reference"
        );
        let target = root
            .pointer(&pointer[1..])
            .context("Unresolved Mock schema reference")?;
        refs.push(pointer.to_owned());
        let value = synthesize(root, target, depth + 1, refs, context);
        refs.pop();
        if let Ok(value) = &value {
            context.values.insert(schema_key, value.clone());
        }
        return value;
    }
    for key in ["const", "examples", "default", "enum"] {
        let candidates: Vec<&Value> = match (key, object.get(key)) {
            ("examples" | "enum", Some(Value::Array(values))) => values.iter().collect(),
            (_, Some(value)) => vec![value],
            _ => Vec::new(),
        };
        for value in candidates {
            if valid_candidate(root, schema, value, context)? {
                context.values.insert(schema_key, value.clone());
                return Ok(value.clone());
            }
        }
        if object.contains_key(key) && matches!(key, "const" | "enum") {
            bail!("Unsatisfiable Mock schema {key}");
        }
    }
    let kind = object
        .get("type")
        .and_then(Value::as_str)
        .context("Mock schema node needs a supported type or authored value")?;
    let result = match kind {
        "null" => Value::Null,
        "boolean" => Value::Bool(false),
        "string" => {
            let min = object.get("minLength").and_then(Value::as_u64).unwrap_or(0);
            let max = object
                .get("maxLength")
                .and_then(Value::as_u64)
                .unwrap_or(MAX_OUTPUT as u64);
            ensure!(
                min <= max && min <= MAX_OUTPUT as u64,
                "Unsatisfiable Mock schema string length"
            );
            Value::String("x".repeat(min as usize))
        }
        "integer" | "number" => {
            let min = object.get("minimum").and_then(Value::as_f64).unwrap_or(0.0);
            let max = object
                .get("maximum")
                .and_then(Value::as_f64)
                .unwrap_or(f64::MAX);
            let mut candidate = if kind == "integer" { min.ceil() } else { min };
            if object.get("minimum").is_none() && candidate > max {
                candidate = if kind == "integer" { max.floor() } else { max };
            }
            ensure!(
                candidate <= max && candidate.is_finite(),
                "Unsatisfiable Mock schema number bounds"
            );
            if kind == "integer" {
                ensure!(
                    candidate.abs() <= 9_007_199_254_740_991.0,
                    "Mock schema integer exceeds exact JSON range"
                );
                Value::Number(Number::from(candidate as i64))
            } else {
                Value::Number(Number::from_f64(candidate).context("Invalid Mock schema number")?)
            }
        }
        "object" => {
            let properties = object.get("properties").and_then(Value::as_object);
            let required = object.get("required").and_then(Value::as_array);
            let mut values = Map::new();
            let mut size = 2usize;
            if let Some(required) = required {
                for name in required.iter().filter_map(Value::as_str) {
                    let child = properties
                        .and_then(|p| p.get(name))
                        .with_context(|| format!("Missing Mock schema property {name}"))?;
                    insert_bounded(
                        &mut values,
                        name,
                        synthesize(root, child, depth + 1, refs, context)?,
                        &mut size,
                    )?;
                }
            }
            if let Some(properties) = properties {
                for (name, child) in properties {
                    if !values.contains_key(name) && has_authored_value(root, child, depth + 1)? {
                        insert_bounded(
                            &mut values,
                            name,
                            synthesize(root, child, depth + 1, refs, context)?,
                            &mut size,
                        )?;
                    }
                }
            }
            Value::Object(values)
        }
        "array" => {
            let min = object.get("minItems").and_then(Value::as_u64).unwrap_or(0);
            let max = object
                .get("maxItems")
                .and_then(Value::as_u64)
                .unwrap_or(MAX_OUTPUT as u64);
            ensure!(
                min <= max && min <= MAX_OUTPUT as u64,
                "Unsatisfiable Mock schema array length"
            );
            let item = object.get("items");
            ensure!(min == 0 || item.is_some(), "Mock schema array needs items");
            let mut values = Vec::new();
            let mut size = 2usize;
            for _ in 0..min {
                let value = synthesize(
                    root,
                    item.context("Missing Mock schema items")?,
                    depth + 1,
                    refs,
                    context,
                )?;
                size = size.saturating_add(serde_json::to_vec(&value)?.len() + 1);
                ensure!(size <= MAX_OUTPUT, "Generated Mock body exceeds 1 MiB");
                values.push(value);
            }
            Value::Array(values)
        }
        _ => bail!("Unsupported Mock schema type"),
    };
    context.values.insert(schema_key, result.clone());
    Ok(result)
}

pub fn generate_mock_schema(source: &str) -> Result<String> {
    let root = parse(source)?;
    let value = synthesize(
        &root,
        &root,
        0,
        &mut Vec::new(),
        &mut GenerateContext::default(),
    )?;
    ensure!(
        validator(&root)?.is_valid(&value),
        "Mock schema generated an invalid value"
    );
    let body = serde_json::to_string(&value)?;
    ensure!(
        body.len() <= MAX_OUTPUT,
        "Generated Mock body exceeds 1 MiB"
    );
    Ok(body)
}

pub fn validate_mock_schema(source: &str) -> Result<()> {
    generate_mock_schema(source).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_required_fields_with_authored_values_and_local_refs() {
        let source = r##"{"type":"object","$defs":{"id":{"type":"integer","minimum":3}},"properties":{"id":{"$ref":"#/$defs/id"},"name":{"type":"string","examples":["Ada"]},"secret":{"type":"string"}},"required":["id","name"]}"##;
        assert_eq!(
            generate_mock_schema(source).unwrap(),
            r#"{"id":3,"name":"Ada"}"#
        );
    }

    #[test]
    fn rejects_unsafe_and_unsatisfiable_schemas() {
        for source in [
            r#"{"type":"string","pattern":".*"}"#,
            r#"{"$ref":"https://example.com/schema"}"#,
            r##"{"$defs":{"loop":{"$ref":"#/$defs/loop"}},"$ref":"#/$defs/loop"}"##,
            r#"{"type":"string","minLength":3,"maxLength":2}"#,
            r#"{"type":"object","required":["missing"]}"#,
        ] {
            assert!(validate_mock_schema(source).is_err(), "{source}");
        }
    }

    #[test]
    fn prefers_valid_authored_values_and_meets_array_bounds() {
        let source = r#"{"type":"array","minItems":2,"maxItems":2,"items":{"type":"string","minLength":2,"enum":["ok","yes"],"examples":["x"],"default":"yes"}}"#;
        assert_eq!(generate_mock_schema(source).unwrap(), r#"["yes","yes"]"#);
    }

    #[test]
    fn old_examples_have_no_schema() {
        let example: crate::Example = serde_json::from_value(serde_json::json!({
            "id":"e", "name":"old", "status":200, "headers":[], "body":"saved"
        }))
        .unwrap();
        assert!(example.mock_schema.is_empty());
        assert!(
            serde_json::to_value(example)
                .unwrap()
                .get("mock_schema")
                .is_none()
        );
    }

    #[test]
    fn referenced_optional_defaults_are_included() {
        let source = r##"{"type":"object","$defs":{"label":{"type":"string","default":"Ada"}},"properties":{"label":{"$ref":"#/$defs/label"}}}"##;
        assert_eq!(generate_mock_schema(source).unwrap(), r#"{"label":"Ada"}"#);
    }

    #[test]
    fn nested_candidates_use_top_level_definitions() {
        let source = r##"{"type":"object","$defs":{"n":{"type":"integer","minimum":5}},"properties":{"nested":{"type":"object","properties":{"n":{"$ref":"#/$defs/n"}},"required":["n"],"examples":[{"n":3},{"n":5}]}},"required":["nested"]}"##;
        assert_eq!(
            generate_mock_schema(source).unwrap(),
            r#"{"nested":{"n":5}}"#
        );
        assert!(validate_mock_schema(r##"{"type":"object","properties":{"x":{"$defs":{"n":{"type":"integer"}},"$ref":"#/$defs/n"}}}"##).is_err());
    }

    #[test]
    fn repeated_array_items_reuse_compiled_candidates_and_candidate_work_is_capped() {
        let source = r#"{"type":"array","minItems":500,"items":{"type":"string","minLength":2,"examples":["x"],"enum":["ok"]}}"#;
        let generated = generate_mock_schema(source).unwrap();
        assert_eq!(
            serde_json::from_str::<Vec<String>>(&generated).unwrap(),
            vec!["ok"; 500]
        );
        let schema = serde_json::json!({"type":"string"});
        let mut context = GenerateContext::default();
        for _ in 0..MAX_CANDIDATES {
            assert!(
                valid_candidate(&schema, &schema, &Value::String("ok".into()), &mut context)
                    .unwrap()
            );
        }
        assert!(
            valid_candidate(&schema, &schema, &Value::String("ok".into()), &mut context).is_err()
        );
    }
}
