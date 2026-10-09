//! Parse user-selected project bytes without extracting archives or reading host paths.
use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    io::{Cursor, Read},
};

pub const ARCHIVE_LIMIT: usize = 10 * 1024 * 1024;
const MANIFEST_LIMIT: usize = 512 * 1024;
const GENERATION_MANIFEST: &str = "moleapi-generation.json";
const REGENERATION_MANIFEST: &str = "moleapi-regeneration.json";

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportRole {
    Previous,
    Working,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProjectImportSource {
    Zip { archive_base64: String },
    Files { files: Vec<WorkingFile> },
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectImportInput {
    pub role: ImportRole,
    pub source: ProjectImportSource,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImportedProject {
    pub files: Vec<ProjectFile>,
}

fn import_file(path: String, bytes: Vec<u8>, executable: bool) -> ProjectFile {
    let size = bytes.len();
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    let (encoding, content) = match String::from_utf8(bytes) {
        Ok(text) => ("utf8".into(), text),
        Err(error) => ("base64".into(), STANDARD.encode(error.into_bytes())),
    };
    ProjectFile {
        path,
        encoding,
        content,
        bytes: size,
        sha256,
        executable,
    }
}

fn check_paths(paths: impl Iterator<Item = String>) -> Result<()> {
    let paths = paths.collect::<BTreeSet<_>>();
    for path in &paths {
        artifact::safe_path(path)?;
        for (index, _) in path.match_indices('/') {
            ensure!(
                !paths.contains(&path[..index]),
                "Project file/directory collision"
            );
        }
    }
    Ok(())
}

fn archive_files(encoded: &str) -> Result<Vec<ProjectFile>> {
    ensure!(
        encoded.len() <= ARCHIVE_LIMIT.div_ceil(3) * 4,
        "Project archive limit exceeded"
    );
    let bytes = STANDARD.decode(encoded)?;
    ensure!(
        bytes.len() <= ARCHIVE_LIMIT,
        "Project archive limit exceeded"
    );
    // zip's filename index hides duplicate central-directory entries. rawzip's
    // zero-copy iterator checks every entry before zip allocates or decompresses.
    let raw = rawzip::ZipArchive::from_slice(&bytes)?;
    ensure!(
        raw.entries_hint() <= 4096,
        "Project archive node count limit exceeded"
    );
    let mut entries = raw.entries();
    let mut expected_names = BTreeSet::new();
    let mut count = 0u64;
    while let Some(entry) = entries.next_entry()? {
        count += 1;
        ensure!(count <= 4096, "Project archive node count limit exceeded");
        let name = std::str::from_utf8(entry.file_path().as_bytes())?;
        let name = name.strip_suffix('/').unwrap_or(name);
        artifact::safe_path(name)?;
        ensure!(
            expected_names.insert(name.to_owned()),
            "Duplicate archive path"
        );
    }
    ensure!(
        count == raw.entries_hint(),
        "Project archive entry count mismatch"
    );
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    ensure!(
        archive.len() <= 4096,
        "Project archive node count limit exceeded"
    );
    let mut names = BTreeSet::new();
    let mut files = Vec::new();
    let mut total = 0usize;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        let directory = file.is_dir();
        let path = if directory {
            file.name().strip_suffix('/').unwrap_or(file.name())
        } else {
            file.name()
        }
        .to_owned();
        artifact::safe_path(&path)?;
        ensure!(names.insert(path.clone()), "Duplicate archive path");
        let mode = file.unix_mode().unwrap_or(0);
        let node = mode & 0o170000;
        ensure!(
            node == 0 || node == if directory { 0o040000 } else { 0o100000 },
            "Project links/special files are not allowed"
        );
        ensure!(
            matches!(
                file.compression(),
                zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated
            ),
            "Unsupported project archive compression"
        );
        if directory {
            continue;
        }
        ensure!(
            files.len() < 2048 && file.size() <= FILE_LIMIT as u64,
            "Project file count/size limit exceeded"
        );
        ensure!(
            (file.size() as usize) <= PROJECT_LIMIT + MANIFEST_LIMIT - total,
            "Expanded project size limit exceeded"
        );
        let mut bytes = Vec::new();
        (&mut file)
            .take((FILE_LIMIT + 1) as u64)
            .read_to_end(&mut bytes)?;
        total = total
            .checked_add(bytes.len())
            .ok_or_else(|| anyhow::anyhow!("Project size overflow"))?;
        ensure!(
            bytes.len() <= FILE_LIMIT && total <= PROJECT_LIMIT + MANIFEST_LIMIT,
            "Expanded project size limit exceeded"
        );
        files.push(import_file(path, bytes, mode & 0o111 != 0));
    }
    ensure!(
        names == expected_names,
        "Inconsistent project archive filenames"
    );
    check_paths(files.iter().map(|file| file.path.clone()))?;
    let regular = files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<BTreeSet<_>>();
    for name in &names {
        for (index, _) in name.match_indices('/') {
            ensure!(
                !regular.contains(&name[..index]),
                "Project file/directory collision"
            );
        }
    }
    // Do not guess a root from an arbitrary source tree: require a recognizable manifest.
    if !files
        .iter()
        .any(|file| [GENERATION_MANIFEST, REGENERATION_MANIFEST].contains(&file.path.as_str()))
    {
        let roots = files
            .iter()
            .filter_map(|file| file.path.split_once('/').map(|(root, _)| root))
            .collect::<BTreeSet<_>>();
        if roots.len() == 1 {
            let root = *roots.first().unwrap();
            let prefix = format!("{root}/");
            if files.iter().any(|file| {
                file.path == format!("{prefix}{GENERATION_MANIFEST}")
                    || file.path == format!("{prefix}{REGENERATION_MANIFEST}")
            }) {
                for file in &mut files {
                    file.path = file
                        .path
                        .strip_prefix(&prefix)
                        .ok_or_else(|| anyhow::anyhow!("Inconsistent project archive root"))?
                        .into();
                }
            }
        }
    }
    Ok(files)
}

#[derive(Deserialize)]
struct Manifest {
    format: String,
    files: Vec<ManifestFile>,
}
#[derive(Deserialize)]
struct ManifestFile {
    path: String,
    bytes: usize,
    sha256: String,
    executable: bool,
}

fn validate_previous(files: &[ProjectFile]) -> Result<()> {
    let metadata = files
        .iter()
        .find(|file| file.path == GENERATION_MANIFEST)
        .ok_or_else(|| anyhow::anyhow!("Previous ZIP requires its original generation manifest"))?;
    ensure!(
        metadata.encoding == "utf8" && metadata.bytes <= MANIFEST_LIMIT,
        "Invalid previous generation manifest"
    );
    let manifest: Manifest = serde_json::from_str(&metadata.content)?;
    ensure!(
        manifest.format == "moleapi-generation-v1" && manifest.files.len() + 1 == files.len(),
        "Previous generation file set mismatch"
    );
    let actual = files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    for file in manifest.files {
        artifact::safe_path(&file.path)?;
        ensure!(
            file.path != GENERATION_MANIFEST && seen.insert(file.path.clone()),
            "Duplicate/reserved generation manifest path"
        );
        let current = actual
            .get(file.path.as_str())
            .ok_or_else(|| anyhow::anyhow!("Previous generated file is missing"))?;
        ensure!(
            current.bytes == file.bytes
                && current.sha256 == file.sha256
                && current.executable == file.executable,
            "Previous generated file hash/size/mode mismatch"
        );
    }
    Ok(())
}

pub fn import_project(input: ProjectImportInput) -> Result<ImportedProject> {
    let mut files = match input.source {
        ProjectImportSource::Zip { archive_base64 } => archive_files(&archive_base64)?,
        ProjectImportSource::Files { files } => {
            ensure!(
                matches!(input.role, ImportRole::Working),
                "Use the original ZIP or snapshot for previous generated files"
            );
            ensure!(files.len() <= 2048, "Project file count limit exceeded");
            let mut result = Vec::new();
            let mut total = 0usize;
            let mut names = BTreeSet::new();
            for file in files {
                artifact::safe_path(&file.path)?;
                ensure!(names.insert(file.path.clone()), "Duplicate project path");
                ensure!(
                    file.content.len() <= FILE_LIMIT.div_ceil(3) * 4,
                    "Project file content limit exceeded"
                );
                let bytes = match file.encoding.as_str() {
                    "utf8" => file.content.into_bytes(),
                    "base64" => STANDARD.decode(file.content)?,
                    _ => anyhow::bail!("Unknown project file encoding"),
                };
                total = total
                    .checked_add(bytes.len())
                    .ok_or_else(|| anyhow::anyhow!("Project size overflow"))?;
                ensure!(
                    bytes.len() <= FILE_LIMIT && total <= PROJECT_LIMIT + MANIFEST_LIMIT,
                    "Project content size limit exceeded"
                );
                result.push(import_file(file.path, bytes, file.executable));
            }
            check_paths(result.iter().map(|file| file.path.clone()))?;
            result
        }
    };
    let mut content_bytes = 0usize;
    for file in &files {
        if [GENERATION_MANIFEST, REGENERATION_MANIFEST].contains(&file.path.as_str()) {
            ensure!(
                file.bytes <= MANIFEST_LIMIT,
                "Project metadata limit exceeded"
            );
        } else {
            content_bytes += file.bytes;
        }
    }
    ensure!(
        content_bytes <= PROJECT_LIMIT,
        "Project content size limit exceeded"
    );
    match input.role {
        ImportRole::Previous => validate_previous(&files)?,
        ImportRole::Working => files.retain(|file| file.path != REGENERATION_MANIFEST),
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let result = ImportedProject { files };
    ensure!(
        serde_json::to_vec(&result)?.len() <= IPC_LIMIT,
        "Imported project reply limit exceeded"
    );
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn zip(files: &[(&str, &[u8], bool)]) -> String {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (path, bytes, executable) in files {
            writer
                .start_file(
                    *path,
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated)
                        .unix_permissions(if *executable { 0o755 } else { 0o644 }),
                )
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        STANDARD.encode(writer.finish().unwrap().into_inner())
    }
    fn imported(role: ImportRole, archive_base64: String) -> Result<ImportedProject> {
        import_project(ProjectImportInput {
            role,
            source: ProjectImportSource::Zip { archive_base64 },
        })
    }
    fn original() -> ProjectArtifact {
        artifact::artifact(
            &ProjectInput {
                target: "fixture".into(),
                specification: serde_json::json!({}),
                options: BTreeMap::new(),
                include_secrets: false,
            },
            "fixture",
            BTreeMap::from([
                (
                    "bin/run".into(),
                    artifact::EmittedFile {
                        bytes: b"#!/bin/sh\n".to_vec(),
                        executable: true,
                    },
                ),
                ("bytes.bin".into(), vec![0, 255, 128].into()),
            ]),
        )
        .unwrap()
    }
    #[test]
    fn original_zip_roundtrip_checks_manifest_and_preserves_binary_and_modes() {
        let original = original();
        let baseline = imported(ImportRole::Previous, original.archive_base64.clone()).unwrap();
        assert_eq!(
            serde_json::to_value(baseline.files).unwrap(),
            serde_json::to_value(&original.files).unwrap()
        );
        let values = original
            .files
            .iter()
            .map(|file| {
                let bytes = if file.encoding == "utf8" {
                    file.content.as_bytes().to_vec()
                } else {
                    STANDARD.decode(&file.content).unwrap()
                };
                (format!("project/{}", file.path), bytes, file.executable)
            })
            .collect::<Vec<_>>();
        let packed = zip(&values
            .iter()
            .map(|(path, bytes, mode)| (path.as_str(), bytes.as_slice(), *mode))
            .collect::<Vec<_>>());
        assert_eq!(
            imported(ImportRole::Previous, packed).unwrap().files[0].path,
            "bin/run"
        );
        let mut changed = values;
        changed[0].1 = b"local edit".to_vec();
        let packed = zip(&changed
            .iter()
            .map(|(path, bytes, mode)| (path.as_str(), bytes.as_slice(), *mode))
            .collect::<Vec<_>>());
        assert!(imported(ImportRole::Previous, packed.clone()).is_err());
        let working = imported(ImportRole::Working, packed).unwrap();
        assert_eq!(working.files[0].content, "local edit");
        assert!(working.files[0].executable);
    }
    #[test]
    fn archives_reject_unsafe_duplicate_collision_symlink_and_expanded_size_inputs() {
        for paths in [
            vec!["../escape"],
            vec!["/absolute"],
            vec!["dir\\file"],
            vec!["a", "a/child"],
            vec!["a", "a/child/"],
        ] {
            let archive = zip(&paths
                .iter()
                .map(|path| (*path, b"x".as_slice(), false))
                .collect::<Vec<_>>());
            assert!(imported(ImportRole::Working, archive).is_err(), "{paths:?}");
        }
        assert!(
            imported(
                ImportRole::Working,
                STANDARD.encode(include_bytes!("../../tests/fixtures/project-duplicate.zip"))
            )
            .is_err()
        );
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .add_symlink(
                "linked",
                "../outside",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        assert!(
            imported(
                ImportRole::Working,
                STANDARD.encode(writer.finish().unwrap().into_inner())
            )
            .is_err()
        );
        let oversized = vec![0; FILE_LIMIT + 1];
        assert!(imported(ImportRole::Working, zip(&[("bomb.bin", &oversized, false)])).is_err());
        assert!(imported(ImportRole::Previous, zip(&[("file", b"x", false)])).is_err());
    }
    #[test]
    fn selected_working_files_recompute_hashes_and_drop_regeneration_metadata() {
        let file = |path: &str, content: &str| WorkingFile {
            path: path.into(),
            encoding: "base64".into(),
            content: content.into(),
            executable: true,
        };
        let source = ProjectImportSource::Files {
            files: vec![
                file("binary", &STANDARD.encode([255, 0])),
                file(REGENERATION_MANIFEST, &STANDARD.encode(b"metadata")),
            ],
        };
        let result = import_project(ProjectImportInput {
            role: ImportRole::Working,
            source: source.clone(),
        })
        .unwrap();
        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].bytes, 2);
        assert_eq!(
            result.files[0].sha256,
            format!("{:x}", Sha256::digest([255, 0]))
        );
        assert_eq!(result.files[0].encoding, "base64");
        assert!(result.files[0].executable);
        assert!(
            import_project(ProjectImportInput {
                role: ImportRole::Previous,
                source
            })
            .is_err()
        );
        assert!(
            import_project(ProjectImportInput {
                role: ImportRole::Working,
                source: ProjectImportSource::Files {
                    files: vec![file("same", "AA=="), file("same", "AA==")]
                }
            })
            .is_err()
        );
        assert!(
            import_project(ProjectImportInput {
                role: ImportRole::Working,
                source: ProjectImportSource::Files { files: vec![] }
            })
            .unwrap()
            .files
            .is_empty()
        );
    }
}
