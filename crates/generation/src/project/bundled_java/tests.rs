use super::*;

fn install(root: &Path, target: &str) -> PathBuf {
    let manifest: Value = serde_json::from_str(MANIFEST).unwrap();
    let directory = root.join("runtime/java");
    let path = directory.join(manifest["assets"][target]["java"].as_str().unwrap());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"resolver fixture; not executed").unwrap();
    std::fs::write(
        directory.join("moleapi-java-runtime.json"),
        serde_json::to_vec(&serde_json::json!({
            "format":"moleapi-java-runtime-v1","version":manifest["version"],"target":target,
            "archive_sha256":manifest["assets"][target]["sha256"]
        }))
        .unwrap(),
    )
    .unwrap();
    path
}

#[test]
fn exact_platform_receipts_resolve_original_layouts_and_reject_other_platforms() {
    let manifest: Value = serde_json::from_str(MANIFEST).unwrap();
    for target in manifest["assets"].as_object().unwrap().keys() {
        let root = tempfile::tempdir().unwrap();
        let executable = install(root.path(), target);
        assert_eq!(resolve(root.path(), target).unwrap(), Some(executable));
        assert!(resolve(root.path(), "not-the-platform").is_err());
    }
}

#[test]
fn absent_bundles_keep_dependency_state_and_explicit_choices_override_invalid_bundles() {
    let root = tempfile::tempdir().unwrap();
    assert!(bundled_java(root.path()).unwrap().is_none());
    let worker = std::env::current_exe().unwrap();
    let explicit = ProjectRuntime::new(worker.clone(), Some(worker.clone())).unwrap();
    let explicit = explicit
        .with_bundled_java(Path::new("not-an-absolute-resource-directory"))
        .unwrap();
    assert_eq!(explicit.java_path(), Some(worker));
    if let Some(target) = target() {
        let executable = install(root.path(), target);
        let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), None)
            .unwrap()
            .with_bundled_java(root.path())
            .unwrap();
        assert_eq!(runtime.java_path(), Some(executable));
    }
}

#[test]
fn oversized_modified_and_incomplete_receipts_cannot_select_an_executable() {
    let root = tempfile::tempdir().unwrap();
    let target = "x86_64-unknown-linux-gnu";
    let executable = install(root.path(), target);
    let receipt = root.path().join("runtime/java/moleapi-java-runtime.json");
    let original = std::fs::read(&receipt).unwrap();
    let mut value: Value = serde_json::from_slice(&original).unwrap();
    value["archive_sha256"] = "modified".into();
    std::fs::write(&receipt, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(resolve(root.path(), target).is_err());
    std::fs::write(&receipt, vec![b' '; 8193]).unwrap();
    assert!(
        resolve(root.path(), target)
            .unwrap_err()
            .to_string()
            .contains("limit")
    );
    std::fs::write(&receipt, original).unwrap();
    std::fs::remove_file(executable).unwrap();
    assert!(resolve(root.path(), target).is_err());
}

#[cfg(unix)]
#[test]
fn executable_links_cannot_escape_the_owned_payload() {
    let root = tempfile::tempdir().unwrap();
    let executable = install(root.path(), "x86_64-unknown-linux-gnu");
    std::fs::remove_file(&executable).unwrap();
    let other = tempfile::NamedTempFile::new().unwrap();
    std::os::unix::fs::symlink(other.path(), executable).unwrap();
    assert!(
        resolve(root.path(), "x86_64-unknown-linux-gnu")
            .unwrap_err()
            .to_string()
            .contains("escapes")
    );
}

#[cfg(unix)]
#[test]
fn a_payload_directory_link_cannot_move_runtime_execution_outside_the_bundle() {
    let root = tempfile::tempdir().unwrap();
    install(root.path(), "x86_64-unknown-linux-gnu");
    let payload = root.path().join("runtime/java/payload");
    let other = tempfile::tempdir().unwrap();
    std::fs::rename(&payload, other.path().join("external")).unwrap();
    std::os::unix::fs::symlink(other.path().join("external"), payload).unwrap();
    assert!(
        resolve(root.path(), "x86_64-unknown-linux-gnu")
            .unwrap_err()
            .to_string()
            .contains("escapes")
    );
}
