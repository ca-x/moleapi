//! Private stdin/stdout worker protocol; the host kills native work past its deadline.
use crate::{ScriptFailure, ScriptOutput};
use moleapi_core::{RequestSpec, Response, VariableScopes};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    io::{Read, Write},
    path::Path,
    process::Stdio,
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};

pub const WORKER_ARGUMENT: &str = "--moleapi-script-worker";
pub const WORKER_DEADLINE: Duration = Duration::from_secs(1);
pub const MAX_WORKER_INPUT: usize = 24 * 1024 * 1024;
pub const MAX_WORKER_OUTPUT: usize = 32 * 1024 * 1024;
#[derive(Serialize, Deserialize)]
struct WorkerInput {
    #[serde(default)]
    condition: Option<String>,
    scripts: Vec<String>,
    request: RequestSpec,
    response: Option<Response>,
    scopes: VariableScopes,
    private_values: BTreeSet<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum WorkerOutput {
    Success { output: Box<ScriptOutput> },
    Failure { failure: ScriptFailure },
}
fn failure(message: &str, scopes: &VariableScopes, privacy_complete: bool) -> ScriptFailure {
    ScriptFailure {
        message: message.into(),
        private_values: scopes.private_values.clone(),
        privacy_complete,
    }
}

/// Call before CLI parsing or desktop initialization. Never opens a port or database.
pub fn dispatch_worker() -> anyhow::Result<bool> {
    if std::env::args_os().nth(1).as_deref() != Some(std::ffi::OsStr::new(WORKER_ARGUMENT)) {
        return Ok(false);
    }
    let mut bytes = Vec::new();
    std::io::stdin()
        .lock()
        .take((MAX_WORKER_INPUT + 1) as u64)
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(
        bytes.len() <= MAX_WORKER_INPUT,
        "Script worker input exceeds limit"
    );
    let mut input: WorkerInput = serde_json::from_slice(&bytes)?;
    input.scopes.private_values = input.private_values;
    let output = match crate::run_with_condition(
        &input.scripts,
        &input.request,
        input.response.as_ref(),
        &input.scopes,
        input.condition.as_deref(),
    ) {
        Ok(output) => WorkerOutput::Success {
            output: Box::new(output),
        },
        Err(error) => WorkerOutput::Failure {
            failure: error
                .downcast::<ScriptFailure>()
                .unwrap_or_else(|_| failure("Script worker failed", &input.scopes, false)),
        },
    };
    let bytes = serde_json::to_vec(&output)?;
    anyhow::ensure!(
        bytes.len() <= MAX_WORKER_OUTPUT,
        "Script worker output exceeds limit"
    );
    std::io::stdout().lock().write_all(&bytes)?;
    Ok(true)
}

/// The executable must be an explicit trusted application/worker path, never PATH lookup.
pub async fn run_worker(
    executable: &Path,
    scripts: Vec<String>,
    request: &RequestSpec,
    response: Option<&Response>,
    scopes: &VariableScopes,
) -> Result<ScriptOutput, ScriptFailure> {
    run_worker_input(executable, scripts, request, response, scopes, None).await
}
pub async fn condition_worker(
    executable: &Path,
    expression: &str,
    request: &RequestSpec,
    response: Option<&Response>,
    scopes: &VariableScopes,
) -> Result<ScriptOutput, ScriptFailure> {
    run_worker_input(
        executable,
        vec![],
        request,
        response,
        scopes,
        Some(expression.into()),
    )
    .await
}
async fn run_worker_input(
    executable: &Path,
    scripts: Vec<String>,
    request: &RequestSpec,
    response: Option<&Response>,
    scopes: &VariableScopes,
    condition: Option<String>,
) -> Result<ScriptOutput, ScriptFailure> {
    let deadline = tokio::time::Instant::now() + WORKER_DEADLINE;
    if !executable.is_absolute() {
        return Err(failure(
            "Script worker executable must be an absolute path",
            scopes,
            true,
        ));
    }
    let payload = serde_json::to_vec(&WorkerInput {
        condition,
        scripts,
        request: request.clone(),
        response: response.cloned(),
        scopes: scopes.clone(),
        private_values: scopes.private_values.clone(),
    })
    .map_err(|_| failure("Cannot encode script worker input", scopes, true))?;
    if payload.len() > MAX_WORKER_INPUT {
        return Err(failure("Script worker input exceeds limit", scopes, true));
    }
    if tokio::time::Instant::now() >= deadline {
        return Err(failure(
            "Script worker exceeded 1 second deadline",
            scopes,
            true,
        ));
    }
    let mut child = Command::new(executable)
        .arg(WORKER_ARGUMENT)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| failure("Cannot start script worker", scopes, true))?;
    if tokio::time::Instant::now() >= deadline {
        let _ = child.kill().await;
        return Err(failure(
            "Script worker exceeded 1 second deadline",
            scopes,
            false,
        ));
    }
    let result = tokio::time::timeout_at(deadline, async {
        let mut stdin = child
            .stdin
            .take()
            .ok_or("Script worker input unavailable")?;
        stdin
            .write_all(&payload)
            .await
            .map_err(|_| "Cannot write script worker input")?;
        stdin
            .shutdown()
            .await
            .map_err(|_| "Cannot close script worker input")?;
        drop(stdin);
        let stdout = child
            .stdout
            .take()
            .ok_or("Script worker output unavailable")?;
        let mut bytes = Vec::new();
        stdout
            .take((MAX_WORKER_OUTPUT + 1) as u64)
            .read_to_end(&mut bytes)
            .await
            .map_err(|_| "Cannot read script worker output")?;
        if bytes.len() > MAX_WORKER_OUTPUT {
            return Err("Script worker output exceeds limit");
        }
        let status = child
            .wait()
            .await
            .map_err(|_| "Cannot reap script worker")?;
        if !status.success() {
            return Err("Script worker exited unsuccessfully");
        }
        let output: WorkerOutput =
            serde_json::from_slice(&bytes).map_err(|_| "Invalid script worker output")?;
        Ok(output)
    })
    .await;
    match result {
        Ok(Ok(WorkerOutput::Success { output })) => Ok(*output),
        Ok(Ok(WorkerOutput::Failure { failure })) => Err(failure),
        Ok(Err(message)) => {
            let _ = child.kill().await;
            Err(failure(message, scopes, false))
        }
        Err(_) => {
            let _ = child.kill().await;
            Err(failure(
                "Script worker exceeded 1 second deadline",
                scopes,
                false,
            ))
        }
    }
}
