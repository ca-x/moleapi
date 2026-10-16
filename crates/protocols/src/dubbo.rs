//! One-shot Apache Dubbo GenericService worker with Rust-owned network admission.
use anyhow::{Context, Result, bail, ensure};
use futures_util::StreamExt;
use moleapi_core::{
    DubboConfig, DubboRegistryAuth, NetworkPolicy, Protocol, RequestSpec, Response, ScriptLog,
    checked_destination, dubbo_provider_url, dubbo_registry_url, dubbo_url, public_ip,
};
use percent_encoding::percent_decode_str;
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    net::{IpAddr, SocketAddr},
    path::Path,
    process::Stdio,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::{Child, Command},
};

pub const WORKER_JAR_SHA256: &str =
    "154c9a82bdebc496319880c13731b0444f47a840458328a3810bc1402443dd56";
const MAX_INPUT: usize = 1024 * 1024;
const MAX_OUTPUT: usize = 1024 * 1024;
const MAX_STDERR: usize = 16 * 1024;

#[derive(Clone, Debug)]
pub struct CheckedProvider {
    /// Original registry value; callers must screen it before history or UI output.
    source: String,
    address: SocketAddr,
}

impl CheckedProvider {
    pub fn original_url(&self) -> &str {
        &self.source
    }

    pub fn destination(&self) -> SocketAddr {
        self.address
    }
}

/// Admit the complete untrusted registry snapshot before preflighting one provider.
pub async fn select_registry_provider(
    config: &DubboConfig,
    sources: &[String],
    policy: NetworkPolicy,
    timeout: Duration,
) -> Result<CheckedProvider> {
    ensure!(
        !sources.is_empty() && sources.len() <= 256,
        "Dubbo registry provider count must be 1–256"
    );
    ensure!(
        sources.iter().map(String::len).sum::<usize>() <= MAX_INPUT,
        "Dubbo registry provider snapshot exceeds 1 MiB"
    );
    let deadline = Instant::now() + timeout;
    let mut providers = sources
        .iter()
        .map(|raw| Ok((raw.clone(), dubbo_provider_url(raw, config)?)))
        .collect::<Result<Vec<_>>>()?;
    providers.sort_by(|left, right| left.0.cmp(&right.0));
    let mut checked = Vec::with_capacity(providers.len());
    for (source, url) in providers {
        let addresses = tokio::time::timeout(
            deadline.saturating_duration_since(Instant::now()),
            checked_destination(&url, policy),
        )
        .await
        .context("Dubbo registry provider admission timed out")??;
        checked.push((source, addresses));
    }
    for (source, addresses) in checked {
        if let Ok(address) = reachable_address(&addresses, deadline).await {
            return Ok(CheckedProvider { source, address });
        }
    }
    bail!("No reachable checked Dubbo provider")
}

