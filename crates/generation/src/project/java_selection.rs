//! Explicit native-user Java selection; never exposed as a hosted HTTP operation.
use super::*;
use anyhow::Context;
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{io::AsyncReadExt, process::Command};

const PREFERENCE_LIMIT: usize = 8192;
const PROBE_LIMIT: usize = 16384;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JavaPreference {
    version: u8,
    java: PathBuf,
}

fn check_path(path: &Path) -> Result<()> {
    ensure!(
        path.is_absolute() && path.is_file(),
        "Select an absolute Java executable file"
    );
    Ok(())
}

fn java_major(output: &[u8]) -> Result<u32> {
    let output = std::str::from_utf8(output).context("Invalid Java version output")?;
    let version = output
        .lines()
        .find_map(|line| {
            line.strip_prefix("openjdk version \"")
                .or_else(|| line.strip_prefix("java version \""))
        })
        .context("Executable did not report a Java version")?;
    let number: String = version.chars().take_while(char::is_ascii_digit).collect();
    let major: u32 = number.parse().context("Invalid Java version")?;
    ensure!(major >= 17, "Java17 or newer is required");
    Ok(major)
}

async fn read_version(stream: impl tokio::io::AsyncRead + Unpin) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    stream
        .take((PROBE_LIMIT + 1) as u64)
        .read_to_end(&mut bytes)
        .await?;
    ensure!(
        bytes.len() <= PROBE_LIMIT,
        "Java version output exceeded limit"
    );
    Ok(bytes)
}

async fn probe(path: &Path) -> Result<u32> {
    check_path(path)?;
    let mut command = Command::new(path);
    command
        .arg("-version")
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("SYSTEMROOT") {
        command.env("SYSTEMROOT", root);
    }
    let mut child = command
        .spawn()
        .context("Cannot start selected Java executable")?;
    let stdout = child.stdout.take().context("Java stdout unavailable")?;
    let stderr = child.stderr.take().context("Java stderr unavailable")?;
    let operation = async {
        let (mut stdout, stderr) = tokio::try_join!(read_version(stdout), read_version(stderr))?;
        ensure!(child.wait().await?.success(), "Java version probe failed");
        stdout.push(b'\n');
        stdout.extend(stderr);
        java_major(&stdout)
    };
    let result = tokio::time::timeout(Duration::from_secs(5), operation).await;
    match result {
        Ok(Ok(major)) => Ok(major),
        failure => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            match failure {
                Ok(Err(error)) => Err(error),
                _ => anyhow::bail!("Java version probe timed out"),
            }
        }
    }
}

impl ProjectRuntime {
    /// Restore only an explicitly persisted native-user choice; no PATH search.
    pub fn restore_native_java(&self, preference: &Path) -> Result<()> {
        let mut source = Vec::new();
        std::fs::File::open(preference)?
            .take((PREFERENCE_LIMIT + 1) as u64)
            .read_to_end(&mut source)?;
        ensure!(
            source.len() <= PREFERENCE_LIMIT,
            "Java preference exceeded limit"
        );
        let preference: JavaPreference = serde_json::from_slice(&source)?;
        ensure!(
            preference.version == 1,
            "Unsupported Java preference version"
        );
        check_path(&preference.java)?;
        self.replace_java(preference.java);
        Ok(())
    }

    /// Called only after the native file dialog explicitly selects an executable.
    /// The caller serializes preference changes; running jobs keep their path snapshot.
    pub async fn select_native_java(&self, path: PathBuf, preference: &Path) -> Result<u32> {
        ensure!(
            preference.is_absolute(),
            "Java preference path must be absolute"
        );
        let major = probe(&path).await?;
        let source = serde_json::to_vec(&JavaPreference {
            version: 1,
            java: path.clone(),
        })?;
        ensure!(
            source.len() <= PREFERENCE_LIMIT,
            "Java preference exceeded limit"
        );
        let directory = preference
            .parent()
            .context("Java preference directory missing")?;
        let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
        temporary.write_all(&source)?;
        temporary.as_file().sync_all()?;
        temporary.persist(preference).map_err(|error| error.error)?;
        self.replace_java(path);
        Ok(major)
    }
}

#[cfg(test)]
mod tests;
