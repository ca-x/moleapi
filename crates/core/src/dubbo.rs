//! Saved classic Dubbo drafts. Apache Dubbo owns Hessian and invocation in the worker.
use crate::{Pair, Protocol, RequestSpec};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const MAX_ARGUMENT_SOURCE: usize = 64 * 1024;
const MAX_ARGUMENTS_TOTAL: usize = 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct DubboArgument {
    pub id: String,
    pub type_name: String,
    pub value_source: String,
    pub secret: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DubboRegistryAuth {
    pub username: String,
    pub password: String,
}

impl Default for DubboArgument {
    fn default() -> Self {
        Self {
            id: String::new(),
            type_name: String::new(),
            value_source: "null".into(),
            secret: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct DubboConfig {
    pub source_version: u8,
    #[serde(skip_serializing_if = "is_direct_source")]
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry_auth: Option<DubboRegistryAuth>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub registry_namespace: String,
    pub transport: String,
    pub serialization: String,
    pub service: String,
    pub method: String,
    pub version: String,
    pub group: String,
    pub arguments: Vec<DubboArgument>,
    pub attachments: Vec<Pair>,
}

fn is_direct_source(value: &str) -> bool {
    value == "direct"
}

impl Default for DubboConfig {
    fn default() -> Self {
        Self {
            source_version: 1,
            source: "direct".into(),
            registry_auth: None,
            registry_namespace: String::new(),
            transport: "classic".into(),
            serialization: "hessian2".into(),
            service: String::new(),
            method: String::new(),
            version: String::new(),
            group: String::new(),
            arguments: Vec::new(),
            attachments: Vec::new(),
        }
    }
}

fn java_name(value: &str, qualified: bool) -> bool {
    let base = value.trim_end_matches("[]");
    !base.is_empty()
        && base.len() <= 256
        && (qualified || !base.contains('.'))
        && base.split('.').all(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .is_some_and(|first| first.is_ascii_alphabetic() || matches!(first, '_' | '$'))
                && chars.all(|next| next.is_ascii_alphanumeric() || matches!(next, '_' | '$'))
        })
}

fn parameter_type(value: &str) -> bool {
    matches!(
        value,
        "boolean" | "byte" | "char" | "short" | "int" | "long" | "float" | "double"
    ) || java_name(value, true)
}

pub fn dubbo_url(raw: &str) -> Result<url::Url> {
    let parsed = url::Url::parse(raw).context("Invalid Dubbo endpoint")?;
    ensure!(
        parsed.scheme() == "dubbo"
            && parsed.host_str().is_some()
            && parsed.port().is_some_and(|port| port > 0)
            && parsed.username().is_empty()
            && parsed.password().is_none()
            && matches!(parsed.path(), "" | "/")
            && parsed.query().is_none()
            && parsed.fragment().is_none(),
        "Classic Dubbo requires a direct dubbo://host:port endpoint"
    );
    Ok(parsed)
}

pub fn dubbo_registry_url(raw: &str, scheme: &str) -> Result<url::Url> {
    ensure!(raw.len() <= 2048, "Dubbo registry URL exceeds limit");
    let url = url::Url::parse(raw).context("Invalid Dubbo registry URL")?;
    ensure!(
        url.scheme() == scheme
            && url.host_str().is_some()
            && url.port().is_some_and(|port| port > 0)
            && url.username().is_empty()
            && url.password().is_none()
            && matches!(url.path(), "" | "/")
            && url.query().is_none()
            && url.fragment().is_none(),
        "Unsupported Dubbo registry endpoint"
    );
    Ok(url)
}

/// Validate one untrusted registry result before the host resolves or connects it.
pub fn dubbo_provider_url(raw: &str, config: &DubboConfig) -> Result<url::Url> {
    ensure!(
        matches!(config.source_version, 1 | 2)
            && config.transport == "classic"
            && config.serialization == "hessian2",
        "Unsupported Dubbo provider selection mode"
    );
    ensure!(raw.len() <= 4096, "Dubbo provider URL exceeds 4 KiB");
    let url = url::Url::parse(raw).context("Invalid Dubbo provider URL")?;
    ensure!(
        url.scheme() == "dubbo"
            && url.host_str().is_some()
            && url.port().is_some_and(|port| port > 0)
            && url.username().is_empty()
            && url.password().is_none()
            && url.path() == format!("/{}", config.service)
            && url.fragment().is_none(),
        "Registry result is not a matching direct Dubbo provider"
    );
    let mut fields = std::collections::HashMap::new();
    for (index, (name, value)) in url.query_pairs().enumerate() {
        ensure!(index < 128, "Dubbo provider metadata exceeds 128 fields");
        if matches!(
            name.as_ref(),
            "side"
                | "version"
                | "group"
                | "serialization"
                | "prefer.serialization"
                | "enabled"
                | "category"
        ) {
            ensure!(
                fields
                    .insert(name.into_owned(), value.into_owned())
                    .is_none(),
                "Duplicate Dubbo provider control field"
            );
        }
    }
    let get = |key| fields.get(key).map(String::as_str);
    ensure!(
        get("side") == Some("provider"),
        "Registry result is not a provider"
    );
    ensure!(
        get("version").unwrap_or("") == config.version,
        "Dubbo provider version differs from request"
    );
    ensure!(
        get("group").unwrap_or("") == config.group,
        "Dubbo provider group differs from request"
    );
    ensure!(
        get("serialization").unwrap_or("hessian2") == "hessian2",
        "Dubbo provider does not support Hessian2"
    );
    if let Some(preferred) = get("prefer.serialization") {
        ensure!(
            preferred.split(',').any(|part| part.trim() == "hessian2"),
            "Dubbo provider does not advertise Hessian2"
        );
    }
    ensure!(
        get("enabled").is_none_or(|value| value == "true"),
        "Dubbo provider is disabled"
    );
    ensure!(
        get("category").is_none_or(|value| value == "providers"),
        "Dubbo registry category is not providers"
    );
    Ok(url)
}

pub fn validate_dubbo(request: &RequestSpec, templates: bool) -> Result<()> {
    let Protocol::Dubbo { config } = &request.protocol else {
        return Ok(());
    };
    ensure!(
        (config.source_version == 1 && config.source == "direct")
            || (config.source_version == 2
                && matches!(config.source.as_str(), "zookeeper" | "nacos")),
        "Unsupported Dubbo source version"
    );
    if let Some(auth) = &config.registry_auth {
        ensure!(
            matches!(config.source.as_str(), "zookeeper" | "nacos"),
            "Dubbo registry authentication requires ZooKeeper or Nacos"
        );
        ensure!(
            !auth.username.is_empty()
                && auth.username.len() <= 128
                && !auth.username.contains(':')
                && !auth.username.chars().any(char::is_control)
                && !auth.password.is_empty()
                && auth.password.len() <= 4096
                && !auth.password.chars().any(char::is_control),
            "Invalid Dubbo registry credentials"
        );
    }
    ensure!(
        config.registry_namespace.is_empty()
            || config.source == "nacos"
                && config.registry_namespace.len() <= 128
                && config
                    .registry_namespace
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')),
        "Invalid Nacos namespace ID"
    );
    ensure!(
        config.transport == "classic" && config.serialization == "hessian2",
        "Only classic Dubbo/Hessian2 is currently selected"
    );
    ensure!(
        config.service.len() <= 256
            && config.method.len() <= 128
            && config.version.len() <= 128
            && config.group.len() <= 128,
        "Dubbo service or method exceeds size limit"
    );
    if config.source == "nacos" {
        ensure!(
            !config.version.is_empty() && !config.group.is_empty(),
            "Nacos discovery requires explicit Dubbo version and group"
        );
    }
    if !templates {
        ensure!(
            java_name(&config.service, true) && config.service.contains('.'),
            "Invalid Dubbo service interface"
        );
        ensure!(java_name(&config.method, false), "Invalid Dubbo method");
    }
    for label in [&config.version, &config.group] {
        if !templates || !label.contains("{{") {
            ensure!(
                label
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')),
                "Invalid Dubbo version/group"
            );
        }
    }
    ensure!(
        request.method == "GET" && request.body_kind == "none" && request.body.is_empty(),
        "Dubbo uses GET marker with no HTTP body"
    );
    ensure!(
        request.query.iter().all(|row| !row.enabled)
            && request.headers.iter().all(|row| !row.enabled),
        "Dubbo arguments and attachments are not HTTP query/headers"
    );
    ensure!(
        matches!(request.auth.kind.as_str(), "none" | "inherit"),
        "Dubbo credentials require explicit attachments"
    );
    ensure!(
        request.network.is_none(),
        "Dubbo network settings require a verified transport mapping"
    );
    ensure!(
        config.arguments.len() <= 32,
        "Dubbo argument limit exceeded"
    );
    let mut ids = HashSet::new();
    let mut total = 0usize;
    for argument in &config.arguments {
        ensure!(
            !argument.id.is_empty() && argument.id.len() <= 128 && ids.insert(&argument.id),
            "Dubbo argument IDs must be unique and nonempty"
        );
        ensure!(
            argument.type_name.len() <= 256
                && (templates && argument.type_name.is_empty()
                    || parameter_type(&argument.type_name)),
            "Invalid Dubbo parameter type"
        );
        ensure!(
            argument.value_source.len() <= MAX_ARGUMENT_SOURCE,
            "Dubbo argument source exceeds 64 KiB"
        );
        total = total.saturating_add(argument.value_source.len());
        ensure!(total <= MAX_ARGUMENTS_TOTAL, "Dubbo arguments exceed 1 MiB");
        if !templates || !argument.value_source.contains("{{") {
            serde_json::from_str::<serde_json::Value>(&argument.value_source)
                .context("Invalid Dubbo argument JSON")?;
        }
    }
    ensure!(
        config.attachments.len() <= 64,
        "Dubbo attachment limit exceeded"
    );
    let mut attachment_keys = HashSet::new();
    let mut attachment_total = 0usize;
    for row in config.attachments.iter().filter(|row| row.enabled) {
        ensure!(
            !row.id.is_empty() && row.id.len() <= 128,
            "Invalid Dubbo attachment ID"
        );
        ensure!(
            row.key.len() <= 128
                && row.value.len() <= 4096
                && row
                    .local_value
                    .as_ref()
                    .is_none_or(|value| value.len() <= 4096),
            "Dubbo attachment exceeds size limit"
        );
        attachment_total = attachment_total.saturating_add(
            row.key.len() + row.value.len() + row.local_value.as_ref().map_or(0, String::len),
        );
        ensure!(
            attachment_total <= 64 * 1024,
            "Dubbo attachments exceed 64 KiB"
        );
        if !templates || !row.key.contains("{{") {
            ensure!(
                row.key
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
                    && !row.key.is_empty(),
                "Invalid Dubbo attachment name"
            );
            ensure!(
                !matches!(
                    row.key.to_ascii_lowercase().as_str(),
                    "path" | "version" | "group" | "interface" | "method"
                ),
                "Dubbo attachment name is protocol-owned"
            );
            ensure!(
                attachment_keys.insert(row.key.to_ascii_lowercase()),
                "Duplicate Dubbo attachment name"
            );
        }
    }
    if !templates || !request.url.contains("{{") {
        if config.source == "direct" {
            dubbo_url(&request.url)?;
        } else {
            dubbo_registry_url(&request.url, &config.source)?;
        }
    }
    Ok(())
}
