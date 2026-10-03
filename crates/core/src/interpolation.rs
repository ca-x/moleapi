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
    interpolate_budget_mode(text, variables, budget, false)
}
fn interpolate_budget_mode(
    text: &str,
    variables: &HashMap<&str, &str>,
    budget: &mut usize,
    graphql: bool,
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
    ensure!(
        graphql || !rest.contains("}}"),
        "Unsupported or unresolved variable"
    );
    append(&mut result, rest, budget)?;
    ensure!(
        !result.contains("{{") && (graphql || !result.contains("}}")),
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
                .map(|v| (v.key.as_str(), v.local_value.as_deref().unwrap_or(&v.value)))
                .collect()
        })
        .unwrap_or_default();
    let mut value = serde_json::to_value(request)?;
    // JavaScript is source code, not an interpolated request field.
    value["pre_request_script"] = "".into();
    value["post_response_script"] = "".into();
    let mqtt_will = if let Protocol::Mqtt { config } = &request.protocol {
        config.will.as_ref().map(|w| w.message.clone())
    } else {
        None
    };
    if request.protocol.is_mqtt() {
        if mqtt_will.is_some() {
            value["protocol"]["will"]["message"]["payload_source"] = "".into();
        }
        value["protocol"]["message"] = serde_json::json!({});
        value["protocol"]["saved_messages"] = serde_json::json!([]);
        if let Protocol::Mqtt { config } = &request.protocol {
            value["protocol"]["subscriptions"] = serde_json::to_value(
                config
                    .subscriptions
                    .iter()
                    .filter(|s| s.enabled)
                    .collect::<Vec<_>>(),
            )?;
        }
    }
    let grpc = if let Protocol::Grpc { message_source, .. } = &request.protocol {
        let message: serde_json::Value =
            serde_json::from_str(message_source).context("Invalid gRPC JSON draft")?;
        value["protocol"]["message_source"] = "".into();
        value["body"] = "".into();
        Some(message)
    } else {
        None
    };
    let socketio = if let Protocol::Socketio { auth_source, .. } = &request.protocol {
        let auth: serde_json::Value =
            serde_json::from_str(auth_source).context("Invalid Socket.IO auth JSON")?;
        value["protocol"]["auth_source"] = "".into();
        value["protocol"]["event"] = "".into();
        value["protocol"]["arguments_source"] = "".into();
        value["protocol"]["attachments_base64"] = serde_json::json!([]);
        Some(auth)
    } else {
        None
    };
    let graphql = request.protocol.is_graphql();
    let mut graphql_body = if graphql {
        Some(
            serde_json::from_str::<serde_json::Value>(&request.body)
                .context("Invalid GraphQL request envelope")?,
        )
    } else {
        None
    };
    if graphql {
        value["body"] = "".into();
        // The script-edited wire envelope is authoritative; avoid resolving stale document text twice.
        value["protocol"]["document"] = "".into();
        value["protocol"]["variables"] = serde_json::json!({});
        value["protocol"]["variables_source"] = serde_json::Value::Null;
        value["protocol"]["operation_name"] = serde_json::Value::Null;
    }
    if request.protocol.is_soap() {
        value["body"] = "".into();
    }
    let form = interpolate(&request.body_kind, &vars)? == "form";
    if form {
        // Decode form fields before interpolation, then encode the resolved values.
        // This also recognizes templates preserved as percent-encoded Postman fields.
        value["body"] = serde_json::Value::String(String::new());
    }
    let mut budget = 20 * 1024 * 1024;
    replace(&mut value, &vars, &mut budget)?;
    if let Some(mut auth) = socketio {
        replace(&mut auth, &vars, &mut budget)?;
        value["protocol"]["auth_source"] = serde_json::to_string(&auth)?.into();
    }
    if let Some(mut message) = grpc {
        replace(&mut message, &vars, &mut budget)?;
        let message_source = serde_json::to_string(&message)?;
        value["protocol"]["message_source"] = message_source.clone().into();
        value["body"] = message_source.into();
    }
    if let Some(body) = &mut graphql_body {
        let query = body
            .get("query")
            .and_then(serde_json::Value::as_str)
            .context("GraphQL envelope query must be a string")?
            .to_owned();
        body["query"] = "".into();
        replace(body, &vars, &mut budget)?;
        // Adjacent closing braces are ordinary GraphQL syntax, not broken template tokens.
        body["query"] = interpolate_budget_mode(&query, &vars, &mut budget, true)?.into();
        value["body"] = serde_json::to_string(body)?.into();
    }
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
    if request.protocol.is_soap() {
        value["body"] = crate::resolve_soap_xml(&request.body, |text| {
            interpolate_budget_mode(text, &vars, &mut budget, true)
        })?
        .into();
    }
    let mut resolved: RequestSpec = serde_json::from_value(value)?;
    resolved.pre_request_script = request.pre_request_script.clone();
    resolved.post_response_script = request.post_response_script.clone();
    reconcile_graphql(&mut resolved)?;
    if let (
        Protocol::Socketio {
            event,
            arguments_source,
            attachments_base64,
            ..
        },
        Protocol::Socketio {
            event: original_event,
            arguments_source: original_arguments,
            attachments_base64: original_attachments,
            ..
        },
    ) = (&mut resolved.protocol, &request.protocol)
    {
        *event = original_event.clone();
        *arguments_source = original_arguments.clone();
        *attachments_base64 = original_attachments.clone();
    }
    if let (Protocol::Mqtt { config }, Protocol::Mqtt { config: original }) =
        (&mut resolved.protocol, &request.protocol)
    {
        if let (Some(will), Some(original_message)) = (&mut config.will, mqtt_will.as_ref()) {
            will.message.payload_source = environment
                .map(|environment| {
                    resolve_mqtt_source(&original_message.payload_source, environment)
                })
                .transpose()?
                .unwrap_or_else(|| original_message.payload_source.clone());
        }
        config.message = original.message.clone();
        config.saved_messages = original.saved_messages.clone();
    }
    let request = resolved;
    validate_request(&request, false)?;
    Ok(request)
}

