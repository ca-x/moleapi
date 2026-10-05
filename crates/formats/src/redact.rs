use moleapi_core::Workspace;
use serde_json::Value;

pub(super) fn workspace(source: &Workspace) -> Workspace {
    let privacy = ExportPrivacy::new(source);
    let tcp_privacy = ExportPrivacy::from_workspace(source, true);
    let data_privacy = &tcp_privacy;
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
            let dynamic = request.body_kind.contains("{{");
            let file_source = serde_json::from_str::<Value>(&request.body).ok();
            if request.body_kind == "binary"
                || dynamic
                    && file_source
                        .as_ref()
                        .is_some_and(|v| v.get("base64").is_some())
            {
                if let Ok(mut file) =
                    serde_json::from_str::<moleapi_core::BinaryBody>(&request.body)
                {
                    file.base64 = None;
                    file.file_name = data_privacy.screen_bounded(&file.file_name, 512);
                    if file.file_name.is_empty() {
                        file.file_name = "[REDACTED]".into();
                    }
                    if data_privacy.screen_text(&file.mime) != file.mime {
                        file.mime.clear();
                    }
                    request.body = serde_json::to_string(&file).unwrap();
                }
            } else if (request.body_kind == "multipart"
                || dynamic
                    && file_source
                        .as_ref()
                        .is_some_and(|v| v.get("parts").is_some()))
                && let Ok(mut body) =
                    serde_json::from_str::<moleapi_core::MultipartBody>(&request.body)
            {
                let mut remaining = moleapi_core::MAX_BODY;
                for part in &mut body.parts {
                    match &mut part.value {
                        moleapi_core::MultipartValue::File { file } => {
                            file.base64 = None;
                            file.file_name = data_privacy.screen_bounded(&file.file_name, 512);
                            if file.file_name.is_empty() {
                                file.file_name = "[REDACTED]".into();
                            }
                            if data_privacy.screen_text(&file.mime) != file.mime {
                                file.mime.clear();
                            }
                        }
                        moleapi_core::MultipartValue::Text { text, mime } => {
                            *text = if sensitive(&part.name) {
                                String::new()
                            } else {
                                let literal = data_privacy.screen_bounded(text, remaining);
                                if literal == *text {
                                    let screened = data_privacy.screen_generation_text(text);
                                    if screened.len() <= remaining {
                                        screened
                                    } else {
                                        String::new()
                                    }
                                } else {
                                    literal
                                }
                            };
                            remaining -= text.len();
                            if data_privacy.screen_text(mime) != *mime {
                                mime.clear();
                            }
                        }
                    }
                    part.name = data_privacy.screen_bounded(&part.name, 512);
                    if part.name.is_empty() {
                        part.name = "[REDACTED]".into();
                    }
                }
                request.body = serde_json::to_string(&body).unwrap();
                if request.body.len() > moleapi_core::MAX_BODY_SOURCE {
                    for part in &mut body.parts {
                        if let moleapi_core::MultipartValue::Text { text, .. } = &mut part.value {
                            text.clear();
                        }
                    }
                    request.body = serde_json::to_string(&body).unwrap();
                }
            }
            match &mut request.protocol {
                moleapi_core::Protocol::Data { config } => {
                    request.url = data_privacy.screen_text(&redact_url(&request.url));
                    config.file_base64.clear();
                    config.sql = data_privacy.screen_generation_text(&config.sql);
                    config.file_name = data_privacy.screen_text(&config.file_name);
                    config.table_name = data_privacy.screen_text(&config.table_name);
                }
                moleapi_core::Protocol::Tcp { config } => {
                    request.url = tcp_privacy.screen_text(&request.url);
                    config.message.payload_source = if config.message.secret || tcp_privacy.withhold
                    {
                        String::new()
                    } else if config.message.encoding == "text" {
                        tcp_privacy.screen_generation_json(&redact_embedded_json(
                            &config.message.payload_source,
                        ))
                    } else if template_reference(&config.message.payload_source) {
                        config.message.payload_source.clone()
                    } else if let Ok(bytes) = moleapi_core::tcp_payload(&config.message) {
                        if let Ok(original) = serde_json::from_slice::<Value>(&bytes) {
                            let text = std::str::from_utf8(&bytes).unwrap_or_default();
                            let screened =
                                tcp_privacy.screen_generation_json(&redact_embedded_json(text));
                            if serde_json::from_str::<Value>(&screened)
                                .is_ok_and(|value| value == original)
                            {
                                config.message.payload_source.clone()
                            } else {
                                String::new()
                            }
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    };
                }
                moleapi_core::Protocol::A2a { config } => {
                    request.url = privacy.screen_text(&request.url);
                    for row in request.headers.iter_mut().chain(request.query.iter_mut()) {
                        row.value = privacy.screen_text(&row.value);
                        row.local_value = None;
                    }
                    config.params_source =
                        privacy.screen_json(&redact_protocol_json_or(&config.params_source, "{}"));
                    if let Some(url) = &mut config.interface_url {
                        *url = privacy.screen_text(&redact_url(url));
                    }
                    if let Some(source) = &mut config.card_source {
                        if let Ok(mut value) = serde_json::from_str::<Value>(source) {
                            redact_value(&mut value);
                            *source = privacy
                                .screen_json(&serde_json::to_string(&value).unwrap_or_default());
                        } else {
                            source.clear();
                        }
                    }
                }
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
                specification.source = if specification.kind == "a2a-agent-card" {
                    privacy.screen_json(&text)
                } else {
                    text
                };
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
        "http" | "https" | "ws" | "wss" | "mqtt" | "mqtts" | "postgres" | "postgresql" | "mysql"
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
        Self::from_workspace(workspace, false)
    }
    fn from_workspace(workspace: &Workspace, generation: bool) -> Self {
        let mut secrets = std::collections::BTreeSet::new();
        let mut pairs = |rows: &[moleapi_core::Pair]| {
            for row in rows {
                if generation && let Some(value) = &row.local_value {
                    secrets.insert(value.clone());
                }
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
        if generation {
            fn collect(value: &Value, secrets: &mut std::collections::BTreeSet<String>) {
                match value {
                    Value::Object(rows) => {
                        for (key, value) in rows {
                            if sensitive(key) {
                                collect_literal(value, secrets);
                            } else {
                                collect(value, secrets);
                            }
                        }
                    }
                    Value::Array(values) => {
                        for value in values {
                            collect(value, secrets);
                        }
                    }
                    _ => {}
                }
            }
            fn collect_literal(value: &Value, secrets: &mut std::collections::BTreeSet<String>) {
                match value {
                    Value::String(text) => {
                        secrets.insert(text.clone());
                    }
                    Value::Number(_) | Value::Bool(_) => {
                        secrets.insert(value.to_string());
                    }
                    Value::Array(values) => {
                        for value in values {
                            collect_literal(value, secrets);
                        }
                    }
                    Value::Object(rows) => {
                        for value in rows.values() {
                            collect_literal(value, secrets);
                        }
                    }
                    _ => {}
                }
            }
            for request in workspace.data.collections.iter().flat_map(|c| &c.requests) {
                if let moleapi_core::Protocol::Tcp { config } = &request.protocol {
                    if config.message.secret {
                        secrets.insert(config.message.payload_source.clone());
                    }
                    if let Ok(bytes) = moleapi_core::tcp_payload(&config.message)
                        && let Ok(text) = std::str::from_utf8(&bytes)
                    {
                        if config.message.secret {
                            secrets.insert(text.to_owned());
                        }
                        if let Ok(value) = serde_json::from_str::<Value>(text) {
                            collect(&value, &mut secrets);
                        }
                    }
                }
                secrets.insert(request.auth.username.clone());
                if request.auth.kind == "basic"
                    && !(request.auth.username.is_empty() && request.auth.password.is_empty())
                {
                    secrets.insert(format!(
                        "{}:{}",
                        request.auth.username, request.auth.password
                    ));
                }
                if let Some((_, query)) = request.url.split_once('?') {
                    for (key, value) in url::form_urlencoded::parse(
                        query.split('#').next().unwrap_or_default().as_bytes(),
                    ) {
                        if sensitive(&key) {
                            secrets.insert(value.into_owned());
                        }
                    }
                }
                if let Ok(url) = url::Url::parse(&request.url) {
                    secrets.insert(url.username().into());
                    secrets.insert(
                        percent_encoding::percent_decode_str(url.username())
                            .decode_utf8_lossy()
                            .into_owned(),
                    );
                    if let Some(password) = url.password() {
                        secrets.insert(password.into());
                        secrets.insert(
                            percent_encoding::percent_decode_str(password)
                                .decode_utf8_lossy()
                                .into_owned(),
                        );
                    }
                    for (key, value) in url.query_pairs() {
                        if sensitive(&key) {
                            secrets.insert(value.into_owned());
                        }
                    }
                }
                if (request.body_kind == "multipart" || request.body_kind.contains("{{"))
                    && let Ok(body) =
                        serde_json::from_str::<moleapi_core::MultipartBody>(&request.body)
                {
                    for part in body.parts {
                        if sensitive(&part.name)
                            && let moleapi_core::MultipartValue::Text { text, .. } = part.value
                        {
                            secrets.insert(text);
                        }
                    }
                }
                if request.body_kind == "form" {
                    for (key, value) in url::form_urlencoded::parse(request.body.as_bytes()) {
                        if sensitive(&key) {
                            secrets.insert(value.into_owned());
                        }
                    }
                } else if let Ok(value) = serde_json::from_str::<Value>(&request.body) {
                    collect(&value, &mut secrets);
                }
            }
            secrets.retain(|text| !template_reference(text));
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
        if generation {
            use base64::{
                Engine,
                engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
            };
            let mut patterns = std::collections::BTreeSet::new();
            for text in &secrets {
                patterns.insert(text.clone());
                patterns.insert(text.replace('\'', "''"));
                patterns.insert(text.replace('\\', "\\\\").replace('\'', "\\'"));
                patterns.insert(STANDARD.encode(text));
                patterns.insert(URL_SAFE_NO_PAD.encode(text));
                let form: String = url::form_urlencoded::byte_serialize(text.as_bytes()).collect();
                patterns.insert(form.clone());
                patterns.insert(form.replace('+', "%20"));
                patterns.insert(form.to_ascii_lowercase());
                if let Ok(mut url) = url::Url::parse("https://privacy.invalid/") {
                    url.set_path(text);
                    patterns.insert(url.path().trim_start_matches('/').into());
                }
            }
            if patterns.iter().map(String::len).sum::<usize>() > 65536 {
                return Self {
                    matcher: None,
                    withhold: true,
                };
            }
            secrets = patterns;
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
    fn screen_bounded(&self, text: &str, maximum: usize) -> String {
        if self.withhold {
            return String::new();
        }
        let Some(matcher) = &self.matcher else {
            return if text.len() <= maximum {
                text.into()
            } else {
                String::new()
            };
        };
        let mut output = String::new();
        let mut last = 0;
        for found in matcher.find_iter(text) {
            let prefix = &text[last..found.start()];
            if output.len().saturating_add(prefix.len()).saturating_add(10) > maximum {
                return String::new();
            }
            output.push_str(prefix);
            output.push_str("[REDACTED]");
            last = found.end();
        }
        let tail = &text[last..];
        if output.len().saturating_add(tail.len()) > maximum {
            return String::new();
        }
        output.push_str(tail);
        output
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
    fn screen_generation_text(&self, text: &str) -> String {
        use base64::{
            Engine,
            engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
        };
        let literal = self.screen_text(text);
        if literal != text {
            return literal;
        }
        // Whole string values and explicit Basic header values can carry an old
        // username plus a still-private password. Decode with the mature codec;
        // never scan arbitrary encodings or expose decoded data in output.
        let encoded = text.strip_prefix("Basic ").unwrap_or(text);
        if encoded.len() >= 8
            && encoded.len() <= 16384
            && let Ok(bytes) = STANDARD
                .decode(encoded)
                .or_else(|_| URL_SAFE_NO_PAD.decode(encoded))
            && let Ok(decoded) = std::str::from_utf8(&bytes)
            && self.screen_text(decoded) != decoded
        {
            return "[REDACTED]".into();
        }
        literal
    }
    fn screen_generation_json(&self, text: &str) -> String {
        let Ok(mut value) = serde_json::from_str::<Value>(text) else {
            return self.screen_generation_text(text);
        };
        fn walk(value: &mut Value, privacy: &ExportPrivacy) {
            match value {
                Value::String(text) => *text = privacy.screen_generation_text(text),
                Value::Array(values) => {
                    for value in values {
                        walk(value, privacy);
                    }
                }
                Value::Object(rows) => {
                    rows.retain(|key, _| privacy.screen_generation_text(key) == *key);
                    for value in rows.values_mut() {
                        walk(value, privacy);
                    }
                }
                _ => {}
            }
        }
        walk(&mut value, self);
        self.screen_json(&serde_json::to_string(&value).unwrap_or_default())
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

pub(super) fn generation_request(
    source: &Workspace,
    request_id: &str,
    include_secrets: bool,
) -> anyhow::Result<moleapi_core::RequestSpec> {
    let request = source
        .data
        .collections
        .iter()
        .flat_map(|c| &c.requests)
        .find(|r| r.id == request_id)
        .ok_or_else(|| anyhow::anyhow!("Request not found"))?;
    if include_secrets {
        return Ok(request.clone());
    }
    let privacy = ExportPrivacy::from_workspace(source, true);
    anyhow::ensure!(
        !privacy.withhold,
        "Request privacy budget exceeded; default generation withheld"
    );
    let mut request = request.clone();
    if !template_reference(&request.auth.token) {
        request.auth.token = "{{TOKEN}}".into();
    }
    if !template_reference(&request.auth.username) {
        request.auth.username = "{{USERNAME}}".into();
    }
    if !template_reference(&request.auth.password) {
        request.auth.password = "{{PASSWORD}}".into();
    }
    request.url = if let Ok(mut url) = url::Url::parse(&request.url) {
        let _ = url.set_username("");
        let _ = url.set_password(None);
        let rows = url
            .query_pairs()
            .map(|(key, value)| {
                let hidden = sensitive(&key);
                (
                    privacy.screen_generation_text(&key),
                    if hidden {
                        "{{REDACTED}}".into()
                    } else {
                        privacy.screen_generation_text(&value)
                    },
                )
            })
            .collect::<Vec<_>>();
        if !rows.is_empty() {
            url.query_pairs_mut().clear().extend_pairs(rows);
        }
        url.to_string()
    } else if let Some((base, query)) = request.url.split_once('?') {
        let (query, fragment) = query
            .split_once('#')
            .map(|(q, f)| (q, Some(f)))
            .unwrap_or((query, None));
        let query = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(
                url::form_urlencoded::parse(query.as_bytes()).map(|(key, value)| {
                    let hidden = sensitive(&key);
                    (
                        privacy.screen_generation_text(&key),
                        if hidden {
                            "{{REDACTED}}".into()
                        } else {
                            privacy.screen_generation_text(&value)
                        },
                    )
                }),
            )
            .finish();
        let mut result = format!("{}?{query}", privacy.screen_text(base));
        if let Some(fragment) = fragment {
            result.push('#');
            result.push_str(&privacy.screen_text(fragment));
        }
        result
    } else {
        privacy.screen_text(&request.url)
    };
    request.body = if request.body_kind == "form" {
        url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(url::form_urlencoded::parse(request.body.as_bytes()).map(
                |(key, value)| {
                    let hidden = sensitive(&key);
                    (
                        privacy.screen_generation_text(&key),
                        if hidden {
                            "{{REDACTED}}".into()
                        } else {
                            privacy.screen_generation_text(&value)
                        },
                    )
                },
            ))
            .finish()
    } else {
        redact_embedded_json(&request.body)
    };
    for row in request.headers.iter_mut().chain(request.query.iter_mut()) {
        row.local_value = None;
        if row.secret == Some(true) || sensitive(&row.key) {
            row.value = "{{REDACTED}}".into();
        }
    }
    // Screen user data, never the fixed RequestSpec/Pair/Auth schema keys.
    // A common credential such as "user" must not delete auth.username.
    request.url = privacy.screen_text(&request.url);
    request.body = if serde_json::from_str::<Value>(&request.body).is_ok() {
        privacy.screen_generation_json(&request.body)
    } else {
        privacy.screen_generation_text(&request.body)
    };
    for row in request.headers.iter_mut().chain(request.query.iter_mut()) {
        row.key = privacy.screen_generation_text(&row.key);
        row.value = privacy.screen_generation_text(&row.value);
    }
    request.auth.username = privacy.screen_text(&request.auth.username);
    request.auth.password = privacy.screen_text(&request.auth.password);
    request.auth.token = privacy.screen_text(&request.auth.token);
    Ok(request)
}

fn template_reference(text: &str) -> bool {
    text.strip_prefix("{{")
        .and_then(|s| s.strip_suffix("}}"))
        .is_some_and(|key| !key.trim().is_empty() && !key.contains(['{', '}']))
}
