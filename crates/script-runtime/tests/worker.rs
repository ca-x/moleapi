use moleapi_core::{RequestSpec, SavedDataset, ScriptMode, VariableScopes};
use moleapi_script_runtime::{
    WORKER_DEADLINE, run_worker, run_worker_with_datasets, run_worker_with_mode,
};
use serde_json::json;
use std::{
    path::Path,
    time::{Duration, Instant},
};
fn request() -> RequestSpec {
    serde_json::from_value(json!({"id":"r","name":"r","method":"GET","url":"https://example.com/","description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":true,"verify_tls":true,"assertions":[],"examples":[]})).unwrap()
}
fn executable() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_moleapi-script-worker"))
}
#[tokio::test]
async fn worker_kills_native_bigint_that_cannot_poll_vm_interrupts() {
    for script in [
        "(2n ** 1000000n).toString();",
        "((2n ** 1000000n) / (3n ** 100000n)).toString();",
    ] {
        let start = Instant::now();
        let result = run_worker(
            executable(),
            vec![script.into()],
            &request(),
            None,
            &VariableScopes::default(),
        )
        .await;
        let failure = result.unwrap_err();
        assert!(failure.message.contains("1 second deadline"), "{failure:?}");
        assert!(!failure.privacy_complete);
        assert!(
            start.elapsed() < WORKER_DEADLINE + Duration::from_secs(1),
            "native work escaped worker deadline"
        );
    }
}
#[tokio::test]
async fn worker_returns_live_results_and_failed_phase_privacy_without_mutations() {
    let output = run_worker(
        executable(),
        vec!["pm.environment.set('next','value');console.log('hello');".into()],
        &request(),
        None,
        &VariableScopes::default(),
    )
    .await
    .unwrap();
    assert_eq!(output.updates[0].value.as_deref(), Some("value"));
    assert!(output.private_values.contains("value"));
    let failure=run_worker(executable(),vec!["pm.variables.set('issued','generated-private');throw new Error(pm.variables.get('issued'));".into()],&request(),None,&VariableScopes::default()).await.unwrap_err();
    assert!(failure.privacy_complete);
    assert!(failure.private_values.contains("generated-private"));
}
#[tokio::test]
async fn versioned_worker_settles_await_and_rejects_detached_or_private_failures() {
    let scopes = VariableScopes::default();
    let script = "const value = await Promise.resolve('ready'); pm.variables.set('result', value);";
    assert!(
        run_worker(executable(), vec![script.into()], &request(), None, &scopes)
            .await
            .is_err()
    );
    let output = run_worker_with_mode(
        executable(),
        vec![script.into()],
        &request(),
        None,
        &scopes,
        ScriptMode::V2,
    )
    .await
    .unwrap();
    assert_eq!(output.updates[0].value.as_deref(), Some("ready"));
    let failure = run_worker_with_mode(executable(), vec!["pm.variables.set('secret','private-value'); await Promise.reject(new Error(pm.variables.get('secret')));".into()], &request(), None, &scopes, ScriptMode::V2).await.unwrap_err();
    assert!(failure.privacy_complete);
    assert!(failure.private_values.contains("private-value"));
    let detached = run_worker_with_mode(
        executable(),
        vec!["Promise.resolve().then(() => pm.variables.set('late','no'));".into()],
        &request(),
        None,
        &scopes,
        ScriptMode::V2,
    )
    .await
    .unwrap_err();
    assert!(detached.message.contains("Unsettled"));
}
#[tokio::test]
async fn worker_queries_owned_literal_snapshot_without_exposing_source() {
    let dataset: SavedDataset = serde_json::from_value(json!({"id":"d","name":"Private","source":{"format":"json","source":"[{\"token\":\"private-row\"}]"}})).unwrap();
    let (snapshot, values) =
        moleapi_script_runtime::dataset_query::snapshot_dataset(&dataset).unwrap();
    let mut scopes = VariableScopes::default();
    scopes.private_values.extend(values);
    let output = run_worker_with_datasets(executable(), vec!["const result=await pm.datasets('d').executeQuery('SELECT token FROM data'); for await (const row of result.rows) { pm.test('row',()=>pm.expect(row.token).to.equal('private-row')); }".into()], &request(), None, &scopes, vec![snapshot.clone()]).await.unwrap();
    assert!(output.tests[0].passed);
    let failure = run_worker_with_datasets(executable(), vec!["const result=await pm.datasets('d').executeQuery('SELECT token FROM data'); for await (const row of result.rows) throw new Error(row.token);".into()], &request(), None, &scopes, vec![snapshot]).await.unwrap_err();
    assert!(failure.private_values.contains("private-row"));
}
#[cfg(unix)]
fn fake_worker(directory: &Path, name: &str, source: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = directory.join(name);
    std::fs::write(&path, source).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    path
}
#[cfg(unix)]
#[tokio::test]
async fn cancellation_kills_and_reaps_the_worker_process() {
    let directory = tempfile::tempdir().unwrap();
    let pid_file = directory.path().join("pid");
    let worker = fake_worker(
        directory.path(),
        "cancel",
        &format!(
            "#!/bin/sh\nprintf '%s' \"$$\" > '{}'\nexec /bin/sleep 30\n",
            pid_file.display()
        ),
    );
    let task = tokio::spawn(async move {
        run_worker(
            &worker,
            vec!["42".into()],
            &request(),
            None,
            &VariableScopes::default(),
        )
        .await
    });
    let deadline = Instant::now() + Duration::from_secs(1);
    while !pid_file.exists() {
        assert!(Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let pid = std::fs::read_to_string(&pid_file).unwrap();
    task.abort();
    let _ = task.await;
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        let status = std::process::Command::new("/bin/kill")
            .arg("-0")
            .arg(&pid)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        if !status.success() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "cancelled worker was not killed/reaped"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
#[cfg(unix)]
#[tokio::test]
async fn worker_protocol_output_is_bounded_and_stderr_is_not_forwarded() {
    let directory = tempfile::tempdir().unwrap();
    let worker = fake_worker(
        directory.path(),
        "flood",
        "#!/bin/sh\n/bin/cat >/dev/null\nprintf 'private-stderr' >&2\nexec /bin/dd if=/dev/zero bs=1048576 count=34 2>/dev/null\n",
    );
    let failure = run_worker(
        &worker,
        vec!["42".into()],
        &request(),
        None,
        &VariableScopes::default(),
    )
    .await
    .unwrap_err();
    // The absolute watchdog may win over the byte quota on a loaded runner.
    // Both must stop the flood; neither may forward private stderr.
    assert!(
        failure.message.contains("output exceeds limit") || failure.message.contains("deadline"),
        "{failure:?}"
    );
    assert!(!failure.message.contains("private-stderr"));
}
#[tokio::test]
async fn worker_rejects_caught_privacy_capture_overflow() {
    let script = "for(let i=0;i<4;i++){pm.variables.set('x','a'.repeat(1048500)+i);pm.variables.unset('x');}try{pm.variables.set('lost','new-private-'.repeat(40));}catch(error){console.log(error.message);}42;";
    let failure = run_worker(
        executable(),
        vec![script.into()],
        &request(),
        None,
        &VariableScopes::default(),
    )
    .await
    .unwrap_err();
    assert!(!failure.privacy_complete);
    assert!(
        failure
            .message
            .contains("Private variable history exceeds execution limit")
    );
    assert_eq!(failure.private_values.len(), 4);
}
