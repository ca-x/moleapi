//! One-shot Apache Dubbo GenericService worker with Rust-owned network admission.
use anyhow::{Context, Result, bail, ensure};
use moleapi_core::{
    NetworkPolicy, Protocol, RequestSpec, Response, checked_destination, dubbo_url,
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
    "fc91d31d99aff504cd33baabb96c454e1562b5bad5a29ed2a14d6b0a293f4709";
const MAX_INPUT: usize = 1024 * 1024;
const MAX_OUTPUT: usize = 1024 * 1024;
const MAX_STDERR: usize = 16 * 1024;

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
}
