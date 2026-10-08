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
pub(crate) fn interpolate_budget(
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
    if request.protocol.is_tcp() {
        value["protocol"]["message"]["payload_source"] = "".into();
        value["body"] = "".into();
    }
    if request.protocol.is_a2a() {
        value["protocol"]["params_source"] = "{}".into();
        value["protocol"]["card_source"] = serde_json::Value::Null;
    }
    if request.protocol.is_mcp() {
        value["protocol"]["arguments_source"] = "{}".into();
        value["protocol"]["config_source"] = serde_json::Value::Null;
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
    let auth_kind = interpolate(&request.auth.kind, &vars)?;
    let jwt_original = if auth_kind == "jwt" {
        request
            .auth
            .jwt
            .as_ref()
            .map(|jwt| jwt.claims_source.clone())
    } else {
        None
    };
    let dormant_jwt = if auth_kind != "jwt" {
        Some(value["auth"]["jwt"].clone())
    } else {
        None
    };
    let dormant_ntlm = if auth_kind != "ntlm" {
        Some(value["auth"]["ntlm"].clone())
    } else {
        None
    };
    if dormant_ntlm.is_some() {
        value["auth"]["ntlm"] = serde_json::Value::Null;
    }
    let dormant_oauth1 = if auth_kind != "oauth1" {
        Some(value["auth"]["oauth1"].clone())
    } else {
        None
    };
    if dormant_oauth1.is_some() {
        value["auth"]["oauth1"] = serde_json::Value::Null;
    }
    if auth_kind == "oauth1"
        && let Some(c) = &request.auth.oauth1
    {
        value["auth"]["oauth1"]["grant"] = serde_json::Value::Null;
        if c.token_id.is_some() {
            value["auth"]["oauth1"]["token"] = "".into();
            value["auth"]["oauth1"]["token_secret"] = "".into();
        }
        if c.location != crate::OAuth1Location::Header {
            value["auth"]["oauth1"]["realm"] = "".into();
        }
        let token = if c.token_id.is_some() {
            String::new()
        } else {
            interpolate(&c.token, &vars)?
        };
        value["auth"]["oauth1"]["token"] = token.clone().into();
        if token.is_empty() {
            value["auth"]["oauth1"]["token_secret"] = "".into();
        }
        let algorithm = interpolate(&c.algorithm, &vars)?;
        value["auth"]["oauth1"]["algorithm"] = algorithm.clone().into();
        if algorithm.starts_with("RSA-") {
            value["auth"]["oauth1"]["consumer_secret"] = "".into();
            value["auth"]["oauth1"]["token_secret"] = "".into();
        } else {
            value["auth"]["oauth1"]["private_key"] = "".into();
        }
    }
    let dormant_hawk = if auth_kind != "hawk" {
        Some(value["auth"]["hawk"].clone())
    } else {
        None
    };
    if dormant_hawk.is_some() {
        value["auth"]["hawk"] = serde_json::Value::Null;
    }
    let dormant_aws = if auth_kind != "aws" {
        Some(value["auth"]["aws"].clone())
    } else {
        None
    };
    if dormant_aws.is_some() {
        value["auth"]["aws"] = serde_json::Value::Null;
    }
    if auth_kind == "oauth2"
        && let Some(config) = &request.auth.oauth2
    {
        value["auth"]["oauth2"] = serde_json::to_value(config.grant_configuration())?;
    }
    let dormant_oauth2 = if auth_kind != "oauth2" {
        Some(value["auth"]["oauth2"].clone())
    } else {
        None
    };
    if dormant_oauth2.is_some() {
        value["auth"]["oauth2"] = serde_json::Value::Null;
    }
    let dormant_key = if auth_kind != "apikey" {
        Some(value["auth"]["api_key"].clone())
    } else {
        None
    };
    if dormant_jwt.is_some() {
        value["auth"]["jwt"] = serde_json::Value::Null;
    }
    if dormant_key.is_some() {
        value["auth"]["api_key"] = serde_json::Value::Null;
    }
    if jwt_original.is_some() {
        value["auth"]["jwt"]["claims_source"] = "".into();
    }
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
    let body_kind = interpolate(&request.body_kind, &vars)?;
    let structured_body = matches!(body_kind.as_str(), "binary" | "multipart");
    if structured_body {
        value["body"] = "".into();
    }
    let form = body_kind == "form";
    if form {
        // Decode form fields before interpolation, then encode the resolved values.
        // This also recognizes templates preserved as percent-encoded Postman fields.
        value["body"] = serde_json::Value::String(String::new());
    }
    let data_original = if let Protocol::Data { config } = &request.protocol {
        value["protocol"]["sql"] = "".into();
        value["protocol"]["file_base64"] = "".into();
        Some((config.sql.clone(), config.file_base64.clone()))
    } else {
        None
    };
    let mut budget = 20 * 1024 * 1024;
    replace(&mut value, &vars, &mut budget)?;
    if let Some(config) = dormant_ntlm {
        value["auth"]["ntlm"] = config;
    }
    if let Some(config) = dormant_oauth1 {
        value["auth"]["oauth1"] = config;
    }
    if let Some(hawk) = dormant_hawk {
        value["auth"]["hawk"] = hawk;
    }
    if let Some(aws) = dormant_aws {
        value["auth"]["aws"] = aws;
    }
    if let Some(jwt) = dormant_jwt
        && !jwt.is_null()
    {
        value["auth"]["jwt"] = jwt;
    }
    if let Some(key) = dormant_key
        && !key.is_null()
    {
        value["auth"]["api_key"] = key;
    }
    if let Some(oauth) = dormant_oauth2
        && !oauth.is_null()
    {
        value["auth"]["oauth2"] = oauth;
    }
    if let Some(source) = jwt_original {
        value["auth"]["jwt"]["claims_source"] =
            crate::authentication::resolve_jwt_claims(&source, &vars, &mut budget)?.into();
    }
    if structured_body {
        value["body"] = crate::request_body::resolve_structured_body(
            &body_kind,
            &request.body,
            &vars,
            &mut budget,
        )?
        .into();
    }
    if let Some((sql, file)) = data_original {
        value["protocol"]["sql"] = sql.into();
        value["protocol"]["file_base64"] = file.into();
    }

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
    if let (Protocol::Mcp { config }, Protocol::Mcp { config: original }) =
        (&mut resolved.protocol, &request.protocol)
    {
        config.arguments_source = original.arguments_source.clone();
        config.config_source = original.config_source.clone();
    }
    if let (Protocol::A2a { config }, Protocol::A2a { config: original }) =
        (&mut resolved.protocol, &request.protocol)
    {
        config.params_source = original.params_source.clone();
        config.card_source = original.card_source.clone();
    }
    if let (Protocol::Tcp { config }, Protocol::Tcp { config: original }) =
        (&mut resolved.protocol, &request.protocol)
    {
        config.message.payload_source = original.message.payload_source.clone();
        resolved.body = request.body.clone();
    }
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