/// Read one ZooKeeper provider snapshot. The SDK only sees a checked literal IP.
pub async fn lookup_zookeeper(
    registry: &str,
    config: &DubboConfig,
    policy: NetworkPolicy,
    timeout: Duration,
) -> Result<Vec<String>> {
    let deadline = Instant::now() + timeout;
    let url = dubbo_registry_url(registry, "zookeeper")?;
    let addresses = tokio::time::timeout(
        deadline.saturating_duration_since(Instant::now()),
        checked_destination(&url, policy),
    )
    .await
    .context("ZooKeeper address lookup timed out")??;
    let address = reachable_address(&addresses, deadline)
        .await
        .context("ZooKeeper registry connection failed")?;
    let remaining = deadline.saturating_duration_since(Instant::now());
    ensure!(!remaining.is_zero(), "ZooKeeper lookup timed out");
    let client = tokio::time::timeout(
        remaining,
        zookeeper_async::ZooKeeper::connect(&address.to_string(), remaining, |_| {}),
    )
    .await
    .context("ZooKeeper connection timed out")
    .and_then(|result| result.map_err(anyhow::Error::from))?;
    if let Some(auth) = &config.registry_auth {
        let credentials = format!("{}:{}", auth.username, auth.password);
        tokio::time::timeout(
            deadline.saturating_duration_since(Instant::now()),
            client.add_auth("digest", credentials.into_bytes()),
        )
        .await
        .context("ZooKeeper authentication timed out")?
        .context("ZooKeeper authentication failed")?;
    }
    let encoded_service: String =
        url::form_urlencoded::byte_serialize(config.service.as_bytes()).collect();
    let path = format!("/dubbo/{encoded_service}/providers");
    let lookup = tokio::time::timeout(
        deadline.saturating_duration_since(Instant::now()),
        client.get_children(&path, false),
    )
    .await
    .context("ZooKeeper provider lookup timed out")?;
    let _ = tokio::time::timeout(
        deadline
            .saturating_duration_since(Instant::now())
            .min(Duration::from_millis(300)),
        client.close(),
    )
    .await;
    let children = match lookup {
        Ok(children) => children,
        Err(zookeeper_async::ZkError::NoNode) => Vec::new(),
        Err(error) => return Err(error).context("ZooKeeper provider lookup failed"),
    };
    ensure!(
        children.len() <= 256,
        "ZooKeeper provider count exceeds limit"
    );
    let mut sources = Vec::with_capacity(children.len());
    let mut total = 0usize;
    for child in children {
        ensure!(
            child.len() <= 12 * 1024,
            "ZooKeeper provider node exceeds limit"
        );
        let source = percent_decode_str(&child)
            .decode_utf8()
            .context("Invalid ZooKeeper provider node encoding")?
            .into_owned();
        ensure!(source.len() <= 4096, "ZooKeeper provider URL exceeds limit");
        total += source.len();
        ensure!(
            total <= MAX_INPUT,
            "ZooKeeper provider snapshot exceeds 1 MiB"
        );
        if registry_candidate_matches(&source, config)? {
            sources.push(source);
        }
    }
    Ok(sources)
}

