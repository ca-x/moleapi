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
fn runner_args<'a>(auth: &[&'a str], command: &[&'a str]) -> Vec<&'a str> {
    auth.iter().chain(command).copied().collect()
}
#[tokio::test]
async fn cli_manages_runner_labels_without_changing_omitted_labels() {
    let temporary = tempfile::tempdir().unwrap();
    let hosted = moleapi_server::hosted_with_worker(
        moleapi_server::Config {
            database_url: format!(
                "sqlite://{}?mode=rwc",
                temporary.path().join("labels.db").display()
            ),
            setup_token: "runner-label-setup".into(),
            allow_registration: true,
            allow_private_network: true,
        },
        std::path::Path::new(env!("CARGO_BIN_EXE_moleapi-cli")),
    )
    .await
    .unwrap();
    let (server, service) = serve(hosted).await;
    let http = reqwest::Client::new();
    let login = api(&http, &server, None, reqwest::Method::POST, "auth/register", Some(json!({"username":"labelowner","password":"fixture-password-123","setup_token":"runner-label-setup"}))).await;
    let token_path = temporary.path().join("session.txt");
    std::fs::write(&token_path, login["token"].as_str().unwrap()).unwrap();
    let auth = [
        "--server",
        server.as_str(),
        "--token-file",
        token_path.to_str().unwrap(),
        "runner",
    ];
    let registered = result(
        cli(&runner_args(
            &auth,
            &[
                "register", "--name", "LAN", "--label", "linux", "--label", "ci",
            ],
        ))
        .await,
        0,
    );
    assert_eq!(registered["labels"], json!(["linux", "ci"]));
    let id = registered["id"].as_str().unwrap();
    let listed = result(cli(&runner_args(&auth, &["list"])).await, 0);
    assert_eq!(listed[0]["labels"], json!(["linux", "ci"]));

    let replaced = result(
        cli(&runner_args(
            &auth,
            &[
                "set",
                "--id",
                id,
                "--name",
                "LAN",
                "--enabled",
                "true",
                "--revision",
                "1",
                "--label",
                "staging",
            ],
        ))
        .await,
        0,
    );
    assert_eq!(replaced["labels"], json!(["staging"]));
    let preserved = result(
        cli(&runner_args(
            &auth,
            &[
                "set",
                "--id",
                id,
                "--name",
                "LAN 2",
                "--enabled",
                "true",
                "--revision",
                "2",
            ],
        ))
        .await,
        0,
    );
    assert_eq!(preserved["labels"], json!(["staging"]));

    for command in [
        vec![
            "register",
            "--name",
            "bad",
            "--label",
            "duplicate",
            "--label",
            "duplicate",
        ],
        vec![
            "set",
            "--id",
            id,
            "--name",
            "LAN 2",
            "--enabled",
            "true",
            "--revision",
            "3",
            "--label",
            "Invalid",
        ],
        vec![
            "set",
            "--id",
            id,
            "--name",
            "LAN 2",
            "--enabled",
            "true",
            "--revision",
            "3",
            "--label",
            "valid",
            "--clear-labels",
        ],
    ] {
        let failed = cli(&runner_args(&auth, &command)).await;
        assert_eq!(
            failed.status.code(),
            Some(2),
            "{}",
            String::from_utf8_lossy(&failed.stderr)
        );
    }
    let still = result(cli(&runner_args(&auth, &["list"])).await, 0);
    assert_eq!(still[0]["revision"], 3);
    assert_eq!(still[0]["labels"], json!(["staging"]));

    let cleared = result(
        cli(&runner_args(
            &auth,
            &[
                "set",
                "--id",
                id,
                "--name",
                "LAN 2",
                "--enabled",
                "true",
                "--revision",
                "3",
                "--clear-labels",
            ],
        ))
        .await,
        0,
    );
    assert_eq!(cleared["labels"], json!([]));
    service.abort();
}
#[tokio::test]
async fn real_agent_executes_private_local_request_and_returns_owned_redacted_report() {
    let (endpoint, fixture) = serve(Router::new().route(
        "/verify",
        get(|headers: HeaderMap| async move {
            assert_eq!(headers["x-secret"], "runtime-agent-secret");
            assert_eq!(headers["x-view"], "one");
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
    data["script_mode"] = json!("v2");
    data["datasets"][0]["views"] =
        json!([{"id":"agent-view","name":"Agent row","sql":"SELECT row FROM data WHERE row = ?"}]);
    data["collections"][0]["requests"]
        .as_array_mut()
        .unwrap()
        .truncate(1);
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = json!(format!("{endpoint}/verify"));
    request["pre_request_script"] = json!(
        "const result=await pm.datasets('rows').executeView('agent-view',['one']); for await (const row of result.rows) pm.request.headers.upsert({key:'x-view',value:row.row});"
    );
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
    args.extend([
        "runner",
        "register",
        "--name",
        "LAN worker",
        "--label",
        "lan",
    ]);
    let runner = result(cli(&args).await, 0);
    let id = runner["id"].as_str().unwrap();
    let agent_token_path = temporary.path().join("agent-token.txt");
    let mut credential_args = auth.to_vec();
    credential_args.extend([
        "runner",
        "credentials",
        "create",
        "--runner",
        id,
        "--workspace",
        "w",
        "--name",
        "LAN workspace agent",
        "--output",
        agent_token_path.to_str().unwrap(),
    ]);
    let credential = result(cli(&credential_args).await, 0);
    let agent_token = std::fs::read_to_string(&agent_token_path).unwrap();
    assert!(agent_token.starts_with("moleapi_runner_"));
    assert!(!credential.to_string().contains(&agent_token));
    let listed = result(
        cli(&[
            auth.as_slice(),
            &["runner", "credentials", "list", "--runner", id],
        ]
        .concat())
        .await,
        0,
    );
    assert_eq!(listed[0]["id"], credential["id"]);
    let group_config = temporary.path().join("group-task.json");
    write(
        &group_config,
        &json!({"runner_label":"lan","expected_revision":original["revision"],"selection":{"collection_id":"c"}}),
    );
    let grouped = result(
        cli(&[
            auth.as_slice(),
            &[
                "runner",
                "queue",
                "--workspace",
                "w",
                "--config",
                group_config.to_str().unwrap(),
            ],
        ]
        .concat())
        .await,
        0,
    );
    assert_eq!(grouped["runner_id"], id);
    assert_eq!(grouped["target_label"], "lan");
    result(
        cli(&[
            auth.as_slice(),
            &[
                "runner",
                "cancel",
                "--workspace",
                "w",
                "--id",
                grouped["id"].as_str().unwrap(),
            ],
        ]
        .concat())
        .await,
        0,
    );
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
    let agent_auth = [
        "--server",
        server.as_str(),
        "--token-file",
        agent_token_path.to_str().unwrap(),
    ];
    let mut args = agent_auth.to_vec();
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
    assert_eq!(executed["response_history_omitted"], 0);
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
    let history_id = report["results"][0]["history_id"].as_str().unwrap();
    let response = api(
        &http,
        &server,
        Some(token),
        reqwest::Method::GET,
        &format!(
            "workspaces/w/reports/{}/steps/0/response",
            executed["report_id"].as_str().unwrap()
        ),
        None,
    )
    .await;
    assert_eq!(response["id"], history_id);
    assert_eq!(response["response"]["status"], 200);
    for secret in ["runtime-agent-secret", "saved-host-secret"] {
        assert!(!report.to_string().contains(secret));
        assert!(!executed.to_string().contains(secret));
        assert!(!response.to_string().contains(secret));
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
        cli(&[
            agent_auth.as_slice(),
            &["runner", "agent", "--id", id, "--once"],
        ]
        .concat())
        .await,
        0,
    );
    assert_eq!(empty["claimed"], false);
    let mut updated_data = after["data"].clone();
    let mut query = updated_data["collections"][0]["requests"][0].clone();
    query["id"] = "data-query".into();
    query["url"] = "".into();
    query["headers"] = json!([]);
    query["post_response_script"] = "".into();
    query["pre_request_script"] = "".into();
    query["protocol"] = json!({"kind":"data","source":"local_file","file_name":"runner.csv","file_format":"csv","file_base64":"dmFsdWUKb25lCnR3bwo=","table_name":"data","sql":"SELECT value FROM data","read_only":true});
    updated_data["collections"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"sources","name":"Sources","description":"","requests":[query]}));
    updated_data["datasets"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"live","name":"Live","query":{"request_id":"data-query"}}));
    let updated = api(&http,&server,Some(token),reqwest::Method::PUT,"workspaces/w",Some(json!({"name":"Hosted source","expected_revision":after["revision"],"data":updated_data}))).await;
    let live_task = api(&http,&server,Some(token),reqwest::Method::POST,"workspaces/w/runner-tasks",Some(json!({"runner_id":id,"expected_revision":updated["revision"],"selection":{"collection_id":"c","environment_id":"dev","dataset_id":"live"}}))).await;
    let live_agent = result(
        cli(&[
            agent_auth.as_slice(),
            &[
                "runner",
                "agent",
                "--id",
                id,
                "--once",
                "--variables-file",
                private.to_str().unwrap(),
            ],
        ]
        .concat())
        .await,
        0,
    );
    assert_eq!(live_agent["task_id"], live_task["id"]);
    assert_eq!(live_agent["status"], "completed");
    let live_report = api(
        &http,
        &server,
        Some(token),
        reqwest::Method::GET,
        &format!(
            "workspaces/w/reports/{}",
            live_agent["report_id"].as_str().unwrap()
        ),
        None,
    )
    .await;
    assert_eq!(live_report["summary"]["passed"], 2, "{live_report}");
    assert_eq!(live_report["summary"]["iteration_count"], 2);
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
    let credential = api(
        &http,
        &server,
        Some(token),
        reqwest::Method::POST,
        &format!("runners/{id}/credentials"),
        Some(json!({"workspace_id":"w","name":"Interrupt agent"})),
    )
    .await;
    let agent_token = credential["token"].as_str().unwrap();
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
        .env("MOLEAPI_AGENT_TOKEN", agent_token)
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

async fn query_dataset_agent_closes_source_after_invalidation(edit_workspace: bool) {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let source_url = format!("http://{}", listener.local_addr().unwrap());
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (closed_tx, closed_rx) = tokio::sync::oneshot::channel();
    let source_server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut bytes = [0u8; 4096];
        let mut request = Vec::new();
        loop {
            let count = socket.read(&mut bytes).await.unwrap();
            if count == 0 {
                return;
            }
            request.extend_from_slice(&bytes[..count]);
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/csv\r\nContent-Length: 1000000\r\n\r\nvalue\n").await.unwrap();
        let _ = started_tx.send(());
        loop {
            match socket.read(&mut bytes).await {
                Ok(0) | Err(_) => {
                    let _ = closed_tx.send(());
                    break;
                }
                Ok(_) => {}
            }
        }
    });
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let (endpoint, fixture) = serve(Router::new().route(
        "/verify",
        get(move || {
            let counter = counter.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                "unexpected"
            }
        }),
    ))
    .await;
    let temporary = tempfile::tempdir().unwrap();
    let hosted = moleapi_server::hosted_with_worker(
        moleapi_server::Config {
            database_url: format!(
                "sqlite://{}?mode=rwc",
                temporary.path().join("live-agent.db").display()
            ),
            setup_token: "live-agent-setup".into(),
            allow_registration: true,
            allow_private_network: true,
        },
        std::path::Path::new(env!("CARGO_BIN_EXE_moleapi-cli")),
    )
    .await
    .unwrap();
    let (server, service) = serve(hosted).await;
    let http = reqwest::Client::new();
    let registered = api(&http,&server,None,reqwest::Method::POST,"auth/register",Some(json!({"username":"liveagentowner","password":"fixture-password-123","setup_token":"live-agent-setup"}))).await;
    let owner = registered["token"].as_str().unwrap();
    let mut data = source(&endpoint)["data"].clone();
    data["collections"][0]["requests"]
        .as_array_mut()
        .unwrap()
        .truncate(1);
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = format!("{endpoint}/verify").into();
    request["extractions"] = json!([]);
    request["post_response_script"] = "".into();
    let mut query = request.clone();
    query["id"] = "live-query".into();
    query["url"] = format!("{source_url}/source").into();
    query["protocol"] = json!({"kind":"data","source":"remote_file","file_name":"live.csv","file_format":"csv","table_name":"data","sql":"SELECT value FROM data","read_only":true});
    query["timeout_ms"] = 30000.into();
    data["collections"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"sources","name":"Sources","description":"","requests":[query]}));
    data["datasets"] = json!([{"id":"live","name":"Live","query":{"request_id":"live-query"}}]);
    data["scenarios"] = json!([]);
    let workspace = api(
        &http,
        &server,
        Some(owner),
        reqwest::Method::POST,
        "workspaces",
        Some(json!({"id":"w","name":"Live","data":data})),
    )
    .await;
    let runner = api(
        &http,
        &server,
        Some(owner),
        reqwest::Method::POST,
        "runners",
        Some(json!({"name":"Live worker"})),
    )
    .await;
    let runner_id = runner["id"].as_str().unwrap();
    let credential = api(
        &http,
        &server,
        Some(owner),
        reqwest::Method::POST,
        &format!("runners/{runner_id}/credentials"),
        Some(json!({"workspace_id":"w","name":"Live credential"})),
    )
    .await;
    let task = api(&http,&server,Some(owner),reqwest::Method::POST,"workspaces/w/runner-tasks",Some(json!({"runner_id":runner_id,"expected_revision":workspace["revision"],"selection":{"collection_id":"c","dataset_id":"live"}}))).await;
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_moleapi-cli"))
        .args([
            "--server",
            &server,
            "--token-env",
            "MOLEAPI_AGENT_TOKEN",
            "runner",
            "agent",
            "--id",
            runner_id,
            "--once",
        ])
        .env("MOLEAPI_AGENT_TOKEN", credential["token"].as_str().unwrap())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(8), started_rx)
        .await
        .unwrap()
        .unwrap();
    if edit_workspace {
        let current = api(
            &http,
            &server,
            Some(owner),
            reqwest::Method::GET,
            "workspaces/w",
            None,
        )
        .await;
        let mut changed = current["data"].clone();
        changed["collections"][1]["requests"][0]["protocol"]["sql"] =
            "SELECT changed_value FROM data".into();
        let updated = api(
            &http,
            &server,
            Some(owner),
            reqwest::Method::PUT,
            "workspaces/w",
            Some(json!({"name":"Live","expected_revision":current["revision"],"data":changed})),
        )
        .await;
        assert_ne!(updated["revision"], current["revision"]);
    } else {
        let revoked = api(
            &http,
            &server,
            Some(owner),
            reqwest::Method::DELETE,
            &format!(
                "runners/{runner_id}/credentials/{}",
                credential["id"].as_str().unwrap()
            ),
            None,
        )
        .await;
        assert_eq!(revoked["revoked"], true);
    }
    tokio::time::timeout(Duration::from_secs(6), closed_rx)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    if !edit_workspace {
        let tasks = api(
            &http,
            &server,
            Some(owner),
            reqwest::Method::GET,
            "workspaces/w/runner-tasks",
            None,
        )
        .await;
        assert!(
            tasks
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value["id"] == task["id"] && value["status"] == "cancelled")
        );
    } else {
        let cancelled = api(
            &http,
            &server,
            Some(owner),
            reqwest::Method::POST,
            &format!(
                "workspaces/w/runner-tasks/{}/cancel",
                task["id"].as_str().unwrap()
            ),
            None,
        )
        .await;
        assert_eq!(cancelled["status"], "cancelled");
    }
    child.start_kill().unwrap();
    let _ = child.wait().await;
    source_server.abort();
    service.abort();
    fixture.abort();
}

#[tokio::test]
async fn revoked_query_dataset_agent_closes_its_live_source_before_next_run_step() {
    query_dataset_agent_closes_source_after_invalidation(false).await;
}

#[tokio::test]
async fn edited_query_dataset_agent_closes_its_old_live_source_before_next_run_step() {
    query_dataset_agent_closes_source_after_invalidation(true).await;
}
