use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
#[test]
fn artifact_zip_records_checksums_and_preserves_executable_permissions() {
    let input = ProjectInput {
        specification: serde_json::json!({}),
        target: "test".into(),
        options: BTreeMap::new(),
        include_secrets: false,
        templates: None,
    };
    let files = BTreeMap::from([(
        "run.sh".into(),
        artifact::EmittedFile {
            bytes: b"#!/bin/sh\nexit 0\n".to_vec(),
            executable: true,
        },
    )]);
    let result = artifact::artifact(&input, "test-engine", files).unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(
        STANDARD.decode(result.archive_base64).unwrap(),
    ))
    .unwrap();
    assert_eq!(
        zip.by_name("run.sh").unwrap().unix_mode().unwrap() & 0o777,
        0o755
    );
    let manifest = result
        .files
        .iter()
        .find(|f| f.path == "moleapi-generation.json")
        .unwrap();
    let meta: Value = serde_json::from_str(&manifest.content).unwrap();
    assert_eq!(meta["files"][0]["sha256"], result.files[0].sha256);
    assert!(
        artifact::artifact(
            &input,
            "test",
            BTreeMap::from([("../outside".into(), vec![1].into())])
        )
        .is_err()
    );
}
#[cfg(unix)]
#[test]
fn symlinks_and_live_output_growth_fail_closed() {
    let directory = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink("/tmp", directory.path().join("escape")).unwrap();
    assert!(artifact::collect(directory.path()).is_err());
    std::fs::remove_file(directory.path().join("escape")).unwrap();
    let file = std::fs::File::create(directory.path().join("large")).unwrap();
    file.set_len((FILE_LIMIT + 1) as u64).unwrap();
    assert!(artifact::check_output_budget(directory.path()).is_err());
}
#[test]
fn target_options_do_not_allow_paths_or_unsupported_properties() {
    let mut input = ProjectInput {
        specification: serde_json::json!({}),
        target: "rust-progenitor".into(),
        options: BTreeMap::from([("packageName".into(), Value::String("../outside".into()))]),
        include_secrets: false,
        templates: None,
    };
    assert!(validate_options(&input).is_err());
    input.options = BTreeMap::from([("templateDir".into(), Value::String("outside".into()))]);
    assert!(validate_options(&input).is_err());
}