#[derive(Clone, Debug, Serialize)]
pub struct DubboMetadataMethod {
    pub name: String,
    pub parameter_types: Vec<String>,
    pub return_type: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct DubboMetadataPreview {
    pub service: String,
    pub version: String,
    pub group: String,
    pub application: String,
    pub metadata_path: String,
    pub document_sha256: String,
    pub raw_document: String,
    pub methods: Vec<DubboMetadataMethod>,
}

/// Read a provider-published service definition from an explicit metadata center.
pub async fn preview_zookeeper_metadata(
    endpoint: &str,
    config: &DubboConfig,
    application: &str,
    auth: Option<&DubboRegistryAuth>,
    policy: NetworkPolicy,
    timeout: Duration,
) -> Result<DubboMetadataPreview> {
    ensure!(
        !config.service.is_empty()
            && config.service.len() <= 256
            && config
                .service
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '$')),
        "Invalid metadata service interface"
    );
    for label in [&config.version, &config.group, application] {
        ensure!(
            !label.is_empty()
                && label.len() <= 128
                && label
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')),
            "Metadata version, group and application must be explicit path-safe values"
        );
    }
    let deadline = Instant::now() + timeout;
    let registry = dubbo_registry_url(endpoint, "zookeeper")?;
    let addresses = tokio::time::timeout(
        deadline.saturating_duration_since(Instant::now()),
        checked_destination(&registry, policy),
    )
    .await
    .context("Metadata center address lookup timed out")??;
    let address = reachable_address(&addresses, deadline)
        .await
        .context("Metadata center connection failed")?;
    let remaining = deadline.saturating_duration_since(Instant::now());
    ensure!(!remaining.is_zero(), "Metadata center lookup timed out");
    let client = tokio::time::timeout(
        remaining,
        zookeeper_async::ZooKeeper::connect(&address.to_string(), remaining, |_| {}),
    )
    .await
    .context("Metadata center connection timed out")
    .and_then(|result| result.map_err(anyhow::Error::from))?;
    if let Some(auth) = auth {
        ensure!(
            !auth.username.is_empty()
                && auth.username.len() <= 128
                && !auth.username.contains(':')
                && !auth.username.chars().any(char::is_control)
                && !auth.password.is_empty()
                && auth.password.len() <= 4096
                && !auth.password.chars().any(char::is_control),
            "Invalid metadata center credentials"
        );
        tokio::time::timeout(
            deadline.saturating_duration_since(Instant::now()),
            client.add_auth(
                "digest",
                format!("{}:{}", auth.username, auth.password).into_bytes(),
            ),
        )
        .await
        .context("Metadata center authentication timed out")?
        .context("Metadata center authentication failed")?;
    }
    let encoded_service: String =
        url::form_urlencoded::byte_serialize(config.service.as_bytes()).collect();
    let path = format!(
        "/dubbo/metadata/{encoded_service}/{}/{}/provider/{application}",
        config.version, config.group
    );
    let result = tokio::time::timeout(
        deadline.saturating_duration_since(Instant::now()),
        client.get_data(&path, false),
    )
    .await
    .context("Metadata definition lookup timed out")?;
    let _ = tokio::time::timeout(
        deadline
            .saturating_duration_since(Instant::now())
            .min(Duration::from_millis(300)),
        client.close(),
    )
    .await;
    let (bytes, _) = result.context("Metadata definition not found or unreadable")?;
    ensure!(
        bytes.len() <= MAX_INPUT,
        "Metadata definition exceeds 1 MiB"
    );
    let raw_document = String::from_utf8(bytes).context("Metadata definition is not UTF-8")?;
    let document_sha256 = format!("{:x}", Sha256::digest(raw_document.as_bytes()));
    let definition: Value =
        serde_json::from_str(&raw_document).context("Invalid Dubbo metadata definition")?;
    ensure!(
        definition.get("canonicalName").and_then(Value::as_str) == Some(config.service.as_str()),
        "Metadata definition service mismatch"
    );
    let source_methods = definition
        .get("methods")
        .and_then(Value::as_array)
        .context("Metadata definition omitted methods")?;
    ensure!(
        source_methods.len() <= 256,
        "Metadata method count exceeds limit"
    );
    let mut methods = Vec::with_capacity(source_methods.len());
    for method in source_methods {
        let name = method
            .get("name")
            .and_then(Value::as_str)
            .context("Metadata method name missing")?;
        ensure!(
            !name.is_empty() && name.len() <= 128,
            "Invalid metadata method name"
        );
        let source_types = method
            .get("parameterTypes")
            .and_then(Value::as_array)
            .context("Metadata method parameters missing")?;
        ensure!(
            source_types.len() <= 32,
            "Metadata parameter count exceeds limit"
        );
        let mut parameter_types = Vec::with_capacity(source_types.len());
        for value in source_types {
            let name = value.as_str().context("Invalid metadata parameter type")?;
            ensure!(
                !name.is_empty() && name.len() <= 256,
                "Invalid metadata parameter type"
            );
            parameter_types.push(name.to_owned());
        }
        let return_type = method
            .get("returnType")
            .and_then(Value::as_str)
            .context("Metadata method return type missing")?;
        ensure!(
            !return_type.is_empty() && return_type.len() <= 256,
            "Invalid metadata return type"
        );
        methods.push(DubboMetadataMethod {
            name: name.to_owned(),
            parameter_types,
            return_type: return_type.to_owned(),
        });
    }
    Ok(DubboMetadataPreview {
        service: config.service.clone(),
        version: config.version.clone(),
        group: config.group.clone(),
        application: application.to_owned(),
        metadata_path: path,
        document_sha256,
        raw_document,
        methods,
    })
}

