//! Three-way regeneration uses diffy; authored files are never overwritten by this service.
use super::*;
use anyhow::ensure;
use base64::{Engine, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsStr,
    io::{Read, Write},
};
pub const DIFF_WORKER_ARG: &str = "--moleapi-project-diff-worker";
const TEXT_LIMIT: usize = 256 * 1024;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkingFile {
    pub path: String,
    pub encoding: String,
    pub content: String,
    #[serde(default)]
    pub executable: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegenerationInput {
    pub previous: Vec<ProjectFile>,
    pub working: Vec<WorkingFile>,
    pub next: Vec<ProjectFile>,
    #[serde(default)]
    pub resolutions: BTreeMap<String, String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegeneratedFile {
    pub path: String,
    pub status: String,
    pub encoding: String,
    pub content: Option<String>,
    pub executable: bool,
    pub patch: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegenerationResult {
    pub files: Vec<RegeneratedFile>,
    pub conflicts: usize,
    pub archive_base64: Option<String>,
}
fn decode(encoding: &str, content: &str) -> Result<Vec<u8>> {
    match encoding {
        "utf8" => Ok(content.as_bytes().to_vec()),
        "base64" => Ok(STANDARD.decode(content)?),
        _ => anyhow::bail!("Unknown project file encoding"),
    }
}
fn generated(
    files: Vec<ProjectFile>,
    total: &mut usize,
) -> Result<BTreeMap<String, (Vec<u8>, bool)>> {
    ensure!(
        files.len() <= 2048,
        "Regeneration file count limit exceeded"
    );
    let mut result = BTreeMap::new();
    for file in files {
        artifact::safe_path(&file.path)?;
        let bytes = decode(&file.encoding, &file.content)?;
        ensure!(
            bytes.len() == file.bytes && format!("{:x}", Sha256::digest(&bytes)) == file.sha256,
            "Generated input hash/size mismatch"
        );
        *total = total.saturating_add(bytes.len());
        ensure!(
            bytes.len() <= FILE_LIMIT && *total <= PROJECT_LIMIT,
            "Regeneration input size limit exceeded"
        );
        ensure!(
            result.insert(file.path, (bytes, file.executable)).is_none(),
            "Duplicate generated path"
        );
    }
    Ok(result)
}
fn text(bytes: &[u8]) -> Option<&str> {
    if bytes.len() > TEXT_LIMIT {
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?;
    if text.lines().count() > 10000 {
        return None;
    }
    Some(text)
}
pub fn regenerate(input: RegenerationInput) -> Result<RegenerationResult> {
    let resolutions = input.resolutions;
    ensure!(resolutions.len() <= 2048, "Resolution count limit exceeded");
    let mut total = 0;
    let previous = generated(input.previous, &mut total)?;
    let next = generated(input.next, &mut total)?;
    ensure!(
        input.working.len() <= 2048,
        "Working file count limit exceeded"
    );
    let mut working = BTreeMap::new();
    for file in input.working {
        artifact::safe_path(&file.path)?;
        let bytes = decode(&file.encoding, &file.content)?;
        total = total.saturating_add(bytes.len());
        ensure!(
            bytes.len() <= FILE_LIMIT && total <= PROJECT_LIMIT,
            "Working input size limit exceeded"
        );
        ensure!(
            working
                .insert(file.path, (bytes, file.executable))
                .is_none(),
            "Duplicate working path"
        );
    }
    let paths: BTreeSet<_> = previous
        .keys()
        .chain(next.keys())
        .chain(working.keys())
        .cloned()
        .collect();
    ensure!(
        paths.len() <= 2048,
        "Regeneration combined path count limit exceeded"
    );
    ensure!(
        resolutions.keys().all(|path| paths.contains(path)),
        "Resolution path is not part of the comparison"
    );
    let mut files = Vec::new();
    let mut conflicts = 0;
    let mut merged = BTreeMap::new();
    for path in paths {
        let base = previous.get(&path);
        let local = working.get(&path);
        let generated = next.get(&path);
        let (status, value, conflict) = if let Some(choice) = resolutions.get(&path) {
            match choice.as_str() {
                "working" => ("resolved", local.cloned(), false),
                "generated" => ("resolved", generated.cloned(), false),
                "delete" => ("resolved", None, false),
                _ => anyhow::bail!("Unknown conflict resolution"),
            }
        } else if local == generated {
            ("unchanged", local.cloned(), false)
        } else if local == base {
            (
                if base.is_none() { "added" } else { "updated" },
                generated.cloned(),
                false,
            )
        } else if generated == base {
            (
                if base.is_none() {
                    "authored"
                } else {
                    "preserved"
                },
                local.cloned(),
                false,
            )
        } else if base.is_none() && generated.is_none() {
            ("authored", local.cloned(), false)
        } else if base.is_none() && local.is_none() {
            ("added", generated.cloned(), false)
        } else if let (Some((base, basemode)), Some((local, mode)), Some((generated, newmode))) =
            (base, local, generated)
        {
            if let (Some(base), Some(local), Some(generated)) =
                (text(base), text(local), text(generated))
            {
                match diffy::merge(base, local, generated) {
                    Ok(text) => (
                        "merged",
                        Some((
                            text.into_bytes(),
                            if mode == basemode { *newmode } else { *mode },
                        )),
                        false,
                    ),
                    Err(markers) => ("conflict", Some((markers.into_bytes(), *mode)), true),
                }
            } else {
                ("conflict", Some((local.clone(), *mode)), true)
            }
        } else {
            ("conflict", local.cloned(), true)
        };
        let patch = match (
            local.as_ref().and_then(|(b, _)| text(b)),
            value.as_ref().and_then(|(b, _)| text(b)),
        ) {
            (Some(old), Some(new)) if old != new => Some(diffy::create_patch(old, new).to_string()),
            _ => None,
        };
        let (encoding, content, executable) = if let Some((bytes, mode)) = &value {
            let (encoding, content) = match String::from_utf8(bytes.clone()) {
                Ok(text) => ("utf8", text),
                Err(_) => ("base64", STANDARD.encode(bytes)),
            };
            (encoding.into(), Some(content), *mode)
        } else {
            ("utf8".into(), None, false)
        };
        if conflict {
            conflicts += 1;
        } else if let Some((bytes, mode)) = value {
            merged.insert(
                path.clone(),
                artifact::EmittedFile {
                    bytes,
                    executable: mode,
                },
            );
        }
        files.push(RegeneratedFile {
            path,
            status: if conflict {
                "conflict".into()
            } else if value_is_deleted(&content) {
                "deleted".into()
            } else {
                status.into()
            },
            encoding,
            content,
            executable,
            patch,
        });
    }
    let archive = if conflicts == 0 {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let mut size = 0;
        ensure!(
            !merged.contains_key("moleapi-regeneration.json"),
            "Regeneration metadata path is reserved"
        );
        let manifest = serde_json::to_vec_pretty(
            &serde_json::json!({"format":"moleapi-regeneration-v1","files":merged.iter().map(|(path,file)|serde_json::json!({"path":path,"sha256":format!("{:x}",Sha256::digest(&file.bytes)),"bytes":file.bytes.len(),"executable":file.executable})).collect::<Vec<_>>()}),
        )?;
        ensure!(
            manifest.len() <= 512 * 1024,
            "Regeneration manifest size exceeded"
        );
        writer.start_file(
            "moleapi-regeneration.json",
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .unix_permissions(0o644),
        )?;
        writer.write_all(&manifest)?;
        for (path, file) in merged {
            size += file.bytes.len();
            ensure!(size <= PROJECT_LIMIT, "Merged output limit exceeded");
            writer.start_file(
                path,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated)
                    .unix_permissions(if file.executable { 0o755 } else { 0o644 }),
            )?;
            writer.write_all(&file.bytes)?;
        }
        Some(STANDARD.encode(writer.finish()?.into_inner()))
    } else {
        None
    };
    let result = RegenerationResult {
        files,
        conflicts,
        archive_base64: archive,
    };
    ensure!(
        serde_json::to_vec(&result)?.len() <= IPC_LIMIT,
        "Regeneration reply limit exceeded"
    );
    Ok(result)
}
fn value_is_deleted(content: &Option<String>) -> bool {
    content.is_none()
}
pub(crate) fn dispatch_diff_worker(limit: impl FnOnce() -> Result<()>) -> Result<bool> {
    if std::env::args_os().nth(1).as_deref() != Some(OsStr::new(DIFF_WORKER_ARG)) {
        return Ok(false);
    }
    limit()?;
    let mut bytes = Vec::new();
    std::io::stdin()
        .lock()
        .take((IPC_LIMIT + 1) as u64)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= IPC_LIMIT,
        "Regeneration worker input limit exceeded"
    );
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum WorkerInput {
        Regenerate(RegenerationInput),
        Import(ProjectImportInput),
    }
    let input: WorkerInput = serde_json::from_slice(&bytes)?;
    let result = match input {
        WorkerInput::Regenerate(input) => {
            regenerate(input).and_then(|result| Ok(serde_json::to_value(result)?))
        }
        WorkerInput::Import(input) => {
            import_project(input).and_then(|result| Ok(serde_json::to_value(result)?))
        }
    };
    let reply = match result {
        Ok(result) => serde_json::json!({"result":result}),
        Err(_) => {
            serde_json::json!({"error":"Project import/regeneration failed; check files, hashes and resource limits"})
        }
    };
    let output = serde_json::to_vec(&reply)?;
    ensure!(
        output.len() <= IPC_LIMIT,
        "Regeneration worker output limit exceeded"
    );
    std::io::stdout().lock().write_all(&output)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn file(path: &str, text: &str) -> ProjectFile {
        ProjectFile {
            path: path.into(),
            encoding: "utf8".into(),
            content: text.into(),
            sha256: format!("{:x}", Sha256::digest(text.as_bytes())),
            bytes: text.len(),
            executable: false,
        }
    }
    fn working(path: &str, text: &str) -> WorkingFile {
        WorkingFile {
            path: path.into(),
            encoding: "utf8".into(),
            content: text.into(),
            executable: false,
        }
    }
    #[test]
    fn nonoverlapping_user_and_generator_edits_merge_and_authored_files_survive() {
        let result = regenerate(RegenerationInput {
            previous: vec![file("lib.txt", "first\nmiddle\nlast\n")],
            working: vec![
                working("lib.txt", "user-first\nmiddle\nlast\n"),
                working("authored.txt", "my code\n"),
            ],
            next: vec![file("lib.txt", "first\nmiddle\ngenerated-last\n")],
            resolutions: BTreeMap::new(),
        })
        .unwrap();
        assert_eq!(result.conflicts, 0);
        assert!(result.archive_base64.is_some());
        assert_eq!(
            result
                .files
                .iter()
                .find(|f| f.path == "lib.txt")
                .unwrap()
                .content
                .as_deref(),
            Some("user-first\nmiddle\ngenerated-last\n")
        );
        assert!(
            result
                .files
                .iter()
                .any(|f| f.path == "authored.txt" && f.content.as_deref() == Some("my code\n"))
        );
    }
    #[test]
    fn conflicting_and_delete_versus_modify_edits_disable_archive_and_keep_inputs() {
        let result = regenerate(RegenerationInput {
            previous: vec![file("a.txt", "old\n")],
            working: vec![working("a.txt", "mine\n")],
            next: vec![file("a.txt", "theirs\n")],
            resolutions: BTreeMap::new(),
        })
        .unwrap();
        assert_eq!(result.conflicts, 1);
        assert!(result.archive_base64.is_none());
        assert!(
            result.files[0]
                .content
                .as_ref()
                .unwrap()
                .contains("<<<<<<<")
        );
        let result = regenerate(RegenerationInput {
            previous: vec![file("a.txt", "old\n")],
            working: vec![],
            next: vec![file("a.txt", "new\n")],
            resolutions: BTreeMap::new(),
        })
        .unwrap();
        assert_eq!(result.files[0].status, "conflict");
        assert!(result.archive_base64.is_none());
    }
    #[test]
    fn explicit_resolution_produces_archive_and_preserves_user_mode() {
        let mut base = file("a.txt", "old\n");
        base.executable = true;
        let mut local = working("a.txt", "mine\n");
        local.executable = false;
        let mut generated = file("a.txt", "theirs\n");
        generated.executable = true;
        let result = regenerate(RegenerationInput {
            previous: vec![base],
            working: vec![local],
            next: vec![generated],
            resolutions: BTreeMap::from([("a.txt".into(), "working".into())]),
        })
        .unwrap();
        assert_eq!(result.conflicts, 0);
        assert_eq!(result.files[0].content.as_deref(), Some("mine\n"));
        assert!(!result.files[0].executable);
        assert!(result.archive_base64.is_some());
    }
    #[test]
    fn malformed_hashes_and_duplicate_or_escape_paths_are_rejected() {
        let mut f = file("a.txt", "old");
        f.content = "changed".into();
        assert!(
            regenerate(RegenerationInput {
                previous: vec![f],
                working: vec![],
                next: vec![],
                resolutions: BTreeMap::new(),
            })
            .is_err()
        );
        assert!(
            regenerate(RegenerationInput {
                previous: vec![],
                working: vec![working("../outside", "x")],
                next: vec![],
                resolutions: BTreeMap::new(),
            })
            .is_err()
        );
    }
}
