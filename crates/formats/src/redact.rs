use moleapi_core::Workspace;
use serde_json::Value;

pub(super) fn workspace(source: &Workspace) -> Workspace {
    let privacy = ExportPrivacy::new(source);
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
                moleapi_core::Protocol::Mcp { config } => {
                    request.url = privacy.screen_text(&request.url);
                    for row in request.headers.iter_mut().chain(request.query.iter_mut()) {
                        row.value = privacy.screen_text(&row.value);
                        row.local_value = None;
                    }
                    mcp_args(&mut config.args);
                    for row in &mut config.env {
                        if row.secret == Some(true) || sensitive(&row.key) {
                            row.value.clear();
                        }
                        row.value = privacy.screen_text(&row.value);
                        row.local_value = None;
                    }
                    config.command = privacy.screen_text(&config.command);
                    config.uri = privacy.screen_text(&redact_url(&config.uri));
                    config.arguments_source = privacy
                        .screen_json(&redact_protocol_json_or(&config.arguments_source, "{}"));
                    for arg in &mut config.args {
                        *arg = privacy.screen_text(arg);
                    }
                    if let Some(source) = &mut config.config_source {
                        *source = privacy.screen_json(&redact_protocol_json_or(source, "{}"));
                        if let Ok(mut value) = serde_json::from_str::<Value>(source) {
                            redact_mcp_args(&mut value);
                            *source = serde_json::to_string_pretty(&value).unwrap_or_default();
                        }
                    }
                }
                moleapi_core::Protocol::Soap { .. } => {
                    request.body = privacy.screen_xml(&request.body, false);
                    for example in &mut request.examples {
                        example.body = privacy.screen_xml(&example.body, false);
                    }
                }
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
            if specification.kind == "wsdl"
                && let Some(files) = value.get_mut("files").and_then(Value::as_array_mut)
            {
                for file in files {
                    if let Some(content) = file.get_mut("content") {
                        *content = Value::String(
                            content
                                .as_str()
                                .map(|source| privacy.screen_xml(source, true))
                                .unwrap_or_default(),
                        );
                    }
                }
            }
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

// XML entity spelling is arbitrary; screen decoded data before default exports.
struct ExportPrivacy {
    matcher: Option<aho_corasick::AhoCorasick>,
    withhold: bool,
}
impl ExportPrivacy {
    fn new(workspace: &Workspace) -> Self {
        let mut secrets = std::collections::BTreeSet::new();
        let mut pairs = |rows: &[moleapi_core::Pair]| {
            for row in rows {
                if row.secret == Some(true) || sensitive(&row.key) {
                    secrets.insert(row.value.clone());
                    if let Some(value) = &row.local_value {
                        secrets.insert(value.clone());
                    }
                }
            }
        };
        pairs(&workspace.data.global_variables);
        for environment in &workspace.data.environments {
            pairs(&environment.variables);
        }
        for collection in &workspace.data.collections {
            pairs(&collection.variables);
            for request in &collection.requests {
                pairs(&request.headers);
                pairs(&request.query);
                if let moleapi_core::Protocol::Mcp { config } = &request.protocol {
                    pairs(&config.env);
                }
            }
        }
        for collection in &workspace.data.collections {
            for request in &collection.requests {
                secrets.insert(request.auth.token.clone());
                secrets.insert(request.auth.password.clone());
                if let moleapi_core::Protocol::Mcp { config } = &request.protocol {
                    if let Some(raw) = &config.config_source
                        && let Ok(source) = serde_json::from_str::<Value>(raw)
                    {
                        // Host sources have no secret flags; imported env/header values
                        // are private by default, including originals changed later.
                        for field in ["env", "headers"] {
                            if let Some(rows) = source.get(field).and_then(Value::as_object) {
                                for value in rows.values().filter_map(Value::as_str) {
                                    secrets.insert(value.into());
                                }
                            }
                        }
                        if let Some(values) = source.get("args")
                            && let Ok(args) = serde_json::from_value::<Vec<String>>(values.clone())
                        {
                            collect_mcp_private_args(&args, &mut secrets);
                        }
                    }
                    collect_mcp_private_args(&config.args, &mut secrets);
                }
            }
        }
        secrets.remove("");
        if secrets.iter().any(|value| value.len() > 4096)
            || secrets.iter().map(String::len).sum::<usize>() > 16384
        {
            return Self {
                matcher: None,
                withhold: true,
            };
        }
        if secrets.is_empty() {
            return Self {
                matcher: None,
                withhold: false,
            };
        }
        match aho_corasick::AhoCorasick::new(secrets) {
            Ok(matcher) => Self {
                matcher: Some(matcher),
                withhold: false,
            },
            Err(_) => Self {
                matcher: None,
                withhold: true,
            },
        }
    }
    fn screen_text(&self, text: &str) -> String {
        if self.withhold {
            return String::new();
        }
        match &self.matcher {
            Some(matcher) => {
                let replacements = vec!["[REDACTED]"; matcher.patterns_len()];
                matcher.replace_all(text, &replacements)
            }
            None => text.into(),
        }
    }
    fn screen_json(&self, text: &str) -> String {
        if self.withhold {
            return "{}".into();
        }
        fn visit(value: &mut Value, privacy: &ExportPrivacy) {
            match value {
                Value::String(text) => *text = privacy.screen_text(text),
                Value::Array(values) => {
                    for value in values {
                        visit(value, privacy);
                    }
                }
                Value::Object(object) => {
                    if let Some(matcher) = &privacy.matcher {
                        object.retain(|key, _| !matcher.is_match(key));
                    }
                    for value in object.values_mut() {
                        visit(value, privacy);
                    }
                }
                Value::Number(_) | Value::Bool(_)
                    if privacy
                        .matcher
                        .as_ref()
                        .is_some_and(|matcher| matcher.is_match(&value.to_string())) =>
                {
                    *value = Value::Null;
                }
                _ => {}
            }
        }
        let Ok(mut value) = serde_json::from_str::<Value>(text) else {
            return "{}".into();
        };
        visit(&mut value, self);
        serde_json::to_string_pretty(&value).unwrap_or_default()
    }
    fn screen_xml(&self, source: &str, definitions: bool) -> String {
        let safe = if definitions {
            moleapi_core::redact_soap_source_xml(source)
        } else {
            moleapi_core::redact_soap_xml(source)
        };
        let Ok(safe) = safe else {
            return String::new();
        };
        if self.withhold {
            return String::new();
        }
        if let Some(matcher) = &self.matcher {
            if matcher.is_match(&safe) {
                return String::new();
            }
            let Ok(values) = moleapi_core::soap_xml_data_values(&safe) else {
                return String::new();
            };
            if values.iter().any(|value| matcher.is_match(value)) {
                return String::new();
            }
        }
        safe
    }
}

fn mcp_args(args: &mut [String]) {
    let mut hidden = false;
    for arg in args {
        if hidden {
            arg.clear();
            hidden = false;
            continue;
        }
        if !arg.starts_with('-') {
            continue;
        }
        if let Some((key, _)) = arg.split_once('=') {
            if mcp_private_flag(key) {
                *arg = format!("{key}=");
            }
        } else if mcp_private_flag(arg) {
            hidden = true;
        }
    }
}
fn redact_mcp_args(value: &mut Value) {
    if let Some(args) = value.get_mut("args") {
        if let Ok(mut values) = serde_json::from_value::<Vec<String>>(args.clone()) {
            mcp_args(&mut values);
            *args = serde_json::to_value(values).unwrap_or(Value::Null);
        } else {
            *args = Value::Array(vec![]);
        }
    }
}

fn mcp_private_flag(flag: &str) -> bool {
    let key = flag.trim_start_matches('-');
    sensitive(key) || sensitive(&key.replace('-', "_"))
}

fn collect_mcp_private_args(args: &[String], secrets: &mut std::collections::BTreeSet<String>) {
    let mut hidden = false;
    for arg in args {
        if hidden {
            secrets.insert(arg.clone());
            hidden = false;
            continue;
        }
        if !arg.starts_with('-') {
            continue;
        }
        if let Some((key, value)) = arg.split_once('=') {
            if mcp_private_flag(key) {
                secrets.insert(value.into());
            }
        } else if mcp_private_flag(arg) {
            hidden = true;
        }
    }
}
