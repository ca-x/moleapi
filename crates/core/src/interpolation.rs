use crate::*;
use anyhow::{Context, Result, ensure};
use std::collections::HashMap;
pub(crate) fn interpolate(text: &str, variables: &HashMap<&str, &str>) -> Result<String> {
    let mut budget = MAX_BODY;
    interpolate_budget(text, variables, &mut budget)
}
fn append(result: &mut String, text: &str, budget: &mut usize) -> Result<()> {
    ensure!(
        result
            .len()
            .checked_add(text.len())
            .is_some_and(|n| n <= MAX_BODY)
            && text.len() <= *budget,
        "Resolved request exceeds expansion limit"
    );
    *budget -= text.len();
    result.push_str(text);
    Ok(())
}
fn interpolate_budget(
    text: &str,
    variables: &HashMap<&str, &str>,
    budget: &mut usize,
) -> Result<String> {
    let mut result = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        append(&mut result, &rest[..start], budget)?;
        let tail = &rest[start + 2..];
        let end = tail
            .find("}}")
            .context("Unsupported or unresolved variable")?;
        let key = tail[..end].trim();
        append(
            &mut result,
            variables
                .get(key)
                .context("Unsupported or unresolved variable")?,
            budget,
        )?;
        rest = &tail[end + 2..];
    }
    ensure!(!rest.contains("}}"), "Unsupported or unresolved variable");
    append(&mut result, rest, budget)?;
    ensure!(
        !result.contains("{{") && !result.contains("}}"),
        "Unsupported or unresolved variable"
    );
    Ok(result)
}
fn replace(
    value: &mut serde_json::Value,
    vars: &HashMap<&str, &str>,
    budget: &mut usize,
) -> Result<()> {
    match value {
        serde_json::Value::String(s) => *s = interpolate_budget(s, vars, budget)?,
        serde_json::Value::Array(items) => {
            for item in items {
                replace(item, vars, budget)?;
            }
        }
        serde_json::Value::Object(map) => {
            for item in map.values_mut() {
                replace(item, vars, budget)?;
            }
        }
        _ => {}
    }
    Ok(())
}
pub fn resolve_request(
    request: &RequestSpec,
    environment: Option<&Environment>,
) -> Result<RequestSpec> {
    let vars = environment
        .map(|e| {
            e.variables
                .iter()
                .filter(|v| v.enabled)
                .map(|v| (v.key.as_str(), v.value.as_str()))
                .collect()
        })
        .unwrap_or_default();
    let mut value = serde_json::to_value(request)?;
    let form = interpolate(&request.body_kind, &vars)? == "form";
    if form {
        // Decode form fields before interpolation, then encode the resolved values.
        // This also recognizes templates preserved as percent-encoded Postman fields.
        value["body"] = serde_json::Value::String(String::new());
    }
    let mut budget = 20 * 1024 * 1024;
    replace(&mut value, &vars, &mut budget)?;
    if form {
        let mut body = String::new();
        for (key, field) in url::form_urlencoded::parse(request.body.as_bytes()) {
            let key = interpolate_budget(&key, &vars, &mut budget)?;
            let field = interpolate_budget(&field, &vars, &mut budget)?;
            let encoded = url::form_urlencoded::Serializer::new(String::new())
                .append_pair(&key, &field)
                .finish();
            let growth = (encoded.len() + usize::from(!body.is_empty()))
                .saturating_sub(key.len() + field.len());
            ensure!(growth <= budget, "Resolved request exceeds expansion limit");
            budget -= growth;
            ensure!(
                body.len()
                    .checked_add(encoded.len() + usize::from(!body.is_empty()))
                    .is_some_and(|n| n <= MAX_BODY),
                "Resolved form exceeds body limit"
            );
            if !body.is_empty() {
                body.push('&');
            }
            body.push_str(&encoded);
        }
        value["body"] = serde_json::Value::String(body);
    }
    let request = serde_json::from_value(value)?;
    validate_request(&request, false)?;
    Ok(request)
}

#[cfg(test)]
mod bounds_tests {
    use super::*;
    #[test]
    fn oversized_substitution_fails_before_materializing_the_expansion() {
        let large = "x".repeat(1024 * 1024);
        let vars = HashMap::from([("value", large.as_str())]);
        let template = "{{value}}".repeat(6);
        assert!(
            interpolate(&template, &vars).is_err(),
            "resolved string must be bounded during construction"
        );
    }
}
