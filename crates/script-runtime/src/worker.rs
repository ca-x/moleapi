//! Private stdin/stdout worker protocol; the host kills native work past its deadline.
use crate::LiveLoader;
use crate::dataset_query::QuerySnapshot;
use crate::{ScriptFailure, ScriptOutput};
use moleapi_core::{RequestSpec, Response, ScriptMode, VariableScopes};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    io::{BufRead, Read, Write},
    path::Path,
    process::Stdio,
    rc::Rc,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::Command,
};

pub const WORKER_ARGUMENT: &str = "--moleapi-script-worker";
pub const WORKER_BRIDGE_ARGUMENT: &str = "--moleapi-script-worker-bridge";
pub const WORKER_DEADLINE: Duration = Duration::from_secs(1);
pub const MAX_WORKER_INPUT: usize = 24 * 1024 * 1024;
pub const MAX_WORKER_OUTPUT: usize = 32 * 1024 * 1024;
#[derive(Serialize, Deserialize)]
struct WorkerInput {
    #[serde(default)]
    mode: ScriptMode,
    #[serde(default)]
    datasets: Vec<QuerySnapshot>,
    #[serde(default)]
    condition: Option<String>,
    scripts: Vec<String>,
    request: RequestSpec,
    response: Option<Response>,
    scopes: VariableScopes,
    private_values: BTreeSet<String>,
}
struct WorkerOptions {
    condition: Option<String>,
    mode: ScriptMode,
    datasets: Vec<QuerySnapshot>,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum WorkerOutput {
    Success {
        output: Box<ScriptOutput>,
    },
    Failure {
        failure: ScriptFailure,
    },
    Load {
        dataset_id: String,
        scopes: Box<VariableScopes>,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum BridgeReply {
    Loaded {
        snapshot: QuerySnapshot,
        private_values: BTreeSet<String>,
    },
    Failed,
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
    let bridge =
        std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new(WORKER_BRIDGE_ARGUMENT));
    if !bridge
        && std::env::args_os().nth(1).as_deref() != Some(std::ffi::OsStr::new(WORKER_ARGUMENT))
    {
        return Ok(false);
    }
    let mut bytes = Vec::new();
    if bridge {
        std::io::stdin()
            .lock()
            .take((MAX_WORKER_INPUT + 1) as u64)
            .read_until(b'\n', &mut bytes)?;
        anyhow::ensure!(
            bytes.last() == Some(&b'\n'),
            "Script bridge input is incomplete"
        );
    } else {
        std::io::stdin()
            .lock()
            .take((MAX_WORKER_INPUT + 1) as u64)
            .read_to_end(&mut bytes)?;
    }
    anyhow::ensure!(
        bytes.len() <= MAX_WORKER_INPUT,
        "Script worker input exceeds limit"
    );
    let mut input: WorkerInput = serde_json::from_slice(&bytes)?;
    input.scopes.private_values = input.private_values;
    let loader: Option<LiveLoader> = bridge.then(|| {
        Rc::new(|id: &str, scopes: &VariableScopes| {
            let request = serde_json::to_vec(&WorkerOutput::Load {
                dataset_id: id.to_owned(),
                scopes: Box::new(scopes.clone()),
            })?;
            let mut stdout = std::io::stdout().lock();
            stdout.write_all(&request)?;
            stdout.write_all(b"\n")?;
            stdout.flush()?;
            drop(stdout);
            let mut reply = Vec::new();
            std::io::stdin()
                .lock()
                .take((MAX_WORKER_INPUT + 1) as u64)
                .read_until(b'\n', &mut reply)?;
            anyhow::ensure!(
                reply.len() <= MAX_WORKER_INPUT && reply.last() == Some(&b'\n'),
                "Live dataset bridge response is invalid"
            );
            match serde_json::from_slice::<BridgeReply>(&reply)? {
                BridgeReply::Loaded {
                    snapshot,
                    private_values,
                } => Ok((snapshot, private_values)),
                BridgeReply::Failed => anyhow::bail!("Live dataset refresh failed"),
            }
        }) as LiveLoader
    });
    let output = match crate::run_with_options(
        &input.scripts,
        &input.request,
        input.response.as_ref(),
        &input.scopes,
        crate::RuntimeOptions {
            condition: input.condition.as_deref(),
            mode: input.mode,
            datasets: &input.datasets,
            loader,
        },
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
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&bytes)?;
    if bridge {
        stdout.write_all(b"\n")?;
    }
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
    run_worker_with_mode(
        executable,
        scripts,
        request,
        response,
        scopes,
        ScriptMode::V1,
    )
    .await
}
pub async fn run_worker_with_mode(
    executable: &Path,
    scripts: Vec<String>,
    request: &RequestSpec,
    response: Option<&Response>,
    scopes: &VariableScopes,
    mode: ScriptMode,
) -> Result<ScriptOutput, ScriptFailure> {
    run_worker_input(
        executable,
        scripts,
        request,
        response,
        scopes,
        WorkerOptions {
            condition: None,
            mode,
            datasets: vec![],
        },
    )
    .await
}
pub async fn run_worker_with_datasets(
    executable: &Path,
    scripts: Vec<String>,
    request: &RequestSpec,
    response: Option<&Response>,
    scopes: &VariableScopes,
    datasets: Vec<QuerySnapshot>,
) -> Result<ScriptOutput, ScriptFailure> {
    run_worker_input(
        executable,
        scripts,
        request,
        response,
        scopes,
        WorkerOptions {
            condition: None,
            mode: ScriptMode::V2,
            datasets,
        },
    )
    .await
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
        WorkerOptions {
            condition: Some(expression.into()),
            mode: ScriptMode::V1,
            datasets: vec![],
        },
    )
    .await
}
async fn run_worker_input(
    executable: &Path,
    scripts: Vec<String>,
    request: &RequestSpec,
    response: Option<&Response>,
    scopes: &VariableScopes,
    options: WorkerOptions,
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
        mode: options.mode,
        datasets: options.datasets,
        condition: options.condition,
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
        Ok(Ok(WorkerOutput::Load { .. })) => Err(failure(
            "Unexpected script worker bridge request",
            scopes,
            false,
        )),
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

pub async fn run_worker_with_live<F, Fut>(
    executable: &Path,
    scripts: Vec<String>,
    request: &RequestSpec,
    response: Option<&Response>,
    scopes: &VariableScopes,
    datasets: Vec<QuerySnapshot>,
    mut resolve: F,
) -> Result<ScriptOutput, ScriptFailure>
where
    F: FnMut(String, VariableScopes, tokio::time::Instant) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<(QuerySnapshot, BTreeSet<String>)>>,
{
    let deadline = tokio::time::Instant::now() + WORKER_DEADLINE;
    if !executable.is_absolute() {
        return Err(failure(
            "Script worker executable must be an absolute path",
            scopes,
            true,
        ));
    }
    let live_ids = datasets
        .iter()
        .filter(|dataset| dataset.live_query)
        .map(|dataset| dataset.id.clone())
        .collect::<BTreeSet<_>>();
    let mut payload = serde_json::to_vec(&WorkerInput {
        mode: ScriptMode::V2,
        datasets,
        condition: None,
        scripts,
        request: request.clone(),
        response: response.cloned(),
        scopes: scopes.clone(),
        private_values: scopes.private_values.clone(),
    })
    .map_err(|_| failure("Cannot encode script worker input", scopes, true))?;
    if payload.len() >= MAX_WORKER_INPUT {
        return Err(failure("Script worker input exceeds limit", scopes, true));
    }
    payload.push(b'\n');
    let mut child = Command::new(executable)
        .arg(WORKER_BRIDGE_ARGUMENT)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| failure("Cannot start script worker", scopes, true))?;
    let mut private_values = scopes.private_values.clone();
    let result = tokio::time::timeout_at(deadline, async {
        let mut stdin = child
            .stdin
            .take()
            .ok_or("Script worker input unavailable")?;
        stdin
            .write_all(&payload)
            .await
            .map_err(|_| "Cannot write script worker input")?;
        let stdout = child
            .stdout
            .take()
            .ok_or("Script worker output unavailable")?;
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = Vec::new();
            reader
                .read_until(b'\n', &mut line)
                .await
                .map_err(|_| "Cannot read script worker output")?;
            if line.is_empty() || line.len() > MAX_WORKER_OUTPUT || line.last() != Some(&b'\n') {
                return Err("Invalid script worker bridge output");
            }
            match serde_json::from_slice::<WorkerOutput>(&line)
                .map_err(|_| "Invalid script worker bridge output")?
            {
                WorkerOutput::Load {
                    dataset_id,
                    scopes: live_scopes,
                } => {
                    let reply = if live_ids.contains(&dataset_id) && live_scopes.validate().is_ok()
                    {
                        private_values.extend(
                            live_scopes
                                .effective()
                                .variables
                                .into_iter()
                                .map(|pair| pair.value),
                        );
                        match resolve(dataset_id.clone(), *live_scopes, deadline).await {
                            Ok((snapshot, values))
                                if snapshot.id == dataset_id && !snapshot.live_query =>
                            {
                                private_values.extend(values.iter().cloned());
                                BridgeReply::Loaded {
                                    snapshot,
                                    private_values: values,
                                }
                            }
                            _ => BridgeReply::Failed,
                        }
                    } else {
                        BridgeReply::Failed
                    };
                    let mut bytes = serde_json::to_vec(&reply)
                        .map_err(|_| "Cannot encode live dataset response")?;
                    if bytes.len() >= MAX_WORKER_INPUT {
                        return Err("Live dataset response exceeds limit");
                    }
                    bytes.push(b'\n');
                    stdin
                        .write_all(&bytes)
                        .await
                        .map_err(|_| "Cannot write live dataset response")?;
                }
                output => {
                    stdin
                        .shutdown()
                        .await
                        .map_err(|_| "Cannot close script worker input")?;
                    let status = child
                        .wait()
                        .await
                        .map_err(|_| "Cannot reap script worker")?;
                    if !status.success() {
                        return Err("Script worker exited unsuccessfully");
                    }
                    return Ok(output);
                }
            }
        }
    })
    .await;
    match result {
        Ok(Ok(WorkerOutput::Success { mut output })) => {
            output.private_values.extend(private_values);
            Ok(*output)
        }
        Ok(Ok(WorkerOutput::Failure { mut failure })) => {
            failure.private_values.extend(private_values);
            Err(failure)
        }
        Ok(Ok(WorkerOutput::Load { .. })) => Err(failure(
            "Unexpected script worker bridge request",
            scopes,
            false,
        )),
        Ok(Err(message)) => {
            let _ = child.kill().await;
            Err(ScriptFailure {
                message: message.into(),
                private_values,
                privacy_complete: false,
            })
        }
        Err(_) => {
            let _ = child.kill().await;
            Err(ScriptFailure {
                message: "Script worker exceeded 1 second deadline".into(),
                private_values,
                privacy_complete: false,
            })
        }
    }
}
