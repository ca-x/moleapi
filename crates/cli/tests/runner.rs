mod common;
use axum::{Json, Router, extract::Query, http::HeaderMap, routing::get};
use common::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;
#[tokio::test]
async fn offline_runs_share_scripts_extraction_environment_dataset_and_exit_codes() {
    let router = Router::new()
        .route(
            "/token",
            get(|| async { Json(json!({"token":"private-fixture-token"})) }),
        )
        .route(
            "/echo",
            get(
                |headers: HeaderMap, Query(query): Query<BTreeMap<String, String>>| async move {
                    assert_eq!(headers.get("x-token").unwrap(), "private-fixture-token");
                    Json(json!(query))
                },
            ),
        );
    let (url, fixture) = serve(router).await;
    let temporary = tempfile::tempdir().unwrap();
    let input = temporary.path().join("workspace.json");
    let report = temporary.path().join("report.xml");
    write(&input, &source(&url));
    let args = [
        "run",
        "--input",
        input.to_str().unwrap(),
        "--scenario",
        "Scenario",
        "--environment",
        "Development",
        "--dataset",
        "Rows",
        "--ci",
        "--no-notifications",
        "--report",
        report.to_str().unwrap(),
        "--reporter",
        "junit",
    ];
    let summary = result(cli(&args).await, 0);
    assert_eq!(summary["passed"], 4);
    assert_eq!(summary["failed"], 0);
    assert_eq!(summary["report_id"], Value::Null);
    assert_eq!(summary["temporary_workspace"], true);
    let xml = std::fs::read_to_string(&report).unwrap();
    let document = roxmltree::Document::parse(&xml).unwrap();
    assert!(
        document
            .descendants()
            .any(|node| node.has_tag_name("testcase"))
    );
    assert!(!xml.contains("private-fixture-token"));
    let mut failed = source(&url);
    failed["data"]["collections"][0]["requests"][0]["post_response_script"] =
        json!("pm.test('fail',()=>pm.expect(1).to.equal(2));");
    write(&input, &failed);
    let output = result(
        cli(&[
            "run",
            "--input",
            input.to_str().unwrap(),
            "--request",
            "Token",
            "--environment",
            "Development",
            "--dataset",
            "Rows",
        ])
        .await,
        1,
    );
    assert_eq!(output["executed_steps"], 2);
    assert_eq!(output["failed"], 2);
    failed["data"]["collections"][0]["requests"] = json!([]);
    failed["data"]["scenarios"] = json!([]);
    write(&input, &failed);
    let output = result(cli(&["run", "--input", input.to_str().unwrap()]).await, 1);
    assert_eq!(output["executed_steps"], 0);
    fixture.abort();
}
#[tokio::test]
async fn remote_login_tokens_owned_runs_exports_and_selection_errors() {
    let (url, fixture) = serve(Router::new().route(
        "/token",
        get(|| async { Json(json!({"token":"private-fixture-token"})) }),
    ))
    .await;
    let temporary = tempfile::tempdir().unwrap();
    let database = temporary.path().join("remote.db");
    let server = moleapi_server::hosted_with_worker(
        moleapi_server::Config {
            database_url: format!("sqlite://{}?mode=rwc", database.display()),
            setup_token: "fixture-setup".into(),
            allow_registration: true,
            allow_private_network: true,
        },
        std::path::Path::new(env!("CARGO_BIN_EXE_moleapi-cli")),
    )
    .await
    .unwrap();
    let (service, handle) = serve(server).await;
    let http = reqwest::Client::new();
    let registered = http.post(format!("{service}/api/auth/register")).json(&json!({"username":"tester","password":"fixture-password-123","setup_token":"fixture-setup"})).send().await.unwrap();
    assert!(registered.status().is_success());
    let token = temporary.path().join("token.txt");
    let login = tokio::process::Command::new(env!("CARGO_BIN_EXE_moleapi-cli"))
        .args([
            "--server",
            &service,
            "login",
            "--username",
            "tester",
            "--password-env",
            "MOLEAPI_TEST_PASSWORD",
            "--output",
            token.to_str().unwrap(),
        ])
        .env("MOLEAPI_TEST_PASSWORD", "fixture-password-123")
        .output()
        .await
        .unwrap();
    result(login, 0);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&token).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    let input = temporary.path().join("input.json");
    let mut data = source(&url);
    data["data"]["collections"][0]["requests"]
        .as_array_mut()
        .unwrap()
        .truncate(1);
    data["data"]["scenarios"] = json!([]);
    write(&input, &data);
    let prefix = [
        "--server",
        service.as_str(),
        "--token-file",
        token.to_str().unwrap(),
    ];
    let mut args = prefix.to_vec();
    args.extend(["import", "--input", input.to_str().unwrap()]);
    let workspace = result(cli(&args).await, 0);
    let id = workspace["id"].as_str().unwrap();
    let report = temporary.path().join("report.json");
    let mut args = prefix.to_vec();
    args.extend([
        "run",
        "--workspace",
        id,
        "--collection",
        "Smoke",
        "--request",
        "Token",
        "--environment",
        "Development",
        "--dataset",
        "Rows",
        "--ci",
        "--report",
        report.to_str().unwrap(),
        "--reporter",
        "json",
    ]);
    let summary = result(cli(&args).await, 0);
    assert_eq!(summary["passed"], 2);
    assert!(summary["report_id"].is_string());
    assert_eq!(summary["temporary_workspace"], false);
    let contents = std::fs::read_to_string(&report).unwrap();
    let _: Value = serde_json::from_str(&contents).unwrap();
    assert!(!contents.contains("private-fixture-token"));
    let mut args = prefix.to_vec();
    args.extend(["run", "--workspace", id, "--environment", "missing"]);
    assert_eq!(cli(&args).await.status.code(), Some(2));
    let mut args = prefix.to_vec();
    args.extend(["run", "--workspace", id, "--request", "missing"]);
    assert_eq!(cli(&args).await.status.code(), Some(2));
    let unauthorized = cli(&["--server", &service, "list", "workspaces"]).await;
    assert_eq!(unauthorized.status.code(), Some(2));
    handle.abort();
    fixture.abort();
}

