mod common;
use axum::{Router, http::HeaderMap, routing::get};
use common::*;
use serde_json::{Value, json};
async fn api(
    http: &reqwest::Client,
    server: &str,
    token: Option<&str>,
    method: reqwest::Method,
    path: &str,
    body: Option<Value>,
) -> Value {
    let mut request = http.request(method, format!("{server}/api/{path}"));
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    if let Some(body) = body {
        request = request.json(&body);
    }
    request
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}
#[tokio::test]
async fn real_agent_executes_private_local_request_and_returns_owned_redacted_report() {
    let (endpoint, fixture) = serve(Router::new().route(
        "/verify",
        get(|headers: HeaderMap| async move {
            assert_eq!(headers["x-secret"], "runtime-agent-secret");
            axum::Json(json!({"received":headers["x-secret"].to_str().unwrap()}))
        }),
    ))
    .await;
    let temporary = tempfile::tempdir().unwrap();
    let hosted = moleapi_server::hosted_with_worker(
        moleapi_server::Config {
            database_url: format!(
                "sqlite://{}?mode=rwc",
                temporary.path().join("hosted.db").display()
            ),
            setup_token: "runner-setup".into(),
            allow_registration: true,
            allow_private_network: true,
        },
        std::path::Path::new(env!("CARGO_BIN_EXE_moleapi-cli")),
    )
    .await
    .unwrap();
    let (server, service) = serve(hosted).await;
    let http = reqwest::Client::new();
    let login = api(&http,&server,None,reqwest::Method::POST,"auth/register",Some(json!({"username":"agentowner","password":"fixture-password-123","setup_token":"runner-setup"}))).await;
    let token = login["token"].as_str().unwrap();
    let token_path = temporary.path().join("session.txt");
    std::fs::write(&token_path, token).unwrap();
    let mut data = source(&endpoint)["data"].clone();
    data["collections"][0]["requests"]
        .as_array_mut()
        .unwrap()
        .truncate(1);
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = json!(format!("{endpoint}/verify"));
    request["headers"] =
        json!([{"id":"secret","key":"X-Secret","value":"{{secret}}","enabled":true}]);
    request["extractions"] = json!([]);
    request["post_response_script"] = json!(
        "pm.test('received',()=>pm.expect(pm.response.json().received).to.equal(pm.variables.get('secret')));"
    );
    data["scenarios"] = json!([]);
    data["global_variables"] =
        json!([{"id":"s","key":"secret","value":"saved-host-secret","enabled":true,"secret":true}]);
    let original = api(
        &http,
        &server,
        Some(token),
        reqwest::Method::POST,
        "workspaces",
        Some(json!({"id":"w","name":"Hosted source","data":data})),
    )
    .await;
    let auth = [
        "--server",
        server.as_str(),
        "--token-file",
        token_path.to_str().unwrap(),
    ];
    let mut args = auth.to_vec();
    args.extend(["runner", "register", "--name", "LAN worker"]);
    let runner = result(cli(&args).await, 0);
    let id = runner["id"].as_str().unwrap();
    let config = temporary.path().join("task.json");
    write(
        &config,
        &json!({"runner_id":id,"expected_revision":original["revision"],"selection":{"collection_id":"c","environment_id":"dev"}}),
    );
    let mut args = auth.to_vec();
    args.extend([
        "runner",
        "queue",
        "--workspace",
        "w",
        "--config",
        config.to_str().unwrap(),
    ]);
    let queued = result(cli(&args).await, 0);
    let private = temporary.path().join("variables.json");
    write(
        &private,
        &json!({"temporary":{"secret":"runtime-agent-secret"}}),
    );
    let mut args = auth.to_vec();
    args.extend([
        "runner",
        "agent",
        "--id",
        id,
        "--once",
        "--variables-file",
        private.to_str().unwrap(),
    ]);
    let executed = result(cli(&args).await, 0);
    assert_eq!(executed["task_id"], queued["id"]);
    assert_eq!(executed["status"], "completed");
    assert_eq!(executed["completion_acknowledged"], true);
    let report = api(
        &http,
        &server,
        Some(token),
        reqwest::Method::GET,
        &format!(
            "workspaces/w/reports/{}",
            executed["report_id"].as_str().unwrap()
        ),
        None,
    )
    .await;
    assert_eq!(report["summary"]["passed"], 1, "{report}");
    for secret in ["runtime-agent-secret", "saved-host-secret"] {
        assert!(!report.to_string().contains(secret));
        assert!(!executed.to_string().contains(secret));
    }
    let after = api(
        &http,
        &server,
        Some(token),
        reqwest::Method::GET,
        "workspaces/w",
        None,
    )
    .await;
    assert_eq!(original, after);
    let mut args = auth.to_vec();
    args.extend(["runner", "tasks", "--workspace", "w"]);
    let tasks = result(cli(&args).await, 0);
    assert_eq!(tasks[0]["report_id"], executed["report_id"]);
    let empty = result(
        cli(&[auth.as_slice(), &["runner", "agent", "--id", id, "--once"]].concat()).await,
        0,
    );
    assert_eq!(empty["claimed"], false);
    service.abort();
    fixture.abort();
}

