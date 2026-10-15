//! One-shot Apache Dubbo GenericService worker with Rust-owned network admission.
use anyhow::{Context, Result, bail, ensure};
use moleapi_core::{
    DubboConfig, NetworkPolicy, Protocol, RequestSpec, Response, checked_destination,
    dubbo_provider_url, dubbo_url,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    net::SocketAddr,
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
        let original = dubbo_url(&request.url)?;
        let addresses = tokio::time::timeout(
            deadline.saturating_duration_since(Instant::now()),
            checked_destination(&original, policy),
        )
        .await
        .context("Dubbo address lookup timed out")??;
        let target = reachable_address(&addresses, deadline).await?;
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
            logs: vec![],
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
