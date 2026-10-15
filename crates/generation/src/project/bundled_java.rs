//! Known application resource locations, never PATH or specification-controlled paths.
use super::*;
use anyhow::Context;
use std::{
    io::Read,
    path::{Path, PathBuf},
};

const MANIFEST: &str = include_str!("../../../../vendor/java-runtime/manifest.json");
const RECEIPT_LIMIT: u64 = 8192;

fn target() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") if cfg!(target_env = "gnu") => Some("x86_64-unknown-linux-gnu"),
        ("linux", "aarch64") if cfg!(target_env = "gnu") => Some("aarch64-unknown-linux-gnu"),
        ("macos", "x86_64") => Some("x86_64-apple-darwin"),
        ("macos", "aarch64") => Some("aarch64-apple-darwin"),
        ("windows", "x86_64") => Some("x86_64-pc-windows-msvc"),
        _ => None,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    format: String,
    version: String,
    target: String,
    archive_sha256: String,
}

fn resolve(root: &Path, target: &str) -> Result<Option<PathBuf>> {
    ensure!(
        root.is_absolute(),
        "Bundled Java resource directory must be absolute"
    );
    let directory = root.join("runtime/java");
    let receipt_path = directory.join("moleapi-java-runtime.json");
    if !receipt_path.try_exists()? {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    std::fs::File::open(receipt_path)?
        .take(RECEIPT_LIMIT + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= RECEIPT_LIMIT,
        "Bundled Java receipt exceeds limit"
    );
    let receipt: Receipt =
        serde_json::from_slice(&bytes).context("Invalid bundled Java receipt")?;
    let manifest: Value = serde_json::from_str(MANIFEST)?;
    let asset = &manifest["assets"][target];
    ensure!(
        receipt.format == "moleapi-java-runtime-v1"
            && receipt.target == target
            && receipt.version == manifest["version"].as_str().unwrap_or_default()
            && receipt.archive_sha256 == asset["sha256"].as_str().unwrap_or_default(),
        "Bundled Java platform/version/checksum receipt mismatch"
    );
    let executable = directory.join(
        asset["java"]
            .as_str()
            .context("Unsupported bundled Java target")?,
    );
    ensure!(executable.is_file(), "Bundled Java executable is missing");
    let bundle = directory.canonicalize()?;
    let payload = directory.join("payload").canonicalize()?;
    ensure!(
        bundle.starts_with(root.canonicalize()?)
            && payload.starts_with(&bundle)
            && executable.canonicalize()?.starts_with(payload),
        "Bundled Java executable escapes application resources"
    );
    Ok(Some(executable))
}

pub(super) fn bundled_java(root: &Path) -> Result<Option<PathBuf>> {
    target().map_or(Ok(None), |target| resolve(root, target))
}

impl ProjectRuntime {
    /// Tauri passes its own trusted resource directory; explicit choices win.
    pub fn with_bundled_java(self, resource_root: &Path) -> Result<Self> {
        if self.java_path().is_none()
            && let Some(path) = bundled_java(resource_root)?
        {
            self.replace_java(path);
        }
        Ok(self)
    }
}

#[cfg(test)]
mod tests;
