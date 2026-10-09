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
    #[serde(default)]
    extractions: Vec<crate::Extraction>,
}
#[derive(Serialize, Deserialize)]
struct Evaluation {
    tests: Vec<TestResult>,
    extractions: Vec<crate::ExtractionResult>,
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
    crate::validate_extractions(&work.extractions)?;
    let result = Evaluation {
        tests: crate::assertions::assertions(&work.checks, &work.response),
        extractions: crate::extractions::evaluate_extractions(&work.extractions, &work.response),
    };
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
    let result = run(worker, checks, &[], response)
        .await
        .map(|result| result.tests);
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
pub async fn extraction_worker(
    worker: &Path,
    rules: &[crate::Extraction],
    response: &Response,
) -> Vec<crate::ExtractionResult> {
    run(worker, &[], rules, response)
        .await
        .map(|result| result.extractions)
        .unwrap_or_else(|_| {
            rules
                .iter()
                .filter(|rule| rule.enabled)
                .map(|rule| crate::ExtractionResult {
                    id: rule.id.clone(),
                    name: rule.name.clone(),
                    required: rule.required,
                    update: None,
                    error: Some("extraction worker failed or exceeded resource limits".into()),
                })
                .collect()
        })
}
async fn run(
    worker: &Path,
    checks: &[Assertion],
    rules: &[crate::Extraction],
    response: &Response,
) -> Result<Evaluation> {
    ensure!(
        worker.is_absolute() && worker.is_file(),
        "Assertion worker requires an explicit application executable"
    );
    ensure!(checks.len() <= 100, "Assertion count limit exceeded");
    let mut projected = response.clone();
    projected.body_base64 = response.body_base64.as_ref().map(|_| String::new());
    if rules.is_empty()
        && checks
            .iter()
            .all(|check| matches!(check.kind.as_str(), "status" | "duration" | "header"))
    {
        projected.body.clear();
    }
    let bytes = serde_json::to_vec(&Work {
        checks: checks.to_vec(),
        response: projected,
        extractions: rules.to_vec(),
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
    let job =
        async {
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
            let result: Evaluation = serde_json::from_slice(&output)?;
            let active = rules.iter().filter(|rule| rule.enabled).collect::<Vec<_>>();
            ensure!(
                result.extractions.len() == active.len()
                    && result
                        .extractions
                        .iter()
                        .zip(active)
                        .all(|(result, rule)| result.id == rule.id
                            && result.required == rule.required
                            && result.update.as_ref().is_none_or(|update| update.scope
                                == rule.scope
                                && update.key == rule.key
                                && update
                                    .value
                                    .as_ref()
                                    .is_some_and(|value| value.len() <= 64 * 1024))),
                "Invalid extraction worker reply"
            );
            ensure!(
                result.tests.len() == checks.len()
                    && result
                        .tests
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
