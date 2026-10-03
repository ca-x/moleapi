use moleapi_core::Workspace;
use serde_json::Value;

pub(super) fn workspace(source: &Workspace) -> Workspace {
    let mut result = source.clone();
    for environment in &mut result.data.environments {
        for variable in &mut environment.variables {
            if variable.secret == Some(true) || sensitive(&variable.key) {
                variable.value.clear();
                variable.local_value = None;
            }
        }
    }
    for variable in &mut result.data.global_variables {
        if variable.secret == Some(true) || sensitive(&variable.key) {
            variable.value.clear();
        }
        variable.local_value = None;
    }
    for collection in &mut result.data.collections {
        for variable in &mut collection.variables {
            if variable.secret == Some(true) || sensitive(&variable.key) {
                variable.value.clear();
            }
            variable.local_value = None;
        }
        for request in &mut collection.requests {
            request.auth.token.clear();
            request.auth.password.clear();
            for row in request.headers.iter_mut().chain(request.query.iter_mut()) {
                if row.secret == Some(true) || sensitive(&row.key) {
                    row.value.clear();
                    row.local_value = None;
                }
            }
            for example in &mut request.examples {
                for header in &mut example.headers {
                    if sensitive(&header.key) {
                        header.value.clear();
                    }
                }
                example.body = redact_embedded_json(&example.body);
            }
            request.url = redact_url(&request.url);
            request.body = redact_embedded_json(&request.body);
            match &mut request.protocol {
                moleapi_core::Protocol::Grpc { message_source, .. } => {
                    *message_source = redact_protocol_json(message_source);
                }
                moleapi_core::Protocol::Graphql {
                    variables,
                    variables_source,
                    connection_params,
                    subscription_url,
                    ..
                } => {
                    redact_value(variables);
                    if let Some(source) = variables_source {
                        *source = redact_protocol_json(source);
                    }
                    redact_value(connection_params);
                    if let Some(url) = subscription_url {
                        *url = redact_url(url);
                    }
                }
                _ => {}
            }
        }
    }
    for environment in &mut result.data.environments {
        for variable in &mut environment.variables {
            variable.local_value = None;
        }
    }
    for specification in &mut result.data.specifications {
        if let Ok(mut value) = serde_yaml_ng::from_str::<Value>(&specification.source) {
            redact_value(&mut value);
            // Redaction changes text; preserve original text only for explicit include_secrets exports.
            if let Ok(text) = serde_json::to_string_pretty(&value) {
                specification.source = text;
            }
        } else {
            // An opaque source cannot be safely screened as structured content.
            specification.source.clear();
        }
    }
    result
}
fn redact_url(raw: &str) -> String {
    let Ok(mut url) = url::Url::parse(raw) else {
        return raw.into();
    };
    if !matches!(url.scheme(), "http" | "https" | "ws" | "wss") {
        return raw.into();
    }
    let _ = url.set_password(None);
    let _ = url.set_username("");
    let pairs = url
        .query_pairs()
        .map(|(key, value)| {
            let hidden = sensitive(&key);
            (
                key.into_owned(),
                if hidden {
                    String::new()
                } else {
                    value.into_owned()
                },
            )
        })
        .collect::<Vec<_>>();
    if !pairs.is_empty() {
        url.query_pairs_mut().clear().extend_pairs(pairs);
    }
    url.to_string()
}
// Protocol JSON drafts may be incomplete; unsafe opaque drafts are withheld in
// a default export rather than treating malformed JSON as screened content.
fn redact_protocol_json(text: &str) -> String {
    match serde_json::from_str::<Value>(text) {
        Ok(mut value) => {
            redact_value(&mut value);
            serde_json::to_string_pretty(&value).unwrap_or_default()
        }
        Err(_) => "{}".into(),
    }
}
fn redact_embedded_json(text: &str) -> String {
    let Ok(mut value) = serde_json::from_str::<Value>(text) else {
        return text.into();
    };
    redact_value(&mut value);
    serde_json::to_string_pretty(&value).unwrap_or_default()
}
fn redact_value(value: &mut Value) {
    match value {
        Value::Object(object) => {
            let secret = object.get("secret") == Some(&Value::Bool(true))
                || object.get("type").and_then(Value::as_str) == Some("secret")
                || object
                    .get("key")
                    .and_then(Value::as_str)
                    .is_some_and(sensitive);
            if secret && let Some(value) = object.get_mut("value") {
                *value = Value::String(String::new());
            }
            for (key, value) in object.iter_mut() {
                if sensitive(key) && value.is_string() {
                    *value = Value::String(String::new());
                } else if matches!(key.as_str(), "url" | "raw") && value.is_string() {
                    let text = value.as_str().unwrap_or_default();
                    let replacement = redact_url(text);
                    let replacement = redact_embedded_json(&replacement);
                    *value = Value::String(replacement);
                } else {
                    redact_value(value);
                }
            }
        }
        Value::Array(array) => {
            for value in array {
                redact_value(value);
            }
        }
        _ => {}
    }
}
fn sensitive(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    matches!(
        key.as_str(),
        "authorization"
            | "cookie"
            | "set-cookie"
            | "api_key"
            | "api_token"
            | "apikey"
            | "api-key"
            | "x-api-key"
            | "token"
            | "access_token"
            | "password"
            | "secret"
            | "client_secret"
            | "clientsecret"
    )
}