fn registry_candidate_matches(source: &str, config: &DubboConfig) -> Result<bool> {
    let url = url::Url::parse(source).context("Invalid ZooKeeper provider URL")?;
    let mut version = None;
    let mut group = None;
    for (index, (name, value)) in url.query_pairs().enumerate() {
        ensure!(
            index < 128,
            "ZooKeeper provider metadata exceeds 128 fields"
        );
        let field = match name.as_ref() {
            "version" => &mut version,
            "group" => &mut group,
            _ => continue,
        };
        ensure!(
            field.replace(value.into_owned()).is_none(),
            "Duplicate ZooKeeper provider group/version field"
        );
    }
    Ok(version.as_deref().unwrap_or("") == config.version
        && group.as_deref().unwrap_or("") == config.group)
}

/// Nacos v2's published instance-list API, pinned to one Rust-admitted address.
/// The returned instance metadata is reconstructed as Apache Dubbo's provider URL.
pub async fn lookup_nacos(
    registry: &str,
    config: &DubboConfig,
    policy: NetworkPolicy,
    timeout: Duration,
) -> Result<Vec<String>> {
    let deadline = Instant::now() + timeout;
    let registry_url = dubbo_registry_url(registry, "nacos")?;
    let addresses = tokio::time::timeout(
        deadline.saturating_duration_since(Instant::now()),
        checked_destination(&registry_url, policy),
    )
    .await
    .context("Nacos address lookup timed out")??;
    let address = reachable_address(&addresses, deadline)
        .await
        .context("Nacos registry connection failed")?;
    ensure!(
        config.registry_auth.is_none() || !public_ip(address.ip()),
        "Nacos HTTP credentials require a private registry address"
    );
    let host = registry_url
        .host_str()
        .context("Nacos host missing")?
        .trim_matches(['[', ']']);
    let mut endpoint = url::Url::parse("http://127.0.0.1")?;
    endpoint
        .set_host(Some(host))
        .map_err(|_| anyhow::anyhow!("Invalid Nacos host"))?;
    endpoint
        .set_port(registry_url.port())
        .map_err(|_| anyhow::anyhow!("Invalid Nacos port"))?;
    endpoint.set_path("/nacos/v1/ns/instance/list");
    endpoint
        .query_pairs_mut()
        .append_pair(
            "serviceName",
            &format!(
                "providers:{}:{}:{}",
                config.service, config.version, config.group
            ),
        )
        .append_pair("groupName", "DEFAULT_GROUP")
        .append_pair("healthyOnly", "false");
    if !config.registry_namespace.is_empty() {
        endpoint
            .query_pairs_mut()
            .append_pair("namespaceId", &config.registry_namespace);
    }
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .resolve(host, address)
        .timeout(deadline.saturating_duration_since(Instant::now()))
        .build()?;
    let mut listing = client.get(endpoint.clone());
    if let Some(auth) = &config.registry_auth {
        let mut login_endpoint = endpoint.clone();
        login_endpoint.set_path("/nacos/v1/auth/users/login");
        login_endpoint.set_query(None);
        let form = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("username", &auth.username)
            .append_pair("password", &auth.password)
            .finish();
        let login = tokio::time::timeout(
            deadline.saturating_duration_since(Instant::now()),
            client
                .post(login_endpoint)
                .header(
                    reqwest::header::CONTENT_TYPE,
                    "application/x-www-form-urlencoded",
                )
                .body(form)
                .send(),
        )
        .await
        .context("Nacos login timed out")?
        .context("Nacos login failed")?;
        ensure!(
            login.status().is_success(),
            "Nacos login returned {}",
            login.status()
        );
        let login_body = bounded_nacos_body(login, 16 * 1024, deadline).await?;
        let login_data: Value =
            serde_json::from_slice(&login_body).context("Invalid Nacos login response")?;
        let token = login_data
            .get("accessToken")
            .and_then(Value::as_str)
            .context("Nacos login did not return an access token")?;
        ensure!(
            !token.is_empty() && token.len() <= 4096 && token.is_ascii(),
            "Invalid Nacos access token"
        );
        listing = listing.bearer_auth(token);
    }
    let response = tokio::time::timeout(
        deadline.saturating_duration_since(Instant::now()),
        listing.send(),
    )
    .await
    .context("Nacos provider lookup timed out")?
    .context("Nacos provider lookup failed")?;
    ensure!(
        response.status().is_success(),
        "Nacos provider lookup returned {}",
        response.status()
    );
    let body = bounded_nacos_body(response, MAX_INPUT, deadline).await?;
    let data: Value = serde_json::from_slice(&body).context("Invalid Nacos provider response")?;
    let instances = data
        .get("hosts")
        .and_then(Value::as_array)
        .context("Nacos provider response omitted hosts")?;
    ensure!(instances.len() <= 256, "Nacos provider count exceeds limit");
    let mut sources = Vec::new();
    let mut total = 0usize;
    for instance in instances {
        ensure!(
            instance.get("healthy").and_then(Value::as_bool).is_some()
                && instance.get("enabled").and_then(Value::as_bool).is_some(),
            "Invalid Nacos provider state"
        );
        if instance["healthy"] != true || instance["enabled"] != true {
            continue;
        }
        let ip: IpAddr = instance
            .get("ip")
            .and_then(Value::as_str)
            .context("Nacos provider IP missing")?
            .parse()
            .context("Invalid Nacos provider IP")?;
        let port: u16 = instance
            .get("port")
            .and_then(Value::as_u64)
            .context("Nacos provider port missing")?
            .try_into()
            .context("Invalid Nacos provider port")?;
        ensure!(port > 0, "Invalid Nacos provider port");
        let metadata = instance
            .get("metadata")
            .and_then(Value::as_object)
            .context("Nacos provider metadata missing")?;
        ensure!(
            metadata.len() <= 128,
            "Nacos provider metadata exceeds limit"
        );
        let path = metadata
            .get("path")
            .and_then(Value::as_str)
            .context("Nacos provider path missing")?;
        ensure!(
            path == config.service
                && metadata.get("protocol").and_then(Value::as_str) == Some("dubbo"),
            "Nacos instance is not a matching classic Dubbo provider"
        );
        let mut provider = url::Url::parse(&format!("dubbo://{}", SocketAddr::new(ip, port)))?;
        provider.set_path(&format!("/{path}"));
        let mut fields = metadata.iter().collect::<Vec<_>>();
        fields.sort_by(|left, right| left.0.cmp(right.0));
        for (key, value) in fields {
            provider.query_pairs_mut().append_pair(
                key,
                value
                    .as_str()
                    .context("Invalid Nacos provider metadata value")?,
            );
        }
        let source = provider.to_string();
        ensure!(source.len() <= 4096, "Nacos provider URL exceeds limit");
        total += source.len();
        ensure!(total <= MAX_INPUT, "Nacos provider snapshot exceeds 1 MiB");
        sources.push(source);
    }
    Ok(sources)
}

