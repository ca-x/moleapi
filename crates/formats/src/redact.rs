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
                moleapi_core::Protocol::Mqtt { config } => {
                    mqtt_message(&mut config.message);
                    for entry in &mut config.saved_messages {
                        for value in [
                            entry
                                .message
                                .topic_secret
                                .then_some(entry.message.topic.as_str()),
                            entry
                                .message
                                .payload_secret
                                .then_some(entry.message.payload_source.as_str()),
                        ]
                        .into_iter()
                        .flatten()
                        .filter(|value| !value.is_empty())
                        {
                            entry.name = entry.name.replace(value, "[REDACTED]");
                        }
                        mqtt_message(&mut entry.message);
                    }
                    if let Some(will) = &mut config.will {
                        mqtt_message(&mut will.message);
                    }
                    mqtt_properties(&mut config.user_properties);
                    for subscription in &mut config.subscriptions {
                        if subscription.filter_secret {
                            subscription.filter.clear();
                        }
                        mqtt_properties(&mut subscription.user_properties);
                    }
                }
                moleapi_core::Protocol::Socketio {
                    auth_source,
                    arguments_source,
                    ..
                } => {
                    *auth_source = redact_protocol_json(auth_source);
                    *arguments_source = redact_protocol_json_or(arguments_source, "[]");
                }
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
                    redact_payload(variables);
                    if let Some(source) = variables_source {
                        *source = redact_protocol_json(source);
                    }
                    redact_payload(connection_params);
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
fn mqtt_properties(values: &mut [moleapi_core::MqttProperty]) {
    for value in values {
        if value.secret || sensitive(&value.key) {
            value.value.clear();
        }
    }
}
fn mqtt_message(value: &mut moleapi_core::MqttMessage) {
    if value.topic_secret {
        value.topic.clear();
    }
    if value.payload_secret {
        value.payload_source.clear();
    } else {
        value.payload_source = redact_embedded_json(&value.payload_source);
    }
    mqtt_properties(&mut value.properties.user_properties);
}
fn redact_url(raw: &str) -> String {
    let Ok(mut url) = url::Url::parse(raw) else {
        return raw.into();
    };
    if !matches!(
        url.scheme(),
        "http" | "https" | "ws" | "wss" | "mqtt" | "mqtts"
    ) {
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
    redact_protocol_json_or(text, "{}")
}
fn redact_protocol_json_or(text: &str, fallback: &str) -> String {
    match serde_json::from_str::<Value>(text) {
        Ok(mut value) => {
            redact_payload(&mut value);
            serde_json::to_string_pretty(&value).unwrap_or_default()
        }
        Err(_) => fallback.into(),
    }
}
fn redact_embedded_json(text: &str) -> String {
    let Ok(mut value) = serde_json::from_str::<Value>(text) else {
        return text.into();
    };
    redact_payload(&mut value);
    serde_json::to_string_pretty(&value).unwrap_or_default()
}
// Payload fields can carry numeric/boolean/structured credentials. Canonical
// specification definitions use the separate schema-preserving traversal below.
fn redact_payload(value: &mut Value) {
    match value {
        Value::Object(object) => {
            let private_row = object.get("secret") == Some(&Value::Bool(true))
                || object.get("type").and_then(Value::as_str) == Some("secret")
                || object
                    .get("key")
                    .and_then(Value::as_str)
                    .is_some_and(sensitive);
            if private_row && let Some(value) = object.get_mut("value") {
                *value = if value.is_string() {
                    Value::String(String::new())
                } else {
                    Value::Null
                };
            }
            for (key, value) in object.iter_mut() {
                if sensitive(key) && !(private_row && key == "secret" && value.is_boolean()) {
                    *value = if value.is_string() {
                        Value::String(String::new())
                    } else {
                        Value::Null
                    };
                } else if matches!(key.as_str(), "url" | "raw") && value.is_string() {
                    let replacement = redact_url(value.as_str().unwrap_or_default());
                    *value = Value::String(redact_embedded_json(&replacement));
                } else {
                    redact_payload(value);
                }
            }
        }
        Value::Array(array) => {
            for value in array {
                redact_payload(value);
            }
        }
        _ => {}
    }
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
