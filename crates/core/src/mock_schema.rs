//! Deterministic, bounded generation for the explicit saved Mock schema profile.
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Map, Number, Value};
use std::collections::{HashMap, hash_map::Entry};

const MAX_SCHEMA: usize = 256 * 1024;
const MAX_OUTPUT: usize = 1024 * 1024;
const MAX_DEPTH: usize = 64;
const MAX_NODES: usize = 10_000;
const MAX_CANDIDATES: usize = 256;
const MAX_BRANCHES: usize = 16;
const MAX_PREFIX_ITEMS: usize = 32;

#[derive(Default)]
struct GenerateContext {
    nodes: usize,
    candidates: usize,
    validators: HashMap<usize, jsonschema::Validator>,
    values: HashMap<(usize, Option<String>), Value>,
    infer_names: bool,
}

fn name_suggestion(name: &str, schema: &Map<String, Value>) -> Option<Value> {
    let normalized: String = name
        .bytes()
        .filter(|byte| *byte != b'_' && *byte != b'-')
        .map(|byte| byte.to_ascii_lowercase() as char)
        .collect();
    let kind = schema.get("type").and_then(|value| {
        value.as_str().or_else(|| {
            value.as_array().and_then(|types| {
                types
                    .iter()
                    .filter_map(Value::as_str)
                    .find(|kind| *kind != "null")
            })
        })
    })?;
    match (kind, normalized.as_str()) {
        ("integer", "id") => Some(Value::Number(Number::from(1))),
        ("boolean", "active" | "enabled" | "isactive") => Some(Value::Bool(true)),
        ("string", "name" | "fullname") => Some(Value::String("Ada Lovelace".into())),
        ("string", "firstname") => Some(Value::String("Ada".into())),
        ("string", "lastname") => Some(Value::String("Lovelace".into())),
        ("string", "email" | "emailaddress") => Some(Value::String("ada@example.test".into())),
        ("string", "phone" | "phonenumber") => Some(Value::String("+1-202-555-0147".into())),
        ("string", "url" | "website" | "avatarurl") => {
            Some(Value::String("https://example.test/mock".into()))
        }
        _ => None,
    }
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
        .should_validate_formats(true)
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
    let compositions = ["oneOf", "anyOf", "allOf"]
        .iter()
        .filter(|key| object.contains_key(**key))
        .count();
    ensure!(
        compositions <= 1,
        "Mock schema has multiple composition operators at {path}"
    );
    if compositions == 1 {
        ensure!(
            object.keys().all(|key| {
                matches!(
                    key.as_str(),
                    "oneOf" | "anyOf" | "allOf" | "title" | "description"
                ) || (key == "$defs" && path == "#")
            }),
            "Mock schema composition siblings are unsupported at {path}"
        );
    }
    if object.contains_key("prefixItems") {
        let array_type = match object.get("type") {
            Some(Value::String(kind)) => kind == "array",
            Some(Value::Array(types)) => {
                types.len() == 2
                    && types.iter().any(|value| value.as_str() == Some("null"))
                    && types.iter().any(|value| value.as_str() == Some("array"))
            }
            _ => false,
        };
        ensure!(
            array_type
                && object.keys().all(|key| {
                    matches!(
                        key.as_str(),
                        "type"
                            | "prefixItems"
                            | "items"
                            | "minItems"
                            | "maxItems"
                            | "title"
                            | "description"
                    ) || (key == "$defs" && path == "#")
                }),
            "Mock schema tuple siblings are unsupported at {path}"
        );
    }
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
            "items" => {
                if value != &Value::Bool(true) && value != &Value::Bool(false) {
                    scan(value, &field, depth + 1, nodes)?;
                }
            }
            "prefixItems" => {
                let branches = value
                    .as_array()
                    .context("Mock schema prefixItems must be an array")?;
                ensure!(
                    !branches.is_empty() && branches.len() <= MAX_PREFIX_ITEMS,
                    "Mock schema tuple item limit exceeded"
                );
                for (index, child) in branches.iter().enumerate() {
                    scan(child, &format!("{field}/{index}"), depth + 1, nodes)?;
                }
            }
            "oneOf" | "anyOf" | "allOf" => {
                let branches = value
                    .as_array()
                    .with_context(|| format!("Mock schema {field} must be an array"))?;
                ensure!(
                    !branches.is_empty() && branches.len() <= MAX_BRANCHES,
                    "Mock schema composition branch limit exceeded at {field}"
                );
                for (index, child) in branches.iter().enumerate() {
                    scan(child, &format!("{field}/{index}"), depth + 1, nodes)?;
                }
            }
            "$ref" => ensure!(
                value
                    .as_str()
                    .is_some_and(|s| s.starts_with("#/$defs/") && !s.contains(' ')),
                "Mock schema {field} must reference #/$defs"
            ),
            "type" => {
                let supported = |s: &str| {
                    matches!(
                        s,
                        "object" | "array" | "string" | "integer" | "number" | "boolean" | "null"
                    )
                };
                ensure!(
                    value.as_str().is_some_and(supported)
                        || value.as_array().is_some_and(|types| types.len() == 2
                            && types.iter().any(|v| v.as_str() == Some("null"))
                            && types
                                .iter()
                                .any(|v| v.as_str().is_some_and(|s| s != "null" && supported(s)))),
                    "Unsupported Mock schema type at {field}"
                );
            }
            "format" => ensure!(
                matches!(
                    value.as_str(),
                    Some("uuid" | "email" | "date" | "date-time" | "uri")
                ),
                "Unsupported Mock schema format at {field}"
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

fn has_name_suggestion(
    root: &Value,
    schema: &Value,
    name: &str,
    depth: usize,
    context: &mut GenerateContext,
) -> Result<bool> {
    context.nodes += 1;
    ensure!(
        depth <= MAX_DEPTH && context.nodes <= MAX_NODES,
        "Mock schema name lookup budget exceeded"
    );
    let object = schema
        .as_object()
        .context("Mock schema node is not an object")?;
    if let Some(pointer) = object.get("$ref").and_then(Value::as_str) {
        let target = root
            .pointer(&pointer[1..])
            .context("Unresolved Mock schema reference")?;
        return has_name_suggestion(root, target, name, depth + 1, context);
    }
    if name_suggestion(name, object).is_some() {
        return Ok(true);
    }
    for key in ["oneOf", "anyOf", "allOf"] {
        if let Some(branches) = object.get(key).and_then(Value::as_array) {
            for branch in branches {
                if has_name_suggestion(root, branch, name, depth + 1, context)? {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

fn union_authored_candidate(
    root: &Value,
    origin: &Value,
    schema: &Value,
    composed: &Value,
    depth: usize,
    context: &mut GenerateContext,
) -> Result<Option<Value>> {
    context.nodes += 1;
    ensure!(
        depth <= MAX_DEPTH && context.nodes <= MAX_NODES,
        "Mock schema union search budget exceeded"
    );
    let object = schema
        .as_object()
        .context("Mock schema node is not an object")?;
    if let Some(pointer) = object.get("$ref").and_then(Value::as_str) {
        let target = root
            .pointer(&pointer[1..])
            .context("Unresolved Mock schema reference")?;
        return union_authored_candidate(root, origin, target, composed, depth + 1, context);
    }
    for key in ["const", "examples", "default", "enum"] {
        let candidates: Vec<&Value> = match (key, object.get(key)) {
            ("examples" | "enum", Some(Value::Array(values))) => values.iter().collect(),
            (_, Some(value)) => vec![value],
            _ => Vec::new(),
        };
        for candidate in candidates {
            if valid_candidate(root, origin, candidate, context)?
                && valid_candidate(root, composed, candidate, context)?
            {
                return Ok(Some(candidate.clone()));
            }
        }
    }
    for key in ["oneOf", "anyOf"] {
        if let Some(branches) = object.get(key).and_then(Value::as_array) {
            for branch in branches {
                if let Some(value) =
                    union_authored_candidate(root, origin, branch, composed, depth + 1, context)?
                {
                    return Ok(Some(value));
                }
            }
        }
    }
    Ok(None)
}

fn union_name_candidate(
    root: &Value,
    origin: &Value,
    schema: &Value,
    composed: &Value,
    name: &str,
    depth: usize,
    context: &mut GenerateContext,
) -> Result<Option<Value>> {
    context.nodes += 1;
    ensure!(
        depth <= MAX_DEPTH && context.nodes <= MAX_NODES,
        "Mock schema union search budget exceeded"
    );
    let object = schema
        .as_object()
        .context("Mock schema node is not an object")?;
    if let Some(pointer) = object.get("$ref").and_then(Value::as_str) {
        let target = root
            .pointer(&pointer[1..])
            .context("Unresolved Mock schema reference")?;
        return union_name_candidate(root, origin, target, composed, name, depth + 1, context);
    }
    if let Some(value) = name_suggestion(name, object)
        && valid_candidate(root, origin, &value, context)?
        && valid_candidate(root, composed, &value, context)?
    {
        return Ok(Some(value));
    }
    for key in ["oneOf", "anyOf"] {
        if let Some(branches) = object.get(key).and_then(Value::as_array) {
            for branch in branches {
                if let Some(value) =
                    union_name_candidate(root, origin, branch, composed, name, depth + 1, context)?
                {
                    return Ok(Some(value));
                }
            }
        }
    }
    Ok(None)
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
    name: Option<&str>,
    depth: usize,
    refs: &mut Vec<String>,
    context: &mut GenerateContext,
) -> Result<Value> {
    context.nodes += 1;
    ensure!(
        depth <= MAX_DEPTH && context.nodes <= MAX_NODES,
        "Mock schema generation budget exceeded"
    );
    let schema_key = (
        schema as *const Value as usize,
        if context.infer_names {
            name.map(str::to_owned)
        } else {
            None
        },
    );
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
        let value = synthesize(root, target, name, depth + 1, refs, context);
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
    if let Some(branches) = object
        .get("oneOf")
        .or_else(|| object.get("anyOf"))
        .and_then(Value::as_array)
    {
        ensure!(depth < MAX_DEPTH, "Mock schema generation depth exceeded");
        if context.infer_names {
            for branch in branches {
                if let Some(value) =
                    union_authored_candidate(root, branch, branch, schema, depth + 1, context)?
                {
                    context.values.insert(schema_key, value.clone());
                    return Ok(value);
                }
            }
            if let Some(name) = name {
                for branch in branches {
                    if let Some(value) = union_name_candidate(
                        root,
                        branch,
                        branch,
                        schema,
                        name,
                        depth + 1,
                        context,
                    )? {
                        context.values.insert(schema_key, value.clone());
                        return Ok(value);
                    }
                }
            }
        }
        for branch in branches {
            let candidate = synthesize(root, branch, name, depth + 1, refs, context);
            ensure!(
                context.nodes <= MAX_NODES && context.candidates <= MAX_CANDIDATES,
                "Mock schema generation budget exceeded"
            );
            if let Ok(value) = candidate
                && valid_candidate(root, schema, &value, context)?
            {
                context.values.insert(schema_key, value.clone());
                return Ok(value);
            }
        }
        bail!("Unsatisfiable Mock schema union");
    }
    if let Some(branches) = object.get("allOf").and_then(Value::as_array) {
        let mut merged = Map::new();
        for branch in branches {
            let value = synthesize(root, branch, name, depth + 1, refs, context)?;
            let fields = value
                .as_object()
                .context("Mock schema allOf supports object branches only")?;
            for (name, value) in fields {
                if let Some(previous) = merged.insert(name.clone(), value.clone()) {
                    ensure!(
                        previous == *value,
                        "Conflicting Mock schema allOf property {name}"
                    );
                }
            }
            ensure!(
                serde_json::to_vec(&merged)?.len() <= MAX_OUTPUT,
                "Generated Mock body exceeds 1 MiB"
            );
        }
        let value = Value::Object(merged);
        ensure!(
            valid_candidate(root, schema, &value, context)?,
            "Unsatisfiable Mock schema allOf"
        );
        context.values.insert(schema_key, value.clone());
        return Ok(value);
    }
    let kind = object
        .get("type")
        .and_then(|value| {
            value.as_str().or_else(|| {
                value.as_array().and_then(|types| {
                    types
                        .iter()
                        .filter_map(Value::as_str)
                        .find(|kind| *kind != "null")
                })
            })
        })
        .context("Mock schema node needs a supported type or authored value")?;
    if context.infer_names
        && let Some(value) = name.and_then(|name| name_suggestion(name, object))
        && valid_candidate(root, schema, &value, context)?
    {
        context.values.insert(schema_key, value.clone());
        return Ok(value);
    }
    let result = match kind {
        "null" => Value::Null,
        "boolean" => Value::Bool(false),
        "string" => {
            if let Some(format) = object.get("format").and_then(Value::as_str) {
                let fixed = match format {
                    "uuid" => "00000000-0000-4000-8000-000000000000",
                    "email" => "mock@example.test",
                    "date" => "2026-01-01",
                    "date-time" => "2026-01-01T00:00:00Z",
                    "uri" => "https://example.test/mock",
                    _ => bail!("Unsupported Mock schema format"),
                };
                return Ok(Value::String(fixed.into()));
            }
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
                        synthesize(root, child, Some(name), depth + 1, refs, context)?,
                        &mut size,
                    )?;
                }
            }
            if let Some(properties) = properties {
                for (name, child) in properties {
                    if !values.contains_key(name)
                        && (has_authored_value(root, child, depth + 1)?
                            || (context.infer_names
                                && has_name_suggestion(root, child, name, depth + 1, context)?))
                    {
                        insert_bounded(
                            &mut values,
                            name,
                            synthesize(root, child, Some(name), depth + 1, refs, context)?,
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
            let prefix = object.get("prefixItems").and_then(Value::as_array);
            ensure!(
                min == 0 || item.is_some() || prefix.is_some(),
                "Mock schema array needs items"
            );
            let prefix_len = prefix.map_or(0, Vec::len);
            let target_len = if item == Some(&Value::Bool(false)) {
                min.max((prefix_len as u64).min(max))
            } else {
                min.max(prefix_len as u64).min(max)
            };
            ensure!(target_len >= min, "Unsatisfiable Mock schema array length");
            ensure!(
                item != Some(&Value::Bool(false)) || target_len <= prefix_len as u64,
                "Unsatisfiable Mock schema tuple length"
            );
            let mut values = Vec::new();
            let mut size = 2usize;
            for index in 0..target_len as usize {
                let child = prefix.and_then(|values| values.get(index)).or(item);
                let value = match child {
                    Some(Value::Bool(true)) => Value::Null,
                    Some(Value::Bool(false)) => bail!("Unsatisfiable Mock schema tuple length"),
                    Some(child) => synthesize(root, child, None, depth + 1, refs, context)?,
                    None if prefix.is_some() => Value::Null,
                    None => bail!("Missing Mock schema items"),
                };
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
    generate(source, false)
}

pub fn generate_mock_schema_with_names(source: &str) -> Result<String> {
    generate(source, true)
}

fn generate(source: &str, infer_names: bool) -> Result<String> {
    let root = parse(source)?;
    let mut context = GenerateContext {
        infer_names,
        ..GenerateContext::default()
    };
    let value = synthesize(&root, &root, None, 0, &mut Vec::new(), &mut context)?;
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
        assert!(!example.mock_name_inference);
        let encoded = serde_json::to_value(example).unwrap();
        assert!(encoded.get("mock_schema").is_none());
        assert!(encoded.get("mock_name_inference").is_none());
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

    #[test]
    fn known_formats_generate_valid_values_and_reject_invalid_const() {
        assert_eq!(
            generate_mock_schema(
                r#"{"type":"string","format":"email","examples":["not-an-email"]}"#
            )
            .unwrap(),
            r#""mock@example.test""#
        );
        assert!(
            validate_mock_schema(r#"{"type":"string","format":"email","const":"not-an-email"}"#)
                .is_err()
        );
        assert!(validate_mock_schema(r#"{"type":"string","format":"phone"}"#).is_err());
    }

    #[test]
    fn unions_choose_a_valid_branch_and_reject_ambiguous_oneof() {
        assert_eq!(
            generate_mock_schema(
                r#"{"oneOf":[{"type":"string","const":"cat"},{"type":"integer","minimum":5}]}"#
            )
            .unwrap(),
            r#""cat""#
        );
        assert_eq!(generate_mock_schema(r#"{"anyOf":[{"type":"string","minLength":3,"maxLength":2},{"type":"integer","minimum":4}]}"#).unwrap(),"4");
        assert_eq!(generate_mock_schema(r##"{"$defs":{"bad":{"type":"string","minLength":3,"maxLength":2}},"anyOf":[{"$ref":"#/$defs/bad"},{"type":"integer","minimum":4}]}"##).unwrap(),"4");
        assert!(
            validate_mock_schema(
                r#"{"oneOf":[{"type":"integer","minimum":0},{"type":"number","minimum":0}]}"#
            )
            .is_err()
        );
    }

    #[test]
    fn allof_merges_distinct_required_objects_and_rejects_conflicting_values() {
        let inherited = r#"{"allOf":[{"type":"object","properties":{"id":{"type":"integer","minimum":3}},"required":["id"]},{"type":"object","properties":{"name":{"type":"string","const":"Ada"}},"required":["name"]}]}"#;
        assert_eq!(
            generate_mock_schema(inherited).unwrap(),
            r#"{"id":3,"name":"Ada"}"#
        );
        assert!(validate_mock_schema(r#"{"allOf":[{"type":"object","properties":{"id":{"const":3}},"required":["id"]},{"type":"object","properties":{"id":{"const":5}},"required":["id"]}]}"#).is_err());
    }

    #[test]
    fn nullable_type_and_prefix_items_respect_tuple_limits() {
        assert_eq!(
            generate_mock_schema(r#"{"type":["null","string"],"minLength":2}"#).unwrap(),
            r#""xx""#
        );
        assert_eq!(
            generate_mock_schema(r#"{"type":["null","array"],"prefixItems":[{"type":"integer","const":3}],"minItems":1}"#).unwrap(),
            "[3]"
        );
        assert_eq!(generate_mock_schema(r#"{"type":"array","prefixItems":[{"type":"string","const":"x"},{"type":"integer","minimum":2}],"items":false,"minItems":2,"maxItems":2}"#).unwrap(),r#"["x",2]"#);
        assert_eq!(generate_mock_schema(r#"{"type":"array","prefixItems":[{"type":"integer","const":1}],"items":{"type":"string","const":"a"},"minItems":3}"#).unwrap(),r#"[1,"a","a"]"#);
        assert_eq!(generate_mock_schema(r#"{"type":"array","prefixItems":[{"type":"integer","const":1}],"items":true,"minItems":2}"#).unwrap(),"[1,null]");
        assert_eq!(
            generate_mock_schema(
                r#"{"type":"array","prefixItems":[{"type":"integer","const":1}],"minItems":2}"#
            )
            .unwrap(),
            "[1,null]"
        );
        assert_eq!(generate_mock_schema(r#"{"type":"array","prefixItems":[{"type":"integer","const":1},{"type":"integer","const":2}],"items":false,"maxItems":1}"#).unwrap(),"[1]");
        assert!(validate_mock_schema(r#"{"type":"array","prefixItems":[{"type":"integer","const":1}],"items":false,"minItems":2}"#).is_err());
    }

    #[test]
    fn composition_and_tuple_branch_limits_reject() {
        let branches = vec![serde_json::json!({"type":"string"}); 17];
        assert!(validate_mock_schema(&serde_json::json!({"oneOf":branches}).to_string()).is_err());
        let tuple = vec![serde_json::json!({"type":"string"}); 33];
        assert!(
            validate_mock_schema(
                &serde_json::json!({"type":"array","prefixItems":tuple}).to_string()
            )
            .is_err()
        );
        for source in [
            r#"{"oneOf":[]}"#,
            r#"{"anyOf":[{"type":"string"}],"minimum":2}"#,
            r#"{"allOf":[{"type":"object"}],"oneOf":[{"type":"object"}]}"#,
            r#"{"type":"array","prefixItems":[]}"#,
            r#"{"type":"array","prefixItems":[{"type":"string"}],"enum":[["x"]]}"#,
        ] {
            assert!(validate_mock_schema(source).is_err(), "{source}");
        }
    }

    #[test]
    fn union_branch_failures_cannot_bypass_global_work_budgets() {
        let mut properties = Map::new();
        for index in 0..300 {
            properties.insert(
                index.to_string(),
                serde_json::json!({"type":"integer","examples":["bad"]}),
            );
        }
        let required: Vec<String> = properties.keys().cloned().collect();
        let candidate_heavy = serde_json::json!({"anyOf":[{"type":"object","properties":properties,"required":required},{"type":"string","const":"ok"}]});
        assert!(validate_mock_schema(&candidate_heavy.to_string()).is_err());

        let node_heavy = serde_json::json!({"anyOf":[{"type":"array","minItems":10001,"items":{"type":"null"}},{"type":"string","const":"ok"}]});
        assert!(validate_mock_schema(&node_heavy.to_string()).is_err());

        let mut nested = serde_json::json!({"type":"string"});
        for _ in 0..65 {
            nested = serde_json::json!({"anyOf":[nested]});
        }
        assert!(validate_mock_schema(&nested.to_string()).is_err());
    }

    #[test]
    fn opted_in_names_suggest_schema_valid_values_without_changing_legacy_output() {
        let source = r#"{"type":"object","properties":{"id":{"type":"integer"},"fullName":{"type":"string"},"email_address":{"type":"string","format":"email"},"phone-number":{"type":"string"},"other":{"type":"string"}},"required":["id","fullName","email_address"]}"#;
        assert_eq!(
            serde_json::from_str::<Value>(&generate_mock_schema(source).unwrap()).unwrap(),
            serde_json::json!({"email_address":"mock@example.test","fullName":"","id":0})
        );
        assert_eq!(
            serde_json::from_str::<Value>(&generate_mock_schema_with_names(source).unwrap())
                .unwrap(),
            serde_json::json!({"email_address":"ada@example.test","fullName":"Ada Lovelace","id":1,"phone-number":"+1-202-555-0147"})
        );
    }

    #[test]
    fn inferred_names_keep_reference_context_and_authored_precedence() {
        let source = r##"{"$defs":{"text":{"type":"string"}},"type":"object","properties":{"email":{"$ref":"#/$defs/text"},"name":{"$ref":"#/$defs/text"},"first_name":{"type":"string","default":"Grace"},"phone":{"type":"string","maxLength":2},"id":{"type":"integer","minimum":5},"is-active":{"type":"boolean"},"status":{"type":"string","enum":["pending","ready"]},"unknown":{"type":"string"}},"required":["email","name","first_name","id","status"]}"##;
        assert_eq!(
            serde_json::from_str::<Value>(&generate_mock_schema_with_names(source).unwrap())
                .unwrap(),
            serde_json::json!({"email":"ada@example.test","first_name":"Grace","id":5,"is-active":true,"name":"Ada Lovelace","phone":"","status":"pending"})
        );
        let composed = r#"{"type":"object","properties":{"email":{"oneOf":[{"type":"string","minLength":5},{"type":"integer"}]}},"required":["email"]}"#;
        assert_eq!(
            generate_mock_schema_with_names(composed).unwrap(),
            r#"{"email":"ada@example.test"}"#
        );
    }

    #[test]
    fn optional_name_rules_require_a_compatible_schema_type() {
        let source = r#"{"type":"object","properties":{"email":{"type":"integer"},"name":{"type":"string"},"unknown":{"type":"string"}}}"#;
        assert_eq!(
            serde_json::from_str::<Value>(&generate_mock_schema_with_names(source).unwrap())
                .unwrap(),
            serde_json::json!({"name":"Ada Lovelace"})
        );
    }

    #[test]
    fn optional_name_lookup_shares_the_global_node_budget() {
        let mut deep = serde_json::json!({"type":"string"});
        for _ in 0..50 {
            deep = serde_json::json!({"oneOf":[deep]});
        }
        let mut properties = Map::new();
        for index in 0..500 {
            properties.insert(
                format!("unknown{index}"),
                serde_json::json!({"$ref":"#/$defs/deep"}),
            );
        }
        let source =
            serde_json::json!({"$defs":{"deep":deep},"type":"object","properties":properties})
                .to_string();
        assert!(generate_mock_schema_with_names(&source).is_err());
    }

    #[test]
    fn union_names_beat_earlier_generic_branches_but_not_authored_values() {
        for keyword in ["oneOf", "anyOf"] {
            for branches in [
                serde_json::json!([{"type":"integer"},{"type":"string"}]),
                serde_json::json!([{"type":"string"},{"type":"integer"}]),
            ] {
                let mut union = Map::new();
                union.insert(keyword.into(), branches);
                let schema = serde_json::json!({"type":"object","properties":{"email":Value::Object(union)},"required":["email"]});
                assert_eq!(
                    serde_json::from_str::<Value>(
                        &generate_mock_schema_with_names(&schema.to_string()).unwrap()
                    )
                    .unwrap(),
                    serde_json::json!({"email":"ada@example.test"}),
                    "{keyword}"
                );
            }
        }
        let authored = r#"{"type":"object","properties":{"email":{"oneOf":[{"type":"integer","const":7},{"type":"string"}]}},"required":["email"]}"#;
        assert_eq!(
            serde_json::from_str::<Value>(&generate_mock_schema_with_names(authored).unwrap())
                .unwrap(),
            serde_json::json!({"email":7})
        );
        let authored_later = r#"{"type":"object","properties":{"email":{"oneOf":[{"type":"integer"},{"type":"string","default":"saved@example.test"}]}},"required":["email"]}"#;
        assert_eq!(
            serde_json::from_str::<Value>(
                &generate_mock_schema_with_names(authored_later).unwrap()
            )
            .unwrap(),
            serde_json::json!({"email":"saved@example.test"})
        );
    }
}