#[cfg(unix)]
#[tokio::test]
async fn ctrl_c_cancels_owned_execution_without_starting_stored_schedules() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let started = Arc::new(tokio::sync::Notify::new());
    let waiting = started.clone();
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    let (url, fixture) = serve(Router::new().route(
        "/slow",
        get(move || {
            let started = waiting.clone();
            let calls = counted.clone();
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                started.notify_one();
                tokio::time::sleep(std::time::Duration::from_secs(20)).await;
                "done"
            }
        }),
    ))
    .await;
    let temporary = tempfile::tempdir().unwrap();
    let database = temporary.path().join("cancel.db");
    let offline = moleapi_server::offline_with_worker(
        &database,
        std::path::Path::new(env!("CARGO_BIN_EXE_moleapi-cli")),
    )
    .await
    .unwrap();
    let (service, handle) = serve(offline).await;
    let mut data = source(&url);
    let request = &mut data["data"]["collections"][0]["requests"][0];
    request["url"] = json!(format!("{url}/slow"));
    request["timeout_ms"] = json!(30000);
    request["extractions"] = json!([]);
    request["post_response_script"] = json!("");
    data["data"]["collections"][0]["requests"]
        .as_array_mut()
        .unwrap()
        .truncate(1);
    data["data"]["scenarios"] = json!([]);
    let http = reqwest::Client::new();
    let saved: Value = http
        .post(format!("{service}/api/workspaces"))
        .json(&json!({"name":"Cancel","data":data["data"]}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let workspace = saved["id"].as_str().unwrap();
    let scheduled=http.post(format!("{service}/api/workspaces/{workspace}/schedules")).json(&json!({"name":"Do not start","cron":"* * * * * *","timezone":"UTC","enabled":true,"collection_id":"c"})).send().await.unwrap();
    assert!(scheduled.status().is_success());
    let child = tokio::process::Command::new(env!("CARGO_BIN_EXE_moleapi-cli"))
        .args([
            "--database",
            database.to_str().unwrap(),
            "run",
            "--workspace",
            workspace,
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), started.notified())
        .await
        .unwrap();
    // A due schedule would dispatch during this live CLI run if background loops were enabled.
    tokio::time::sleep(std::time::Duration::from_millis(2200)).await;
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let signalled = tokio::process::Command::new("kill")
        .args(["-INT", &child.id().unwrap().to_string()])
        .status()
        .await
        .unwrap();
    assert!(signalled.success());
    let output = tokio::time::timeout(std::time::Duration::from_secs(8), child.wait_with_output())
        .await
        .unwrap()
        .unwrap();
    let summary = result(output, 130);
    assert_eq!(summary["stopped_reason"], "cancelled");
    let schedules: Value = http
        .get(format!("{service}/api/workspaces/{workspace}/schedules"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(schedules[0]["last_run"].is_null());
    handle.abort();
    fixture.abort();
}