#[cfg(unix)]
#[tokio::test]
async fn interrupt_cancels_local_execution_and_persists_cancelled_hosted_task() {
    use std::{sync::Arc, time::Duration};
    let started = Arc::new(tokio::sync::Notify::new());
    let signal = started.clone();
    let (endpoint, fixture) = serve(Router::new().route(
        "/slow",
        get(move || {
            let signal = signal.clone();
            async move {
                signal.notify_one();
                tokio::time::sleep(Duration::from_secs(20)).await;
                "done"
            }
        }),
    ))
    .await;
    let temporary = tempfile::tempdir().unwrap();
    let hosted = moleapi_server::hosted_with_worker(
        moleapi_server::Config {
            database_url: format!(
                "sqlite://{}?mode=rwc",
                temporary.path().join("hosted.db").display()
            ),
            setup_token: "setup".into(),
            allow_registration: true,
            allow_private_network: true,
        },
        std::path::Path::new(env!("CARGO_BIN_EXE_moleapi-cli")),
    )
    .await
    .unwrap();
    let (server, service) = serve(hosted).await;
    let http = reqwest::Client::new();
    let login = api(&http,&server,None,reqwest::Method::POST,"auth/register",Some(json!({"username":"interruptowner","password":"fixture-password-123","setup_token":"setup"}))).await;
    let token = login["token"].as_str().unwrap();
    let mut data = source(&endpoint)["data"].clone();
    data["collections"][0]["requests"]
        .as_array_mut()
        .unwrap()
        .truncate(1);
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = json!(format!("{endpoint}/slow"));
    request["timeout_ms"] = json!(30000);
    request["extractions"] = json!([]);
    request["post_response_script"] = json!("");
    data["scenarios"] = json!([]);
    let workspace = api(
        &http,
        &server,
        Some(token),
        reqwest::Method::POST,
        "workspaces",
        Some(json!({"id":"w","name":"Interrupt","data":data})),
    )
    .await;
    let runner = api(
        &http,
        &server,
        Some(token),
        reqwest::Method::POST,
        "runners",
        Some(json!({"name":"Interrupt worker"})),
    )
    .await;
    let id = runner["id"].as_str().unwrap();
    let task = api(&http,&server,Some(token),reqwest::Method::POST,"workspaces/w/runner-tasks",Some(json!({"runner_id":id,"expected_revision":workspace["revision"],"selection":{"collection_id":"c"}}))).await;
    let child = tokio::process::Command::new(env!("CARGO_BIN_EXE_moleapi-cli"))
        .args([
            "--server",
            &server,
            "--token-env",
            "MOLEAPI_AGENT_TOKEN",
            "runner",
            "agent",
            "--id",
            id,
            "--once",
        ])
        .env("MOLEAPI_AGENT_TOKEN", token)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), started.notified())
        .await
        .unwrap();
    assert!(
        tokio::process::Command::new("kill")
            .args(["-INT", &child.id().unwrap().to_string()])
            .status()
            .await
            .unwrap()
            .success()
    );
    let summary = result(
        tokio::time::timeout(Duration::from_secs(8), child.wait_with_output())
            .await
            .unwrap()
            .unwrap(),
        130,
    );
    assert_eq!(summary["status"], "cancelled");
    assert_eq!(summary["completion_acknowledged"], true);
    let tasks = api(
        &http,
        &server,
        Some(token),
        reqwest::Method::GET,
        "workspaces/w/runner-tasks",
        None,
    )
    .await;
    assert_eq!(tasks[0]["id"], task["id"]);
    assert_eq!(tasks[0]["status"], "cancelled");
    service.abort();
    fixture.abort();
}
