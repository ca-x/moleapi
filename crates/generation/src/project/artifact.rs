use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};
use std::{
    io::{Cursor, Write},
    path::Path,
};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectFile {
    pub executable: bool,
    pub path: String,
    pub encoding: String,
    pub content: String,
    pub sha256: String,
    pub bytes: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectArtifact {
    pub engine: String,
    pub target: String,
    pub source_sha256: String,
    pub options: BTreeMap<String, Value>,
    pub include_secrets: bool,
    pub files: Vec<ProjectFile>,
    pub archive_base64: String,
}
pub(crate) fn safe_path(path: &str) -> Result<()> {
    ensure!(
        !path.is_empty()
            && path.len() <= 1024
            && !path.contains(['\\', '\0', ':'])
            && path
                .split('/')
                .all(|p| !p.is_empty() && p != "." && p != "..")
            && !Path::new(path).is_absolute(),
        "Unsafe generated file path"
    );
    Ok(())
}
pub(crate) struct EmittedFile {
    pub bytes: Vec<u8>,
    pub executable: bool,
}
impl From<Vec<u8>> for EmittedFile {
    fn from(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            executable: false,
        }
    }
}
pub(crate) fn artifact(
    input: &ProjectInput,
    engine: &str,
    files: BTreeMap<String, EmittedFile>,
) -> Result<ProjectArtifact> {
    ensure!(
        !files.is_empty() && files.len() < 2048,
        "Generated file count exceeds limit"
    );
    let mut total = 0usize;
    let mut result = Vec::new();
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    ensure!(
        !files.contains_key("moleapi-generation.json"),
        "Generator attempted to overwrite artifact manifest"
    );
    for (path, file) in files {
        let bytes = file.bytes;
        let executable = file.executable;
        safe_path(&path)?;
        total = total
            .checked_add(bytes.len())
            .ok_or_else(|| anyhow::anyhow!("Generated project size overflow"))?;
        ensure!(
            bytes.len() <= FILE_LIMIT && total <= PROJECT_LIMIT,
            "Generated project exceeds content limits"
        );
        zip.start_file(
            &path,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .unix_permissions(if executable { 0o755 } else { 0o644 }),
        )?;
        zip.write_all(&bytes)?;
        let (encoding, content) = match String::from_utf8(bytes.clone()) {
            Ok(text) => ("utf8".into(), text),
            Err(_) => ("base64".into(), STANDARD.encode(&bytes)),
        };
        result.push(ProjectFile {
            executable,
            path,
            encoding,
            content,
            sha256: format!("{:x}", Sha256::digest(&bytes)),
            bytes: bytes.len(),
        });
    }
    let manifest = serde_json::to_vec_pretty(
        &serde_json::json!({"format":"moleapi-generation-v1","engine":engine,"target":input.target,"source_sha256":format!("{:x}",Sha256::digest(serde_json::to_vec(&input.specification)?)),"options":input.options,"include_secrets":input.include_secrets,"files":result.iter().map(|f|serde_json::json!({"path":f.path,"sha256":f.sha256,"bytes":f.bytes,"executable":f.executable})).collect::<Vec<_>>()}),
    )?;
    ensure!(
        manifest.len() <= 512 * 1024,
        "Generation manifest limit exceeded"
    );
    zip.start_file(
        "moleapi-generation.json",
        zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644),
    )?;
    zip.write_all(&manifest)?;
    result.push(ProjectFile {
        path: "moleapi-generation.json".into(),
        encoding: "utf8".into(),
        content: String::from_utf8(manifest.clone())?,
        sha256: format!("{:x}", Sha256::digest(&manifest)),
        bytes: manifest.len(),
        executable: false,
    });
    let archive = zip.finish()?.into_inner();
    let result = ProjectArtifact {
        engine: engine.into(),
        target: input.target.clone(),
        source_sha256: format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&input.specification)?)
        ),
        options: input.options.clone(),
        include_secrets: input.include_secrets,
        files: result,
        archive_base64: STANDARD.encode(archive),
    };
    ensure!(
        serde_json::to_vec(&result)?.len() <= IPC_LIMIT,
        "Generated artifact reply exceeds limit"
    );
    Ok(result)
}
pub(crate) fn collect(root: &Path) -> Result<BTreeMap<String, EmittedFile>> {
    let canonical = root.canonicalize()?;
    let mut pending = vec![canonical.clone()];
    let mut files = BTreeMap::new();
    let mut total = 0usize;
    let mut nodes = 0usize;
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            nodes += 1;
            ensure!(
                nodes <= 4096,
                "Generated directory/file count limit exceeded"
            );
            let meta = entry.path().symlink_metadata()?;
            ensure!(
                !meta.file_type().is_symlink(),
                "Generated symlinks are not allowed"
            );
            let path = entry.path().canonicalize()?;
            ensure!(
                path.starts_with(&canonical),
                "Generated path escaped output root"
            );
            if meta.is_dir() {
                pending.push(path);
                ensure!(pending.len() <= 2048, "Generated directory limit exceeded");
            } else {
                ensure!(
                    meta.is_file() && meta.len() <= FILE_LIMIT as u64,
                    "Unsupported generated file or size"
                );
                let bytes = std::fs::read(&path)?;
                total += bytes.len();
                ensure!(
                    total <= PROJECT_LIMIT && files.len() < 2048,
                    "Generated project size/count limit exceeded"
                );
                let relative = path
                    .strip_prefix(&canonical)?
                    .components()
                    .map(|p| {
                        p.as_os_str()
                            .to_str()
                            .ok_or_else(|| anyhow::anyhow!("Generated filename is not UTF8"))
                    })
                    .collect::<Result<Vec<_>>>()?
                    .join("/");
                safe_path(&relative)?;
                #[cfg(unix)]
                let executable = {
                    use std::os::unix::fs::PermissionsExt;
                    meta.permissions().mode() & 0o111 != 0
                };
                #[cfg(not(unix))]
                let executable = false;
                files.insert(relative, EmittedFile { bytes, executable });
            }
        }
    }
    Ok(files)
}

/// Check budgets while the trusted engine is still writing, not only afterward.
pub(crate) fn check_output_budget(root: &Path) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }
    let mut pending = vec![root.to_owned()];
    let mut nodes = 0usize;
    let mut bytes = 0u64;
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            nodes += 1;
            ensure!(
                nodes <= 4096,
                "Generated directory/file count limit exceeded"
            );
            let meta = match entry.path().symlink_metadata() {
                Ok(meta) => meta,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e.into()),
            };
            ensure!(
                !meta.file_type().is_symlink(),
                "Generated symlinks are not allowed"
            );
            if meta.is_dir() {
                pending.push(entry.path());
            } else {
                ensure!(
                    meta.is_file() && meta.len() <= FILE_LIMIT as u64,
                    "Generated file exceeded content limit"
                );
                bytes = bytes.saturating_add(meta.len());
                ensure!(
                    bytes <= PROJECT_LIMIT as u64,
                    "Generated output exceeded aggregate budget"
                );
            }
        }
    }
    Ok(())
}