async fn bounded_nacos_body(
    response: reqwest::Response,
    limit: usize,
    deadline: Instant,
) -> Result<Vec<u8>> {
    ensure!(
        response
            .content_length()
            .is_none_or(|length| length <= limit as u64),
        "Nacos response exceeds {limit} bytes"
    );
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = tokio::time::timeout(
        deadline.saturating_duration_since(Instant::now()),
        stream.next(),
    )
    .await
    .context("Nacos response timed out")?
    {
        let chunk = chunk.context("Nacos response failed")?;
        ensure!(
            body.len().saturating_add(chunk.len()) <= limit,
            "Nacos response exceeds {limit} bytes"
        );
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

#[derive(Clone)]
pub struct DubboRuntime {
    jar: Arc<[u8]>,
}

impl DubboRuntime {
    pub fn new(jar: &Path) -> Result<Self> {
        ensure!(
            jar.is_absolute() && jar.is_file(),
            "Dubbo worker JAR must be an explicit absolute file"
        );
        let bytes = std::fs::read(jar).context("Cannot read Dubbo worker JAR")?;
        ensure!(
            bytes.len() <= 64 * 1024 * 1024,
            "Dubbo worker JAR exceeds 64 MiB"
        );
        ensure!(
            format!("{:x}", Sha256::digest(&bytes)) == WORKER_JAR_SHA256,
            "Dubbo worker JAR digest mismatch"
        );
        Ok(Self { jar: bytes.into() })
    }

    pub async fn invoke(
        &self,
        java: &Path,
        request: &RequestSpec,
        policy: NetworkPolicy,
    ) -> Result<Response> {
        ensure!(
            java.is_absolute() && java.is_file(),
            "Dubbo Java executable must be an explicit absolute file"
        );
        let java = java.canonicalize()?;
        moleapi_core::validate_request(request, false)?;
        let Protocol::Dubbo { config } = &request.protocol else {
            bail!("Expected classic Dubbo request");
        };
        let start = Instant::now();
        let deadline = start + Duration::from_millis(request.timeout_ms);
        let target = if config.source != "direct" {
            let providers = match config.source.as_str() {
                "zookeeper" => {
                    lookup_zookeeper(
                        &request.url,
                        config,
                        policy,
                        deadline.saturating_duration_since(Instant::now()),
                    )
                    .await?
                }
                "nacos" => {
                    lookup_nacos(
                        &request.url,
                        config,
                        policy,
                        deadline.saturating_duration_since(Instant::now()),
                    )
                    .await?
                }
                _ => bail!("Unsupported Dubbo registry source"),
            };
            select_registry_provider(
                config,
                &providers,
                policy,
                deadline.saturating_duration_since(Instant::now()),
            )
            .await?
            .destination()
        } else {
            let original = dubbo_url(&request.url)?;
            let addresses = tokio::time::timeout(
                deadline.saturating_duration_since(Instant::now()),
                checked_destination(&original, policy),
            )
            .await
            .context("Dubbo address lookup timed out")??;
            reachable_address(&addresses, deadline).await?
        };
        let endpoint = pinned_url(target, &config.service);
        let attachments = config
            .attachments
            .iter()
            .filter(|row| row.enabled)
            .map(|row| {
                (
                    row.key.clone(),
                    Value::String(row.local_value.as_deref().unwrap_or(&row.value).to_owned()),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        let input = json!({
            "format":"moleapi-dubbo-call-v1", "endpoint":endpoint,
            "service":config.service, "method":config.method,
            "version":config.version, "group":config.group,
            "timeout_ms":request.timeout_ms,
            "parameter_types":config.arguments.iter().map(|arg| &arg.type_name).collect::<Vec<_>>(),
            "argument_sources":config.arguments.iter().map(|arg| &arg.value_source).collect::<Vec<_>>(),
            "attachments":attachments
        });
        let bytes = serde_json::to_vec(&input)?;
        ensure!(bytes.len() <= MAX_INPUT, "Dubbo worker input exceeds 1 MiB");
        let temporary = tempfile::tempdir()?;
        let jar = temporary.path().join("dubbo-worker.jar");
        tokio::time::timeout(
            deadline.saturating_duration_since(Instant::now()),
            tokio::fs::write(&jar, self.jar.as_ref()),
        )
        .await
        .context("Dubbo worker setup timed out")??;
        let mut child = Command::new(&java)
            .env_clear()
            .arg("-Xmx128m")
            .arg("-Xss1m")
            .arg("-Djava.awt.headless=true")
            .arg("-Djava.net.useSystemProxies=false")
            .arg("-Dfile.encoding=UTF-8")
            .arg(format!("-Duser.home={}", temporary.path().display()))
            .arg("-jar")
            .arg(&jar)
            .current_dir(temporary.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .context("Cannot start Dubbo worker")?;
        let limit = deadline.saturating_duration_since(Instant::now());
        let result = tokio::time::timeout(limit, exchange(&mut child, &bytes)).await;
        let output = match result {
            Ok(Ok(value)) => value,
            Ok(Err(error)) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                return Err(error);
            }
            Err(_) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                bail!("Dubbo worker timed out");
            }
        };
        let reply: Value = serde_json::from_slice(&output).context("Invalid Dubbo worker reply")?;
        let (status, status_text, body) = if reply["ok"] == true {
            ensure!(
                reply.get("value").is_some(),
                "Dubbo worker omitted result value"
            );
            (200, "Dubbo OK", serde_json::to_string(&reply["value"])?)
        } else if reply["kind"] == "remote_exception" {
            (
                500,
                "Dubbo remote exception",
                serde_json::to_string(&json!({
                    "class":reply["class"], "message":reply["message"]
                }))?,
            )
        } else {
            bail!("Dubbo worker rejected the invocation");
        };
        Ok(Response {
            skipped: false,
            private_auth_values: vec![],
            soap_fault: None,
            request_updates: vec![],
            logs: if config.source != "direct" {
                vec![ScriptLog {
                    level: "info".into(),
                    message: format!(
                        "{} → {target}",
                        if config.source == "zookeeper" {
                            "ZooKeeper"
                        } else {
                            "Nacos"
                        }
                    ),
                }]
            } else {
                vec![]
            },
            variable_updates: vec![],
            status,
            status_text: status_text.into(),
            headers: vec![],
            size_bytes: body.len(),
            body,
            body_base64: None,
            elapsed_ms: start.elapsed().as_millis() as u64,
            truncated: false,
            url: request.url.clone(),
            tests: vec![],
        })
    }
}

fn pinned_url(target: SocketAddr, service: &str) -> String {
    format!("dubbo://{target}/{service}")
}

async fn reachable_address(addresses: &[SocketAddr], deadline: Instant) -> Result<SocketAddr> {
    for address in addresses {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            bail!("Dubbo connection timed out");
        }
        let limit = remaining.min(Duration::from_secs(3));
        if let Ok(Ok(stream)) =
            tokio::time::timeout(limit, tokio::net::TcpStream::connect(address)).await
        {
            drop(stream);
            return Ok(*address);
        }
    }
    bail!("Dubbo destination could not be connected")
}

async fn bounded_read(stream: impl AsyncRead + Unpin, limit: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    stream
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .await?;
    ensure!(bytes.len() <= limit, "Dubbo worker output limit exceeded");
    Ok(bytes)
}

async fn exchange(child: &mut Child, input: &[u8]) -> Result<Vec<u8>> {
    let mut stdin = child.stdin.take().context("Dubbo worker stdin missing")?;
    stdin.write_all(input).await?;
    stdin.shutdown().await?;
    drop(stdin);
    let stdout = child.stdout.take().context("Dubbo worker stdout missing")?;
    let stderr = child.stderr.take().context("Dubbo worker stderr missing")?;
    let (output, _diagnostics, status) = tokio::try_join!(
        bounded_read(stdout, MAX_OUTPUT),
        bounded_read(stderr, MAX_STDERR),
        async { Ok::<_, anyhow::Error>(child.wait().await?) }
    )?;
    ensure!(status.success(), "Dubbo worker failed");
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_filter_rejects_ambiguous_controls_before_selection() {
        let config = DubboConfig {
            version: "1.0.0".into(),
            group: "qa".into(),
            ..Default::default()
        };
        let base = "dubbo://example.com:20880/fixture.Service?version=1.0.0&group=qa";
        assert!(registry_candidate_matches(base, &config).unwrap());
        assert!(
            !registry_candidate_matches(
                "dubbo://example.com:20880/fixture.Service?version=2.0.0&group=qa",
                &config
            )
            .unwrap()
        );
        assert!(registry_candidate_matches(&format!("{base}&group=other"), &config).is_err());
        assert!(registry_candidate_matches("not a URL", &config).is_err());
    }

    #[tokio::test]
    async fn nacos_rejects_non_dubbo_instances_and_oversized_responses() {
        let config = DubboConfig {
            source_version: 2,
            source: "nacos".into(),
            service: "fixture.Service".into(),
            version: "1.0.0".into(),
            group: "qa".into(),
            ..Default::default()
        };
        let policy = NetworkPolicy {
            allow_private_network: true,
        };
        let invalid = json!({"hosts":[{"healthy":true,"enabled":true,
            "ip":"127.0.0.1","port":20880,"metadata":{"path":"fixture.Service",
            "protocol":"tri","version":"1.0.0","group":"qa","side":"provider"}}]})
        .to_string();
        for (body, expected) in [
            (invalid, "not a matching classic Dubbo provider"),
            (
                "x".repeat(MAX_INPUT + 1),
                "Nacos response exceeds 1048576 bytes",
            ),
        ] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            let app = axum::Router::new().route(
                "/nacos/v1/ns/instance/list",
                axum::routing::get(move || {
                    let value = body.clone();
                    async move { value }
                }),
            );
            let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
            let result = lookup_nacos(
                &format!("nacos://127.0.0.1:{port}"),
                &config,
                policy,
                Duration::from_secs(3),
            )
            .await;
            assert!(result.unwrap_err().to_string().contains(expected));
            server.abort();
        }
    }

    #[test]
    fn only_pinned_worker_bytes_are_accepted() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), b"untrusted jar").unwrap();
        assert!(
            DubboRuntime::new(file.path())
                .err()
                .unwrap()
                .to_string()
                .contains("digest mismatch")
        );
        assert_eq!(
            pinned_url("127.0.0.1:20880".parse().unwrap(), "fixture.EchoService"),
            "dubbo://127.0.0.1:20880/fixture.EchoService"
        );
        assert_eq!(
            pinned_url("[::1]:20880".parse().unwrap(), "fixture.EchoService"),
            "dubbo://[::1]:20880/fixture.EchoService"
        );
    }

    #[tokio::test]
    async fn worker_pipe_enforces_output_limit_before_parsing() {
        let (mut sender, receiver) = tokio::io::duplex(128);
        let writer = tokio::spawn(async move {
            sender.write_all(&[b'x'; 9]).await.unwrap();
        });
        assert!(bounded_read(receiver, 8).await.is_err());
        writer.await.unwrap();
    }

    #[tokio::test]
    async fn selects_reachable_checked_address_after_ipv6_refusal() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let addresses = [
            format!("[::1]:{port}").parse().unwrap(),
            listener.local_addr().unwrap(),
        ];
        assert_eq!(
            reachable_address(&addresses, Instant::now() + Duration::from_secs(3))
                .await
                .unwrap(),
            addresses[1]
        );
    }

    #[tokio::test]
    async fn registry_snapshot_checks_every_provider_before_selecting_one() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let config = DubboConfig {
            service: "fixture.EchoService".into(),
            version: "1.0.0".into(),
            ..Default::default()
        };
        let reachable =
            format!("dubbo://{address}/fixture.EchoService?side=provider&version=1.0.0");
        let selected = select_registry_provider(
            &config,
            std::slice::from_ref(&reachable),
            NetworkPolicy {
                allow_private_network: true,
            },
            Duration::from_secs(2),
        )
        .await
        .unwrap();
        assert_eq!(selected.destination(), address);
        assert_eq!(selected.original_url(), reachable);
        let mismatched = [
            reachable.clone(),
            "dubbo://127.0.0.2:20880/fixture.EchoService?side=provider&version=2.0.0".into(),
        ];
        assert!(
            select_registry_provider(
                &config,
                &mismatched,
                NetworkPolicy {
                    allow_private_network: true,
                },
                Duration::from_secs(2),
            )
            .await
            .is_err(),
            "a reachable first provider cannot hide a malformed later entry"
        );
        let unsafe_snapshot = [
            reachable,
            "dubbo://127.0.0.2:20880/fixture.EchoService?side=provider&version=1.0.0".into(),
        ];
        assert!(
            select_registry_provider(
                &config,
                &unsafe_snapshot,
                NetworkPolicy {
                    allow_private_network: false,
                },
                Duration::from_secs(2),
            )
            .await
            .is_err()
        );
    }
}
