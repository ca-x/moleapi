//! Shared application worker for untrusted response assertion evaluation.
use crate::{Assertion, Response, TestResult};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsStr,
    io::{Read, Write},
    path::Path,
    process::Stdio,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};
const ARG: &str = "--moleapi-assertion-worker";
const INPUT: usize = 16 * 1024 * 1024;
const OUTPUT: usize = 4 * 1024 * 1024;
#[derive(Serialize, Deserialize)]
struct Work {
    checks: Vec<Assertion>,
    response: Response,
}
pub fn dispatch_assertion_worker(limit: impl FnOnce() -> Result<()>) -> Result<bool> {
    if std::env::args_os().nth(1).as_deref() != Some(OsStr::new(ARG)) {
        return Ok(false);
    }
    limit()?;
    let mut input = Vec::new();
    std::io::stdin()
        .lock()
        .take((INPUT + 1) as u64)
        .read_to_end(&mut input)?;
    ensure!(
        input.len() <= INPUT,
        "Assertion worker input limit exceeded"
    );
    let work: Work = serde_json::from_slice(&input)?;
    ensure!(work.checks.len() <= 100, "Assertion count limit exceeded");
    let result = crate::assertions::assertions(&work.checks, &work.response);
    let output = serde_json::to_vec(&result)?;
    ensure!(
        output.len() <= OUTPUT,
        "Assertion worker output limit exceeded"
    );
    std::io::stdout().lock().write_all(&output)?;
    Ok(true)
}
pub async fn assertion_worker(
    worker: &Path,
    checks: &[Assertion],
    response: &Response,
) -> Vec<TestResult> {
    let result = run(worker, checks, response).await;
    result.unwrap_or_else(|_| {
        checks
            .iter()
            .map(|check| TestResult {
                id: check.id.clone(),
                name: check.name.clone(),
                passed: false,
                actual: "assertion worker failed, timed out or exceeded resource limits".into(),
                expected: if check.kind == "schema" {
                    "valid JSON Schema".into()
                } else {
                    if check.expected.len() <= 4096 {
                        check.expected.clone()
                    } else {
                        "[expected detail omitted: size limit]".into()
                    }
                },
            })
            .collect()
    })
}
async fn run(worker: &Path, checks: &[Assertion], response: &Response) -> Result<Vec<TestResult>> {
    ensure!(
        worker.is_absolute() && worker.is_file(),
        "Assertion worker requires an explicit application executable"
    );
    ensure!(checks.len() <= 100, "Assertion count limit exceeded");
    let mut projected = response.clone();
    projected.body_base64 = response.body_base64.as_ref().map(|_| String::new());
    if checks
        .iter()
        .all(|check| matches!(check.kind.as_str(), "status" | "duration" | "header"))
    {
        projected.body.clear();
    }
    let bytes = serde_json::to_vec(&Work {
        checks: checks.to_vec(),
        response: projected,
    })?;
    ensure!(
        bytes.len() <= INPUT,
        "Assertion worker input limit exceeded"
    );
    let mut child = Command::new(worker)
        .arg(ARG)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let job = async {
        let mut stdin = child
            .stdin
            .take()
            .context("Assertion worker stdin missing")?;
        stdin.write_all(&bytes).await?;
        stdin.shutdown().await?;
        drop(stdin);
        let mut output = Vec::new();
        child
            .stdout
            .take()
            .context("Assertion worker stdout missing")?
            .take((OUTPUT + 1) as u64)
            .read_to_end(&mut output)
            .await?;
        ensure!(
            output.len() <= OUTPUT && child.wait().await?.success(),
            "Assertion worker failed"
        );
        let result: Vec<TestResult> = serde_json::from_slice(&output)?;
        ensure!(
            result.len() == checks.len()
                && result
                    .iter()
                    .zip(checks)
                    .all(|(test, check)| test.id == check.id),
            "Invalid assertion worker reply"
        );
        Ok(result)
    };
    let result = tokio::time::timeout(std::time::Duration::from_secs(2), job)
        .await
        .unwrap_or_else(|_| Err(anyhow::anyhow!("Assertion worker timed out")));
    if result.is_err() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
    result
}
