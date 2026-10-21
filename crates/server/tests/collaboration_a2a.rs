mod common;
#[path = "fixtures/a2a/legacy.rs"]
mod fixture;
#[path = "fixtures/a2a/server.rs"]
mod modern;
use axum::response::IntoResponse;
use common::*;
use std::time::Duration;

struct Shared {
    _dir: tempfile::TempDir,
    router: Router,
    owner: String,
    editor: String,
    other: String,
    viewer: String,
    foreign: String,
    base: String,
    saved: Value,
}
async fn setup(url: &str) -> Shared {
    setup_with(url, "0.3", "jsonrpc").await
}
async fn setup_with(url: &str, dialect: &str, transport: &str) -> Shared {
    let dir = tempfile::tempdir().unwrap();
    let mut settings = config(
        moleapi_server::sqlite_database_url(&dir.path().join("a2a.db")).unwrap(),
        true,
    );
    settings.allow_private_network = true;
    let router = hosted(settings).await.unwrap();
    let owner = register(&router, "a2aowner").await;
    let editor = register(&router, "a2aeditor").await;
    let other = register(&router, "a2aother").await;
    let viewer = register(&router, "a2aviewer").await;
    let foreign = register(&router, "a2aforeign").await;
    let mut data = example_data();
    data["script_mode"] = "v2".into();
    data["collections"][0]["auth"] =
        json!({"kind":"bearer","token":"folder-private","username":"","password":""});
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = url.into();
    request["examples"] = json!([]);
    request["protocol"] = json!({"kind":"a2a","dialect":dialect,"transport":transport,"operation":"message/send","params_source":"{unfinished"});
    request["pre_request_script"] =
        "pm.request.headers.add({key:'X-A2a-Owner',value:'owner-script'});".into();
    request["auth"] =
        json!({"kind":"bearer","token":"{{fixture_token}}","username":"","password":""});
    request["timeout_ms"] = json!(60000);
    data["global_variables"] = json!([{"id":"v","key":"message","value":"owner-value","enabled":true},{"id":"s","key":"fixture_token","value":"fixture-token","enabled":true,"secret":true}]);
    data["global_variables"].as_array_mut().unwrap().push(
        json!({"id":"n","key":"numeric_private","value":"987654","enabled":true,"secret":true}),
    );
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"A2A","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let mut collision = data;
    collision["global_variables"][0]["value"] = "wrong-member".into();
    collision["global_variables"][1]["value"] = "wrong-token".into();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            Some(&editor),
            Some(json!({"id":"w","name":"Collision","data":collision}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let mut base = String::new();
    for (token, role) in [(&editor, "editor"), (&other, "editor"), (&viewer, "viewer")] {
        let (_, invite) = call(
            &router,
            "POST",
            "/api/workspaces/w/invitations",
            Some(&owner),
            Some(json!({"role":role,"expires_in_days":1})),
        )
        .await;
        let (status, accepted) = call(
            &router,
            "POST",
            "/api/workspace-invitations/accept",
            Some(token),
            Some(json!({"token":invite["token"]})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{accepted}");
        base = format!(
            "/api/owners/{}/workspaces/w",
            accepted["owner_id"].as_str().unwrap()
        );
    }
    Shared {
        _dir: dir,
        router,
        owner,
        editor,
        other,
        viewer,
        foreign,
        base,
        saved,
    }
}
async fn create(shared: &Shared) -> (StatusCode, Value) {
    call(
        &shared.router,
        "POST",
        &format!("{}/sessions", shared.base),
        Some(&shared.editor),
        Some(json!({"request_id":"r","expected_revision":shared.saved["revision"]})),
    )
    .await
}
async fn until(shared: &Shared, path: &str, predicate: impl Fn(&Value) -> bool) -> Value {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let (status, value) =
                call(&shared.router, "GET", path, Some(&shared.editor), None).await;
            assert_eq!(status, StatusCode::OK, "{value}");
            if predicate(&value) {
                break value;
            }
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
    })
    .await
    .unwrap()
}
async fn send(shared: &Shared, path: &str, message: Value) {
    let (status, value) = call(
        &shared.router,
        "POST",
        &format!("{path}/send"),
        Some(&shared.editor),
        Some(message),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
}
async fn event(shared: &Shared, path: &str, predicate: impl Fn(&Value) -> bool) -> Value {
    let batch = until(shared, &format!("{path}/events"), |b| {
        b["events"].as_array().unwrap().iter().any(&predicate)
    })
    .await;
    batch["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| predicate(e))
        .unwrap()
        .clone()
}
#[tokio::test]
async fn shared_a2a_routes_and_saved_creator_sessions_are_admitted() {
    let shared = setup("http://127.0.0.1:1/rpc").await;
    let (status, value) = create(&shared).await;
    assert_eq!(status, StatusCode::OK, "{value}");
}
#[tokio::test]
async fn shared_a2a_card_import_is_a_selected_saved_request_candidate() {
    let shared = setup("http://127.0.0.1:1/rpc").await;
    let input = json!({"request_id":"r","expected_revision":shared.saved["revision"],"source":card("http://127.0.0.1:1/rpc")});
    let (status, value) = call(
        &shared.router,
        "POST",
        &format!("{}/a2a/cards/import", shared.base),
        Some(&shared.editor),
        Some(input),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(value["specification"]["kind"], "a2a-agent-card");
}
fn card(url: &str) -> String {
    json!({"name":"Shared agent","description":"Fixture","version":"1","protocolVersion":"0.3.0","url":url,"capabilities":{"streaming":true},"defaultInputModes":["text/plain"],"defaultOutputModes":["text/plain"],"skills":[]}).to_string()
}

#[tokio::test]
async fn member_a2a_admission_is_saved_revision_creator_bound_and_rejects_other_commands() {
    let shared = setup("http://127.0.0.1:1/rpc").await;
    let input = json!({"request_id":"r","expected_revision":shared.saved["revision"]});
    assert_eq!(
        call(
            &shared.router,
            "POST",
            &format!("{}/sessions", shared.base),
            Some(&shared.viewer),
            Some(input)
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &shared.router,
            "POST",
            &format!("{}/sessions", shared.base),
            Some(&shared.editor),
            Some(json!({"request_id":"r","expected_revision":0}))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(
            &shared.router,
            "POST",
            &format!("{}/sessions", shared.base),
            Some(&shared.foreign),
            Some(json!({"request_id":"r","expected_revision":shared.saved["revision"]}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, opened) = create(&shared).await;
    assert_eq!(status, StatusCode::OK, "{opened}");
    let path = format!(
        "{}/sessions/{}",
        shared.base,
        opened["id"].as_str().unwrap()
    );
    for token in [&shared.other, &shared.viewer, &shared.foreign] {
        assert_eq!(
            call(&shared.router, "GET", &path, Some(token), None)
                .await
                .0,
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        call(
            &shared.router,
            "POST",
            &format!("{path}/send"),
            Some(&shared.editor),
            Some(json!({"kind":"text","text":"not-a2a"}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &shared.router,
            "POST",
            &format!("{path}/close"),
            Some(&shared.editor),
            Some(json!({}))
        )
        .await
        .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn shared_card_candidates_preserve_original_and_reject_role_stale_foreign_scope_and_private_data()
 {
    let shared = setup("http://127.0.0.1:1").await;
    let path = format!("{}/a2a/cards/import", shared.base);
    let mut raw: Value = serde_json::from_str(&card("http://127.0.0.1:1")).unwrap();
    raw["vendor"] = json!({"unknown":[true,173]});
    let source = serde_json::to_string_pretty(&raw).unwrap();
    let input =
        json!({"request_id":"r","expected_revision":shared.saved["revision"],"source":source});
    for (token, status) in [
        (&shared.viewer, StatusCode::FORBIDDEN),
        (&shared.foreign, StatusCode::NOT_FOUND),
    ] {
        assert_eq!(
            call(
                &shared.router,
                "POST",
                &path,
                Some(token),
                Some(input.clone())
            )
            .await
            .0,
            status
        );
    }
    let mut stale = input.clone();
    stale["expected_revision"] = json!(0);
    assert_eq!(
        call(
            &shared.router,
            "POST",
            &path,
            Some(&shared.editor),
            Some(stale)
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let mut override_input = input.clone();
    override_input["locals"] = json!([]);
    assert_eq!(
        call(
            &shared.router,
            "POST",
            &path,
            Some(&shared.editor),
            Some(override_input)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (status, value) = call(
        &shared.router,
        "POST",
        &path,
        Some(&shared.editor),
        Some(input.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(value["specification"]["source"], source);
    let (_, before) = call(
        &shared.router,
        "GET",
        "/api/workspaces/w",
        Some(&shared.owner),
        None,
    )
    .await;
    assert_eq!(before["data"], shared.saved["data"]);
    assert_eq!(before["revision"], shared.saved["revision"]);
    for private in [
        json!("fixture-token"),
        json!("folder-private"),
        json!(987654),
        json!({"fixture-token":"safe"}),
        json!("http://agent.test/path?credential=value"),
    ] {
        let mut bad = raw.clone();
        bad["vendor"] = private;
        let mut input = input.clone();
        input["source"] = json!(bad.to_string());
        let (status, body) = call(
            &shared.router,
            "POST",
            &path,
            Some(&shared.editor),
            Some(input),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert!(!body.to_string().contains("fixture-token"));
    }
    let mut data = shared.saved["data"].clone();
    data["specifications"] = json!([value["specification"].clone()]);
    data["collections"][0]["requests"][0]["specification_id"] =
        value["specification"]["id"].clone();
    data["collections"][0]["requests"][0]["protocol"]["card_source"] = json!(source);
    let (status, saved) = call(
        &shared.router,
        "PUT",
        &shared.base,
        Some(&shared.editor),
        Some(json!({"name":"A2A","expected_revision":before["revision"],"data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let mut read = json!({"request_id":"r","expected_revision":saved["revision"],"specification_id":value["specification"]["id"]});
    let (status, restored) = call(
        &shared.router,
        "POST",
        &format!("{}/a2a/cards/read", shared.base),
        Some(&shared.editor),
        Some(read.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{restored}");
    assert_eq!(restored["specification"], value["specification"]);
    read["specification_id"] = json!("unlinked");
    assert_eq!(
        call(
            &shared.router,
            "POST",
            &format!("{}/a2a/cards/read", shared.base),
            Some(&shared.editor),
            Some(read)
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}
async fn invoke(
    shared: &Shared,
    path: &str,
    ticket: &str,
    method: &str,
    params: Value,
    kind: &str,
) -> Value {
    send(shared,path,json!({"kind":"a2a_request","request_id":ticket,"method":method,"params_source":params.to_string()})).await;
    event(shared, path, |e| {
        e["message"]["kind"] == kind && e["message"]["request_id"] == ticket
    })
    .await["message"]
        .clone()
}
#[tokio::test]
async fn real_member_sdk_versions_transports_card_auth_owner_variables_stream_tasks_and_audits() {
    for (dialect, transport) in [("0.3", "jsonrpc"), ("1.0", "jsonrpc"), ("1.0", "http-json")] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let app = if dialect == "0.3" {
            fixture::router(&url)
        } else {
            modern::router(&url)
        }
        .layer(axum::middleware::from_fn(
            |request: axum::extract::Request, next: axum::middleware::Next| async move {
                if request
                    .headers()
                    .get("authorization")
                    .and_then(|h| h.to_str().ok())
                    != Some("Bearer fixture-token")
                    || request
                        .headers()
                        .get("x-a2a-owner")
                        .and_then(|h| h.to_str().ok())
                        != Some("owner-script")
                {
                    return StatusCode::UNAUTHORIZED.into_response();
                }
                next.run(request).await
            },
        ));
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let mut shared = setup_with(&url, dialect, transport).await;
        let(status,candidate)=call(&shared.router,"POST",&format!("{}/a2a/cards/discover",shared.base),Some(&shared.editor),Some(json!({"request_id":"r","expected_revision":shared.saved["revision"],"discovery_id":"card","url":url}))).await;
        assert_eq!(status, StatusCode::OK, "{candidate}");
        assert_eq!(candidate["specification"]["kind"], "a2a-agent-card");
        let mut data = shared.saved["data"].clone();
        data["specifications"] = json!([candidate["specification"].clone()]);
        data["collections"][0]["requests"][0]["specification_id"] =
            candidate["specification"]["id"].clone();
        data["collections"][0]["requests"][0]["protocol"]["card_source"] =
            candidate["specification"]["source"].clone();
        let(status,saved)=call(&shared.router,"PUT",&shared.base,Some(&shared.editor),Some(json!({"name":shared.saved["name"],"expected_revision":shared.saved["revision"],"data":data}))).await;
        assert_eq!(status, StatusCode::OK, "{saved}");
        shared.saved = saved;
        let (status, opened) = create(&shared).await;
        assert_eq!(status, StatusCode::OK, "{opened}");
        let path = format!(
            "{}/sessions/{}",
            shared.base,
            opened["id"].as_str().unwrap()
        );
        event(&shared, &path, |e| e["message"]["kind"] == "a2a_ready").await;
        let params = if dialect == "0.3" {
            json!({"message":{"kind":"message","messageId":"input","role":"user","contextId":"{{message}}","parts":[{"kind":"text","text":"shared message"}]}})
        } else {
            json!({"message":{"messageId":"first","role":"ROLE_USER","contextId":"{{message}}","parts":[{"text":"shared message"}]},"configuration":{"historyLength":20}})
        };
        let response = invoke(
            &shared,
            &path,
            "send",
            "message/send",
            params.clone(),
            "a2a_result",
        )
        .await;
        let task = if dialect == "0.3" {
            &response["result"]
        } else {
            &response["result"]["task"]
        };
        assert_eq!(task["contextId"], "owner-value", "{response}");
        let task_id = task["id"].clone();
        let get = invoke(
            &shared,
            &path,
            "get",
            "tasks/get",
            json!({"id":task_id,"historyLength":20}),
            "a2a_result",
        )
        .await;
        assert_eq!(get["result"]["id"], task_id);
        assert!(!get["result"]["history"].as_array().unwrap().is_empty());
        invoke(
            &shared,
            &path,
            "stream",
            "message/stream",
            params,
            "a2a_stream",
        )
        .await;
        event(&shared, &path, |e| {
            e["message"]["kind"] == "a2a_finished" && e["message"]["request_id"] == "stream"
        })
        .await;
        if dialect == "0.3" {
            let cancelled = invoke(
                &shared,
                &path,
                "cancel",
                "tasks/cancel",
                json!({"id":task_id}),
                "a2a_result",
            )
            .await;
            assert_eq!(cancelled["result"]["status"]["state"], "canceled");
            invoke(
                &shared,
                &path,
                "resume",
                "tasks/resubscribe",
                json!({"id":task_id}),
                "a2a_stream",
            )
            .await;
            send(&shared,&path,json!({"kind":"a2a_request","request_id":"wait","method":"message/send","params_source":json!({"message":{"kind":"message","messageId":"wait","role":"user","parts":[{"kind":"text","text":"wait"}]}}).to_string()})).await;
            send(
                &shared,
                &path,
                json!({"kind":"a2a_stop","request_id":"wait"}),
            )
            .await;
            let stopped = event(&shared, &path, |e| {
                e["message"]["kind"] == "a2a_error" && e["message"]["request_id"] == "wait"
            })
            .await;
            assert!(stopped.to_string().contains("stopped locally"));
        }
        if dialect == "1.0" {
            let context = task["contextId"].clone();
            let list = invoke(
                &shared,
                &path,
                "list",
                "tasks/list",
                json!({"contextId":context,"pageSize":5,"historyLength":10}),
                "a2a_result",
            )
            .await;
            assert!(
                list["result"]["tasks"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|t| t["id"] == task_id)
            );
            let config=invoke(&shared,&path,"push-set","tasks/pushNotificationConfig/set",json!({"taskId":task_id,"id":"shared-config","url":"http://127.0.0.1:1/explicit-callback"}),"a2a_result").await;
            let config_id = config["result"]["id"].clone();
            assert_eq!(
                config["result"]["url"],
                "http://127.0.0.1:1/explicit-callback"
            );
            let get = invoke(
                &shared,
                &path,
                "push-get",
                "tasks/pushNotificationConfig/get",
                json!({"taskId":task_id,"id":config_id}),
                "a2a_result",
            )
            .await;
            assert_eq!(get["result"]["id"], config_id);
            let list = invoke(
                &shared,
                &path,
                "push-list",
                "tasks/pushNotificationConfig/list",
                json!({"taskId":task_id,"pageSize":10}),
                "a2a_result",
            )
            .await;
            assert_eq!(list["result"]["configs"].as_array().unwrap().len(), 1);
            invoke(
                &shared,
                &path,
                "push-delete",
                "tasks/pushNotificationConfig/delete",
                json!({"taskId":task_id,"id":config_id}),
                "a2a_result",
            )
            .await;
            let error = invoke(
                &shared,
                &path,
                "missing",
                "tasks/get",
                json!({"id":"missing"}),
                "a2a_error",
            )
            .await;
            assert!(error.to_string().contains("not found"), "{error}");
        }
        for token in [&shared.other, &shared.viewer, &shared.foreign] {
            assert_eq!(
                call(
                    &shared.router,
                    "POST",
                    &format!("{path}/send"),
                    Some(token),
                    Some(json!({"kind":"a2a_stop","request_id":"wait"}))
                )
                .await
                .0,
                StatusCode::NOT_FOUND
            );
        }
        let (_, canonical) = call(
            &shared.router,
            "GET",
            "/api/workspaces/w",
            Some(&shared.owner),
            None,
        )
        .await;
        assert_eq!(canonical["data"], shared.saved["data"]);
        assert_eq!(canonical["revision"], shared.saved["revision"]);
        let (_, audit) = call(
            &shared.router,
            "GET",
            &format!("{}/audit", shared.base),
            Some(&shared.owner),
            None,
        )
        .await;
        assert!(audit.to_string().contains("protocol.a2a.request"));
        assert!(
            !audit.to_string().contains("owner-value")
                && !audit.to_string().contains("fixture-token")
                && !audit.to_string().contains("message/send")
        );
        call(&shared.router, "DELETE", &path, Some(&shared.editor), None).await;
        server.abort();
    }
}

#[tokio::test]
async fn shared_a2a_pending_discovery_drops_socket_after_actor_stop_role_revision_or_logout() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    for operation in ["discovery"] {
        for boundary in ["stop", "role", "revision", "logout"] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
            let (closed_tx, closed_rx) = tokio::sync::oneshot::channel();
            let peer = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut header = Vec::new();
                let mut b = [0];
                while !header.ends_with(b"\r\n\r\n") {
                    assert_eq!(socket.read(&mut b).await.unwrap(), 1);
                    header.push(b[0]);
                }
                let wire = String::from_utf8_lossy(&header).to_ascii_lowercase();
                assert!(wire.contains("x-a2a-owner: owner-script"));
                assert!(wire.contains("authorization: bearer fixture-token"));
                // Consume an optional request body before observing client EOF.
                let length = String::from_utf8_lossy(&header)
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|v| v.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                let mut body = vec![0; length];
                socket.read_exact(&mut body).await.unwrap();
                socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 65535\r\n\r\n").await.unwrap();
                ready_tx.send(()).unwrap();
                let result = socket.read(&mut b).await;
                let _ = closed_tx.send(result);
            });
            let shared = setup(&format!("http://{address}/")).await;
            let url = format!("{}/a2a/cards/discover", shared.base);
            let input = json!({"request_id":"r","expected_revision":shared.saved["revision"],"discovery_id":"held","url":format!("http://{address}/")});
            let router = shared.router.clone();
            let actor = shared.editor.clone();
            let work = tokio::spawn(async move {
                call(&router, "POST", &url, Some(&actor), Some(input)).await
            });
            tokio::time::timeout(Duration::from_secs(3), ready_rx)
                .await
                .unwrap()
                .unwrap();
            // A different editor with the same discovery id cannot stop this actor's fetch.
            let stop = json!({"request_id":"r","expected_revision":shared.saved["revision"],"discovery_id":"held"});
            assert_eq!(
                call(
                    &shared.router,
                    "POST",
                    &format!("{}/a2a/cards/discover/cancel", shared.base),
                    Some(&shared.other),
                    Some(stop.clone())
                )
                .await
                .0,
                StatusCode::OK
            );
            assert!(!work.is_finished());
            let expected = match boundary {
                "stop" => {
                    assert_eq!(
                        call(
                            &shared.router,
                            "POST",
                            &format!("{}/a2a/cards/discover/cancel", shared.base),
                            Some(&shared.editor),
                            Some(stop)
                        )
                        .await
                        .0,
                        StatusCode::OK
                    );
                    StatusCode::BAD_REQUEST
                }

                "role" => {
                    let (_, members) = call(
                        &shared.router,
                        "GET",
                        &format!("{}/members", shared.base),
                        Some(&shared.owner),
                        None,
                    )
                    .await;
                    let member = members
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|m| m["username"] == "a2aeditor")
                        .unwrap();
                    assert_eq!(
                        call(
                            &shared.router,
                            "PATCH",
                            &format!(
                                "{}/members/{}",
                                shared.base,
                                member["member_id"].as_str().unwrap()
                            ),
                            Some(&shared.owner),
                            Some(json!({"role":"viewer","expected_revision":member["revision"]}))
                        )
                        .await
                        .0,
                        StatusCode::OK
                    );
                    StatusCode::NOT_FOUND
                }
                "revision" => {
                    let(status,value)=call(&shared.router,"PUT",&shared.base,Some(&shared.editor),Some(json!({"name":shared.saved["name"],"data":shared.saved["data"],"expected_revision":shared.saved["revision"]}))).await;
                    assert_eq!(status, StatusCode::OK, "{value}");
                    StatusCode::CONFLICT
                }
                _ => {
                    assert_eq!(
                        call(
                            &shared.router,
                            "POST",
                            "/api/auth/logout",
                            Some(&shared.editor),
                            None
                        )
                        .await
                        .0,
                        StatusCode::OK
                    );
                    StatusCode::UNAUTHORIZED
                }
            };
            let (status, value) = tokio::time::timeout(Duration::from_secs(2), work)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(status, expected, "{operation}/{boundary}: {value}");
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(2), closed_rx)
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap(),
                0,
                "{operation}/{boundary}"
            );
            peer.await.unwrap();
        }
    }
}