/// Resolve one field using the same bounded interpolation as complete requests.
/// Callers can inspect private fields before scripts define unrelated URL/body variables.
pub fn resolve_value(text: &str, environment: &Environment) -> Result<String> {
    let vars = environment
        .variables
        .iter()
        .filter(|v| v.enabled)
        .map(|v| (v.key.as_str(), v.local_value.as_deref().unwrap_or(&v.value)))
        .collect();
    interpolate(text, &vars)
}

/// Resolve JSON string values without treating adjacent JSON braces as template tokens.
pub fn resolve_grpc_source(source: &str, environment: &Environment) -> Result<String> {
    ensure!(source.len() <= 1024 * 1024, "gRPC JSON exceeds 1 MiB");
    let mut value: serde_json::Value =
        serde_json::from_str(source).context("Invalid gRPC JSON draft")?;
    let vars = environment
        .variables
        .iter()
        .filter(|v| v.enabled)
        .map(|v| (v.key.as_str(), v.local_value.as_deref().unwrap_or(&v.value)))
        .collect();
    let mut budget = 1024 * 1024;
    replace(&mut value, &vars, &mut budget)?;
    let source = serde_json::to_string(&value)?;
    ensure!(source.len() <= 1024 * 1024, "gRPC JSON exceeds 1 MiB");
    Ok(source)
}

/// Interpolate payload source verbatim without coercing JSON numbers or treating JSON closing braces as templates.
pub fn resolve_mqtt_source(text: &str, environment: &Environment) -> Result<String> {
    let vars = environment
        .variables
        .iter()
        .filter(|v| v.enabled)
        .map(|v| (v.key.as_str(), v.local_value.as_deref().unwrap_or(&v.value)))
        .collect();
    let mut budget = MAX_BODY;
    interpolate_budget_mode(text, &vars, &mut budget, true)
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
