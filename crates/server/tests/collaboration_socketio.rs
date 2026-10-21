mod common;
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
    let dir = tempfile::tempdir().unwrap();
    let mut settings = config(
        moleapi_server::sqlite_database_url(&dir.path().join("socketio.db")).unwrap(),
        true,
    );
    settings.allow_private_network = true;
    let router = hosted(settings).await.unwrap();
    let owner = register(&router, "socketowner").await;
    let editor = register(&router, "socketeditor").await;
    let other = register(&router, "socketother").await;
    let viewer = register(&router, "socketviewer").await;
    let foreign = register(&router, "socketforeign").await;
    let mut data = example_data();
    data["script_mode"] = "v2".into();
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = url.into();
    request["examples"] = json!([]);
    request["protocol"] = json!({"kind":"socketio","namespace":"/fixture","path":"/custom/socket.io/","auth_source":"{\"token\":\"{{fixture_token}}\"}","listeners":["echo","question","server-ack-result","private"],"event":"echo","arguments_source":"[\"saved draft\"]"});
    request["query"] = json!([{"id":"q","key":"required","value":"yes","enabled":true}]);
    request["pre_request_script"] = "pm.request.headers.add({key:'X-Fixture',value:'yes'});".into();
    data["global_variables"] = json!([{"id":"v","key":"message","value":"owner-value","enabled":true},{"id":"s","key":"fixture_token","value":"fixture-token","enabled":true,"secret":true}]);
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Socket.IO","data":data})),
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
async fn member_socketio_admission_is_saved_revision_creator_bound_and_rejects_other_commands() {
    let shared = setup("ws://127.0.0.1:1").await;
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
            Some(json!({"kind":"text","text":"not-socketio"}))
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
#[ignore = "official Socket.IO4 fixture on18886: see protocols/tests/fixtures/socketio/README.md"]
async fn real_member_socketio_binary_acks_listeners_canonical_source_privacy_and_revocation() {
    let mut shared = setup("ws://127.0.0.1:18886").await;
    let (status, opened) = create(&shared).await;
    assert_eq!(status, StatusCode::OK, "{opened}");
    let path = format!(
        "{}/sessions/{}",
        shared.base,
        opened["id"].as_str().unwrap()
    );
    until(&shared, &path, |s| s["state"] == "open").await;
    send(&shared,&path,json!({"kind":"socketio_emit","event":"echo","arguments_source":"[\"{{message}}\",{\"_placeholder\":true,\"num\":0}]","attachments_base64":["AP8H"],"ack_id":"first-ack","ack_timeout_ms":1000})).await;
    let echo = event(&shared, &path, |e| {
        e["direction"] == "incoming" && e["message"]["event"] == "echo"
    })
    .await;
    assert_eq!(echo["message"]["arguments"][0], "owner-value");
    assert_eq!(echo["message"]["attachments_base64"], json!(["AP8H"]));
    let ack = event(&shared, &path, |e| {
        e["message"]["kind"] == "socketio_ack" && e["message"]["ack_id"] == "first-ack"
    })
    .await;
    assert_eq!(ack["message"]["status"], "ok");
    assert_eq!(ack["message"]["attachments_base64"], json!(["AP8H"]));
    send(
        &shared,
        &path,
        json!({"kind":"socketio_listen","event":"open","enabled":true}),
    )
    .await;
    send(
        &shared,
        &path,
        json!({"kind":"socketio_emit","event":"open","arguments_source":"[\"ordinary-open\"]"}),
    )
    .await;
    event(&shared, &path, |e| {
        e["direction"] == "incoming" && e["message"]["event"] == "open"
    })
    .await;
    send(
        &shared,
        &path,
        json!({"kind":"socketio_listen","event":"open","enabled":false}),
    )
    .await;
    send(&shared,&path,json!({"kind":"socketio_emit","event":"ack-timeout","arguments_source":"[]","ack_id":"timeout","ack_timeout_ms":100})).await;
    event(&shared, &path, |e| {
        e["message"]["ack_id"] == "timeout" && e["message"]["status"] == "timeout"
    })
    .await;
    send(
        &shared,
        &path,
        json!({"kind":"socketio_emit","event":"server-ack","arguments_source":"[]"}),
    )
    .await;
    let question = event(&shared, &path, |e| e["message"]["event"] == "question").await;
    let callback = question["message"]["ack_id"].as_str().unwrap();
    let reply = json!({"kind":"socketio_ack","ack_id":callback,"arguments_source":"[\"answered\",{\"_placeholder\":true,\"num\":0}]","attachments_base64":["/wA="]});
    assert_eq!(
        call(
            &shared.router,
            "POST",
            &format!("{path}/send"),
            Some(&shared.other),
            Some(reply.clone())
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    send(&shared, &path, reply.clone()).await;
    let result = event(&shared, &path, |e| {
        e["message"]["event"] == "server-ack-result"
    })
    .await;
    assert_eq!(result["message"]["arguments"][1], "answered");
    assert_eq!(result["message"]["attachments_base64"], json!(["/wA="]));
    assert_eq!(
        call(
            &shared.router,
            "POST",
            &format!("{path}/send"),
            Some(&shared.editor),
            Some(reply)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    send(&shared,&path,json!({"kind":"socketio_emit","event":"private","arguments_source":"[\"{{fixture_token}}\"]"})).await;
    let private = event(&shared, &path, |e| {
        e["direction"] == "incoming" && e["message"]["event"] == "private"
    })
    .await;
    assert!(!private.to_string().contains("fixture-token"));
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
    for action in [
        "protocol.socketio.emit",
        "protocol.socketio.listen",
        "protocol.socketio.ack",
    ] {
        assert!(
            audit
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["action"] == action)
        );
    }
    assert!(!audit.to_string().contains("owner-value"));
    assert!(!audit.to_string().contains(callback));
    assert!(!audit.to_string().contains("AP8H"));
    let (status,updated)=call(&shared.router,"PUT",&shared.base,Some(&shared.editor),Some(json!({"name":"Revised","data":shared.saved["data"],"expected_revision":shared.saved["revision"]}))).await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert!(matches!(
        call(&shared.router, "GET", &path, Some(&shared.editor), None)
            .await
            .0,
        StatusCode::CONFLICT | StatusCode::NOT_FOUND
    ));
    shared.saved = updated;
    let (status, opened) = create(&shared).await;
    assert_eq!(status, StatusCode::OK, "{opened}");
    let second = format!(
        "{}/sessions/{}",
        shared.base,
        opened["id"].as_str().unwrap()
    );
    until(&shared, &second, |s| s["state"] == "open").await;
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
    assert_eq!(
        call(&shared.router, "GET", &second, Some(&shared.editor), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}
