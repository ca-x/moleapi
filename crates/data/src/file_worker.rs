//! File SQL runs in a bounded headless application child, not the hosted UI process.
use crate::{FileSource, QueryResult, SchemaTable};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use moleapi_core::DataConfig;
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::Path,
    process::Stdio,
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};
#[global_allocator]
static MEMORY: cap::Cap<std::alloc::System> = cap::Cap::new(std::alloc::System, usize::MAX);
const ARGUMENT: &str = "--moleapi-data-worker";
const MAX_INPUT: usize = 8 * 1024 * 1024;
const MAX_OUTPUT: usize = 10 * 1024 * 1024;
#[derive(Serialize, Deserialize)]
struct Input {
    config: DataConfig,
    bytes_base64: String,
    query: Option<String>,
}
#[derive(Serialize, Deserialize)]
pub struct FileOutput {
    pub schema: SchemaTable,
    pub result: Option<QueryResult>,
}
#[derive(Serialize, Deserialize)]
enum Reply {
    Success(FileOutput),
    Failure(String),
}
/// Invoke before CLI parsing/Tauri/Tokio initialization; only this sentinel enables heap caps.
pub fn dispatch_file_worker() -> Result<bool> {
    if std::env::args_os().nth(1).as_deref() != Some(std::ffi::OsStr::new(ARGUMENT)) {
        return Ok(false);
    }
    MEMORY
        .set_limit(512 * 1024 * 1024)
        .map_err(|_| anyhow::anyhow!("Cannot set file SQL worker heap limit"))?;
    let mut input = Vec::new();
    std::io::stdin()
        .lock()
        .take((MAX_INPUT + 1) as u64)
        .read_to_end(&mut input)?;
    ensure!(input.len() <= MAX_INPUT, "File worker input limit exceeded");
    let input: Input = serde_json::from_slice(&input)?;
    let bytes = STANDARD.decode(input.bytes_base64)?;
    ensure!(
        bytes.len() <= moleapi_core::MAX_DATA_FILE,
        "File worker file limit exceeded"
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let value = runtime.block_on(async {
        let source = FileSource::open(&input.config, &bytes)?;
        let result = match input.query {
            Some(sql) => Some(source.query(&sql).await?),
            None => None,
        };
        Ok::<_, anyhow::Error>(FileOutput {
            schema: source.schema(),
            result,
        })
    });
    let reply = match value {
        Ok(value) => Reply::Success(value),
        Err(error) => Reply::Failure(error.to_string()),
    };
    let output = serde_json::to_vec(&reply)?;
    ensure!(
        output.len() <= MAX_OUTPUT,
        "File worker output exceeds limit"
    );
    std::io::stdout().lock().write_all(&output)?;
    Ok(true)
}
pub async fn run_file_worker(
    executable: &Path,
    config: &DataConfig,
    bytes: &[u8],
    query: Option<&str>,
    timeout: Duration,
) -> Result<FileOutput> {
    ensure!(
        executable.is_absolute(),
        "File SQL worker requires an explicit application path"
    );
    ensure!(
        bytes.len() <= moleapi_core::MAX_DATA_FILE,
        "Data file exceeds 5 MiB"
    );
    moleapi_core::validate_data_config(config)?;
    let mut config = config.clone();
    config.file_base64.clear();
    let input = serde_json::to_vec(&Input {
        config,
        bytes_base64: STANDARD.encode(bytes),
        query: query.map(String::from),
    })?;
    ensure!(input.len() <= MAX_INPUT, "File worker input exceeds limit");
    let mut child = Command::new(executable)
        .arg(ARGUMENT)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let reply = tokio::time::timeout(timeout.min(Duration::from_secs(5)), async {
        let mut stdin = child.stdin.take().context("Missing file worker stdin")?;
        stdin.write_all(&input).await?;
        stdin.shutdown().await?;
        drop(stdin);
        let stdout = child.stdout.take().context("Missing file worker stdout")?;
        let mut bytes = Vec::new();
        stdout
            .take((MAX_OUTPUT + 1) as u64)
            .read_to_end(&mut bytes)
            .await?;
        ensure!(
            bytes.len() <= MAX_OUTPUT,
            "File SQL worker output limit exceeded"
        );
        let status = child.wait().await?;
        ensure!(
            status.success(),
            "File SQL worker exited; input/heap/resource limits may have been exceeded"
        );
        match serde_json::from_slice::<Reply>(&bytes)? {
            Reply::Success(value) => Ok(value),
            Reply::Failure(error) => Err(anyhow::anyhow!(error)),
        }
    })
    .await;
    match reply {
        Ok(value) => value,
        Err(_) => {
            let _ = child.kill().await;
            anyhow::bail!("File SQL worker timed out");
        }
    }
}
