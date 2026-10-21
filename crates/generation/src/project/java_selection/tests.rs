use super::*;

#[test]
fn requires_a_recognized_java17_or_newer_version() {
    for text in [
        "openjdk version \"21.0.12.1\" 2026-07-21",
        "java version \"17.0.18\"",
        "openjdk version \"25-ea\"",
    ] {
        assert!(java_major(text.as_bytes()).unwrap() >= 17);
    }
    for text in [
        "openjdk version \"16.0.2\"",
        "java version \"1.8.0\"",
        "program 99.0",
        "openjdk version \"invalid\"",
    ] {
        assert!(java_major(text.as_bytes()).is_err());
    }
}

#[cfg(unix)]
fn executable(directory: &Path, name: &str, script: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = directory.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    path
}

#[cfg(unix)]
#[tokio::test]
async fn selection_is_shared_atomic_private_and_restored_without_path_discovery() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().unwrap();
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), None).unwrap();
    let shared = runtime.clone();
    let preference = directory.path().join("java-runtime.json");
    let java = executable(
        directory.path(),
        "chosen java",
        "[ \"$1\" = -version ] || exit 2\nprintf 'openjdk version \"21.0.12\"\\n' >&2",
    );
    assert_eq!(
        runtime
            .select_native_java(java.clone(), &preference)
            .await
            .unwrap(),
        21
    );
    assert_eq!(shared.java_path(), Some(java.clone()));
    assert_eq!(
        std::fs::metadata(&preference).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let original = std::fs::read(&preference).unwrap();
    let restarted = ProjectRuntime::new(std::env::current_exe().unwrap(), None).unwrap();
    restarted.restore_native_java(&preference).unwrap();
    assert_eq!(restarted.java_path(), Some(java.clone()));
    let old = executable(
        directory.path(),
        "old",
        "printf 'java version \"1.8.0\"\\n' >&2",
    );
    assert!(runtime.select_native_java(old, &preference).await.is_err());
    let broken = executable(
        directory.path(),
        "broken",
        "printf 'openjdk version \"21\"\\n' >&2\nexit 1",
    );
    assert!(
        runtime
            .select_native_java(broken, &preference)
            .await
            .is_err()
    );
    let other = executable(directory.path(), "new", "printf 'java version \"25\"\\n'");
    assert!(
        runtime
            .select_native_java(other.clone(), &preference.join("unwritable"))
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(&preference).unwrap(), original);
    assert_eq!(shared.java_path(), Some(java));
    assert_eq!(
        runtime
            .select_native_java(other.clone(), &preference)
            .await
            .unwrap(),
        25
    );
    restarted.restore_native_java(&preference).unwrap();
    assert_eq!(restarted.java_path(), Some(other));
}

#[cfg(unix)]
#[tokio::test]
async fn invalid_bounded_and_timed_out_probes_do_not_create_a_preference() {
    let directory = tempfile::tempdir().unwrap();
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), None).unwrap();
    let preference = directory.path().join("java-runtime.json");
    let large = executable(
        directory.path(),
        "large",
        "/usr/bin/head -c 17000 /dev/zero",
    );
    assert!(
        read_version(tokio::io::repeat(0))
            .await
            .unwrap_err()
            .to_string()
            .contains("limit")
    );
    // A loaded test runner can hit the process watchdog before this script's
    // bounded output is consumed. Both outcomes must reject the selection.
    assert!(
        runtime
            .select_native_java(large, &preference)
            .await
            .is_err()
    );
    let slow = executable(directory.path(), "slow", "exec /bin/sleep 20");
    let failure = runtime
        .select_native_java(slow, &preference)
        .await
        .expect_err("A hanging Java probe must be rejected");
    assert!(failure.to_string().contains("timed out"), "{failure:#}");
    assert!(
        runtime
            .select_native_java(PathBuf::from("relative-java"), &preference)
            .await
            .is_err()
    );
    assert!(!preference.exists());
    assert!(runtime.java_path().is_none());
    for content in [
        r#"{"version":2,"java":"/not-selected"}"#,
        r#"{"version":1,"java":"relative-java"}"#,
        r#"{"version":1,"java":"/not-selected","execute":true}"#,
    ] {
        std::fs::write(&preference, content).unwrap();
        assert!(runtime.restore_native_java(&preference).is_err());
        assert!(runtime.java_path().is_none());
    }
    std::fs::write(&preference, vec![b' '; PREFERENCE_LIMIT + 1]).unwrap();
    assert!(
        runtime
            .restore_native_java(&preference)
            .unwrap_err()
            .to_string()
            .contains("limit")
    );
}
