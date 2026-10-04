use crate::*;
use anyhow::{Context, Result, bail, ensure};
use reqwest::header::{HeaderName, HeaderValue};
use std::collections::HashSet;
pub fn validate_request(r: &RequestSpec, templates: bool) -> Result<()> {
    crate::validate_a2a(r, templates)?;
    crate::validate_mcp(r, templates)?;
    validate_soap(r, templates)?;
    validate_graphql_draft(r, templates)?;
    validate_mqtt(r, templates)?;
    if let Protocol::Socketio {
        namespace,
        path,
        auth_source,
        listeners,
        event,
        arguments_source,
        attachments_base64,
        ..
    } = &r.protocol
    {
        ensure!(
            event.len() <= 256
                && arguments_source.len() <= 1024 * 1024
                && attachments_base64.len() <= 32
                && attachments_base64.iter().map(String::len).sum::<usize>() <= 1024 * 1024 * 2,
            "Socket.IO emit draft exceeds size limit"
        );
        ensure!(
            namespace.len() <= 256 && path.len() <= 1024 && auth_source.len() <= 1024 * 1024,
            "Socket.IO configuration exceeds size limit"
        );
        ensure!(
            listeners.len() <= 64 && listeners.iter().all(|event| event.len() <= 256),
            "Socket.IO listener limit reached"
        );
        if !templates {
            ensure!(
                namespace.starts_with('/')
                    && !namespace.contains([',', '?', '#'])
                    && !namespace.chars().any(char::is_control),
                "Invalid Socket.IO namespace"
            );
            ensure!(
                path.starts_with('/')
                    && path != "/"
                    && !path.contains(['?', '#'])
                    && !path.chars().any(char::is_control),
                "Invalid Socket.IO path"
            );
            let auth: serde_json::Value =
                serde_json::from_str(auth_source).context("Invalid Socket.IO auth JSON")?;
            ensure!(auth.is_object(), "Socket.IO auth must be a JSON object");
            for event in listeners {
                validate_socketio_event(event)?;
            }
            ensure!(
                r.method == "GET" && r.body_kind == "none",
                "Socket.IO requires GET with body mode None"
            );
            let url = protocol_url(&r.url, true)?;
            ensure!(
                !url.query_pairs()
                    .any(|(key, _)| matches!(key.as_ref(), "EIO" | "transport" | "sid")),
                "Socket.IO transport query keys are SDK-owned"
            );
            ensure!(
                !r.query
                    .iter()
                    .any(|p| p.enabled && matches!(p.key.as_str(), "EIO" | "transport" | "sid")),
                "Socket.IO transport query keys are SDK-owned"
            );
        }
    }
    if let Protocol::Grpc {
        service,
        method,
        message_source,
    } = &r.protocol
    {
        ensure!(
            service.len() <= 256 && method.len() <= 256 && message_source.len() <= 1024 * 1024,
            "gRPC draft exceeds size limit"
        );
        if !templates {
            ensure!(
                !service.is_empty() && !method.is_empty(),
                "Select a gRPC service and method"
            );
            serde_json::from_str::<serde_json::Value>(message_source)
                .context("Invalid gRPC JSON draft")?;
            ensure!(
                r.query.iter().all(|p| !p.enabled),
                "gRPC endpoints do not support URL query parameters"
            );
        }
    }
    crate::validate_tcp(r, templates)?;
    validate_script(&r.pre_request_script)?;
    validate_script(&r.post_response_script)?;
    ensure!(
        matches!(
            r.method.as_str(),
            "GET" | "POST" | "PATCH" | "PUT" | "DELETE" | "HEAD" | "OPTIONS"
        ) || (templates && r.method.contains("{{")),
        "Unsupported HTTP method"
    );
    ensure!(
        (1..=120_000).contains(&r.timeout_ms),
        "Timeout must be between 1 and 120000 ms"
    );
    ensure!(
        matches!(r.body_kind.as_str(), "none" | "json" | "text" | "form")
            || (templates && r.body_kind.contains("{{")),
        "Unsupported body kind"
    );
    ensure!(
        matches!(r.auth.kind.as_str(), "none" | "bearer" | "basic")
            || (templates && r.auth.kind.contains("{{")),
        "Unsupported authentication kind"
    );
    ensure!(r.body.len() <= MAX_BODY, "Request body exceeds 5 MiB");
    if !(templates && r.protocol.is_a2a() && r.url.is_empty())
        && !r.protocol.is_mqtt()
        && !r.protocol.is_tcp()
        && !matches!(&r.protocol, Protocol::Mcp { config } if config.transport == "stdio" || templates && r.url.is_empty())
        && (!templates || !r.url.contains("{{"))
    {
        protocol_url(
            &r.url,
            r.protocol == Protocol::Websocket || r.protocol.is_socketio(),
        )?;
    }
    if r.protocol.is_tcp() && (!templates || !r.url.contains("{{")) {
        crate::tcp_url(&r.url)?;
    }
    if !r.protocol.is_graphql() && r.body_kind == "json" && (!templates || !r.body.contains("{{")) {
        serde_json::from_str::<serde_json::Value>(&r.body).context("Invalid JSON body")?;
    }
    for h in &r.headers {
        if !h.enabled {
            continue;
        }
        if !templates || !h.key.contains("{{") {
            HeaderName::from_bytes(h.key.as_bytes()).context("Invalid header name")?;
        }
        if !templates || !h.value.contains("{{") {
            HeaderValue::from_str(&h.value).context("Invalid header value")?;
        }
    }
    let mut assertion_ids = HashSet::new();
    for a in &r.assertions {
        ensure!(
            !a.id.is_empty() && assertion_ids.insert(&a.id),
            "Assertion IDs must be unique and nonempty"
        );
        if templates && a.kind.contains("{{") {
            continue;
        }
        match a.kind.as_str() {
            "status" | "duration" => {
                if !templates || !a.expected.contains("{{") {
                    a.expected
                        .parse::<u64>()
                        .context("Assertion expected value must be numeric")?;
                }
            }
            "contains" => {}
            "json" => {
                ensure!(
                    a.target.is_empty()
                        || a.target.starts_with('/')
                        || (templates && a.target.contains("{{")),
                    "JSON assertion target must be a JSON pointer"
                );
                if !templates || !a.target.contains("{{") {
                    validate_pointer(&a.target)?;
                }
                if !templates || !a.expected.contains("{{") {
                    serde_json::from_str::<serde_json::Value>(&a.expected)
                        .context("JSON assertion expected value must be valid JSON")?;
                }
            }
            _ => bail!("Unsupported assertion kind"),
        }
    }
    for example in &r.examples {
        ensure!(
            (200..=599).contains(&example.status),
            "Example status must be 200–599"
        );
        ensure!(example.body.len() <= MAX_BODY, "Example body exceeds 5 MiB");
        for h in example.headers.iter().filter(|h| h.enabled) {
            if !templates || !h.key.contains("{{") {
                HeaderName::from_bytes(h.key.as_bytes()).context("Invalid example header")?;
            }
            if !templates || !h.value.contains("{{") {
                HeaderValue::from_str(&h.value).context("Invalid example header")?;
            }
        }
    }
    Ok(())
}
pub fn validate_workspace(data: &WorkspaceData) -> Result<()> {
    ensure!(
        data.schema_version == 1,
        "Unsupported workspace schema version"
    );
    validate_variables(&data.global_variables)?;
    validate_script(&data.pre_request_script)?;
    validate_script(&data.post_response_script)?;
    let mut specifications = HashSet::new();
    for spec in &data.specifications {
        ensure!(
            !spec.id.is_empty() && specifications.insert(&spec.id),
            "Specification IDs must be unique and nonempty"
        );
        ensure!(spec.source.len() <= MAX_BODY, "Specification exceeds 5 MiB");
        if spec.kind == "a2a-agent-card" {
            ensure!(
                matches!(spec.dialect.as_str(), "a2a-0.3" | "a2a-1.0"),
                "Unsupported Agent Card dialect"
            );
            ensure!(spec.source.len() <= 1024 * 1024, "Agent Card exceeds 1 MiB");
            crate::validate_mcp_json(
                &serde_json::from_str::<serde_json::Value>(&spec.source)
                    .context("Invalid Agent Card JSON")?,
            )?;
        }
        if spec.kind == "wsdl" {
            soap_schema(spec)?;
        }
        if spec.kind == "protobuf" {
            protobuf_pool(spec)?;
        }
        if matches!(spec.kind.as_str(), "graphql-sdl" | "graphql-introspection") {
            graphql_schema_sdl(spec)?;
        }
    }
    let mut collections = HashSet::new();
    let mut requests = HashSet::new();
    let mut environments = HashSet::new();
    for c in &data.collections {
        validate_variables(&c.variables)?;
        validate_script(&c.pre_request_script)?;
        validate_script(&c.post_response_script)?;
        ensure!(
            !c.id.is_empty() && collections.insert(&c.id),
            "Collection IDs must be unique and nonempty"
        );
        for r in &c.requests {
            ensure!(
                !r.id.is_empty() && requests.insert(&r.id),
                "Request IDs must be unique and nonempty"
            );
            validate_request(r, true)?;
            if let Some(id) = &r.specification_id {
                ensure!(
                    specifications.contains(id),
                    "Request specification does not exist"
                );
            }
            let mut examples = HashSet::new();
            for e in &r.examples {
                ensure!(
                    !e.id.is_empty() && examples.insert(&e.id),
                    "Example IDs must be unique and nonempty"
                );
            }
        }
    }
    for e in &data.environments {
        validate_variables(&e.variables)?;
        ensure!(
            !e.id.is_empty() && environments.insert(&e.id),
            "Environment IDs must be unique and nonempty"
        );
    }
    if let Some(id) = &data.active_environment_id {
        ensure!(
            environments.contains(id),
            "Active environment does not exist"
        );
    }
    ensure!(
        serde_json::to_vec(data)?.len() <= 20 * 1024 * 1024,
        "Workspace exceeds 20 MiB"
    );
    Ok(())
}

fn validate_pointer(pointer: &str) -> Result<()> {
    ensure!(
        pointer.is_empty() || pointer.starts_with('/'),
        "JSON assertion target must be a JSON pointer"
    );
    let mut chars = pointer.chars();
    while let Some(c) = chars.next() {
        if c == '~' {
            ensure!(
                matches!(chars.next(), Some('0' | '1')),
                "Invalid JSON pointer escape"
            );
        }
    }
    Ok(())
}

/// Bound user-controlled names and reject the official Socket.IO reserved event set.
pub fn validate_socketio_event(event: &str) -> Result<()> {
    ensure!(
        !event.is_empty() && event.len() <= 256 && !event.chars().any(char::is_control),
        "Invalid Socket.IO event name"
    );
    ensure!(
        !matches!(
            event,
            "connect"
                | "connect_error"
                | "disconnect"
                | "disconnecting"
                | "newListener"
                | "removeListener"
        ),
        "Reserved Socket.IO event name"
    );
    Ok(())
}
