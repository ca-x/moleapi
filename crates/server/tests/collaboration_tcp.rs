mod common;
use common::*;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn until(
    router: &Router,
    path: &str,
    token: &str,
    predicate: impl Fn(&Value) -> bool,
) -> Value {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let (status, value) = call(router, "GET", path, Some(token), None).await;
            assert_eq!(status, StatusCode::OK, "{value}");
            if predicate(&value) {
                return value;
            }
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
    })
    .await
    .unwrap()
}
#[tokio::test]
async fn shared_tcp_uses_saved_owner_snapshot_codecs_creator_half_close_and_role_fences() {
    let dir = tempfile::tempdir().unwrap();
    let mut settings = config(
        moleapi_server::sqlite_database_url(&dir.path().join("tcp.db")).unwrap(),
        true,
    );
    settings.allow_private_network = true;
    let router = hosted(settings).await.unwrap();
    let owner = register(&router, "tcpowner").await;
    let editor = register(&router, "tcpeditor").await;
    let other = register(&router, "tcpother").await;
    let viewer = register(&router, "tcpviewer").await;
    let foreign = register(&router, "tcpforeign").await;
    let mut source = example_data();
    source["collections"][0]["requests"][0]["protocol"] = json!({"kind":"tcp"});
    source["collections"][0]["requests"][0]["url"] = "tcp://127.0.0.1:1".into();
    source["collections"][0]["requests"][0]["examples"] = json!([]);
    source["global_variables"] =
        json!([{"id":"place","key":"place","value":"owner-value","enabled":true}]);
    let (status, mut saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"TCP","data":source})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let mut collision = source.clone();
    collision["global_variables"][0]["value"] = "wrong-member".into();
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
    let mut editor_id = Value::Null;
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
        if token == &editor {
            editor_id = accepted["member_id"].clone();
        }
    }
    for framing in ["raw", "lines", "length_be", "length_le"] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("tcp://{}", listener.local_addr().unwrap());
        let raw = framing == "raw";
        let fixture = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut chunk = [0u8; 128];
            loop {
                let n = socket.read(&mut chunk).await.unwrap();
                if n == 0 {
                    if raw {
                        socket.write_all(b"after EOF").await.unwrap();
                    }
                    break;
                }
                socket.write_all(&chunk[..n]).await.unwrap();
            }
        });
        saved["data"]["collections"][0]["requests"][0]["url"] = url.into();
        saved["data"]["collections"][0]["requests"][0]["protocol"] = json!({"kind":"tcp","framing":framing,"message":{"encoding":"text","payload_source":"saved template","secret":false}});
        let (status,updated)=call(&router,"PUT",&base,Some(&editor),Some(json!({"name":saved["name"],"data":saved["data"],"expected_revision":saved["revision"]}))).await;
        assert_eq!(status, StatusCode::OK, "{updated}");
        saved = updated;
        let input = json!({"request_id":"r","expected_revision":saved["revision"]});
        for (token, expected) in [
            (&viewer, StatusCode::FORBIDDEN),
            (&foreign, StatusCode::NOT_FOUND),
        ] {
            assert_eq!(
                call(
                    &router,
                    "POST",
                    &format!("{base}/sessions"),
                    Some(token),
                    Some(input.clone())
                )
                .await
                .0,
                expected
            );
        }
        assert_eq!(
            call(
                &router,
                "POST",
                &format!("{base}/sessions"),
                Some(&editor),
                Some(json!({"request_id":"r","expected_revision":0}))
            )
            .await
            .0,
            StatusCode::CONFLICT
        );
        let (status, opened) = call(
            &router,
            "POST",
            &format!("{base}/sessions"),
            Some(&editor),
            Some(input),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{opened}");
        let path = format!("{base}/sessions/{}", opened["id"].as_str().unwrap());
        until(&router, &path, &editor, |s| s["state"] == "open").await;
        for token in [&other, &viewer, &foreign] {
            assert_eq!(
                call(
                    &router,
                    "POST",
                    &format!("{path}/send"),
                    Some(token),
                    Some(json!({"kind":"tcp_half_close"}))
                )
                .await
                .0,
                StatusCode::NOT_FOUND
            );
        }
        assert_eq!(
            call(
                &router,
                "POST",
                &format!("{path}/send"),
                Some(&editor),
                Some(json!({"kind":"text","text":"not-tcp"}))
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
        let (status,response)=call(&router,"POST",&format!("{path}/send"),Some(&editor),Some(json!({"kind":"tcp_send","message":{"encoding":"text","payload_source":"{{place}}","secret":false}}))).await;
        assert_eq!(status, StatusCode::OK, "{response}");
        let events = until(&router, &format!("{path}/events"), &editor, |b| {
            b["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["direction"] == "incoming" && e["message"]["text"] == "owner-value")
        })
        .await;
        assert!(!events.to_string().contains("wrong-member"));
        let (_, canonical) = call(&router, "GET", "/api/workspaces/w", Some(&owner), None).await;
        assert_eq!(canonical["data"], saved["data"]);
        assert_eq!(canonical["revision"], saved["revision"]);
        if raw {
            assert_eq!(
                call(
                    &router,
                    "POST",
                    &format!("{path}/send"),
                    Some(&editor),
                    Some(json!({"kind":"tcp_half_close"}))
                )
                .await
                .0,
                StatusCode::OK
            );
            until(&router, &format!("{path}/events"), &editor, |b| {
                b["events"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|e| e["message"]["text"] == "after EOF")
            })
            .await;
            assert_eq!(call(&router,"POST",&format!("{path}/send"),Some(&editor),Some(json!({"kind":"tcp_send","message":{"encoding":"text","payload_source":"late"}}))).await.0,StatusCode::BAD_REQUEST);
        } else if framing == "length_le" {
            assert_eq!(
                call(
                    &router,
                    "PATCH",
                    &format!("{base}/members/{}", editor_id.as_str().unwrap()),
                    Some(&owner),
                    Some(json!({"role":"viewer","expected_revision":1}))
                )
                .await
                .0,
                StatusCode::OK
            );
            tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    if call(&router, "GET", &path, Some(&editor), None).await.0
                        == StatusCode::NOT_FOUND
                    {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            })
            .await
            .unwrap();
        } else {
            assert_eq!(
                call(
                    &router,
                    "POST",
                    &format!("{path}/close"),
                    Some(&editor),
                    Some(json!({}))
                )
                .await
                .0,
                StatusCode::OK
            );
        }
        tokio::time::timeout(Duration::from_secs(5), fixture)
            .await
            .unwrap()
            .unwrap();
    }
    let (_, audit) = call(&router, "GET", &format!("{base}/audit"), Some(&owner), None).await;
    assert!(
        audit
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["action"] == "protocol.tcp.send")
    );
    assert!(
        audit
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["action"] == "protocol.tcp.half_close")
    );
    assert!(!audit.to_string().contains("owner-value"));
    assert!(!audit.to_string().contains("{{place}}"));
}

#[tokio::test]
async fn shared_tcp_private_payloads_and_revision_logout_close_the_captured_connection() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("tcp://{}", listener.local_addr().unwrap());
    let (closed, mut observed) = tokio::sync::mpsc::channel(2);
    let fixture = tokio::spawn(async move {
        let mut peers = Vec::new();
        for _ in 0..2 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let closed = closed.clone();
            peers.push(tokio::spawn(async move {
                let mut bytes = [0u8; 128];
                loop {
                    let n = socket.read(&mut bytes).await.unwrap();
                    if n == 0 {
                        break;
                    }
                    socket.write_all(&bytes[..n]).await.unwrap();
                }
                closed.send(()).await.unwrap();
            }));
        }
        for peer in peers {
            peer.await.unwrap();
        }
    });
    let dir = tempfile::tempdir().unwrap();
    let mut settings = config(
        moleapi_server::sqlite_database_url(&dir.path().join("private.db")).unwrap(),
        true,
    );
    settings.allow_private_network = true;
    let router = hosted(settings).await.unwrap();
    let owner = register(&router, "privateowner").await;
    let editor = register(&router, "privateeditor").await;
    let mut data = example_data();
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = url.into();
    request["protocol"] = json!({"kind":"tcp"});
    request["examples"] = json!([]);
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Private TCP","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (_, invite) = call(
        &router,
        "POST",
        "/api/workspaces/w/invitations",
        Some(&owner),
        Some(json!({"role":"editor","expires_in_days":1})),
    )
    .await;
    let (_, accepted) = call(
        &router,
        "POST",
        "/api/workspace-invitations/accept",
        Some(&editor),
        Some(json!({"token":invite["token"]})),
    )
    .await;
    let base = format!(
        "/api/owners/{}/workspaces/w",
        accepted["owner_id"].as_str().unwrap()
    );
    let (status, opened) = call(
        &router,
        "POST",
        &format!("{base}/sessions"),
        Some(&editor),
        Some(json!({"request_id":"r","expected_revision":saved["revision"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{opened}");
    let path = format!("{base}/sessions/{}", opened["id"].as_str().unwrap());
    until(&router, &path, &editor, |s| s["state"] == "open").await;
    assert_eq!(call(&router,"POST",&format!("{path}/send"),Some(&editor),Some(json!({"kind":"tcp_send","message":{"encoding":"hex","payload_source":"00 01 ff 41","secret":false}}))).await.0,StatusCode::OK);
    until(&router, &format!("{path}/events"), &editor, |b| {
        b["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["direction"] == "incoming" && e["message"]["base64"] == "AAH/QQ==")
    })
    .await;
    assert_eq!(call(&router,"POST",&format!("{path}/send"),Some(&editor),Some(json!({"kind":"tcp_send","message":{"encoding":"base64","payload_source":"cHJpdmF0ZS10Y3AtcGF5bG9hZA==","secret":true}}))).await.0,StatusCode::OK);
    let events = until(&router, &format!("{path}/events"), &editor, |b| {
        b["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["direction"] == "incoming" && e["message"]["redacted"] == true)
    })
    .await;
    assert!(!events.to_string().contains("private-tcp-payload"));
    assert!(!events.to_string().contains("cHJpdmF0ZS10Y3AtcGF5bG9hZA=="));
    let (status, updated) = call(
        &router,
        "PUT",
        &base,
        Some(&editor),
        Some(json!({"name":"Revised","data":saved["data"],"expected_revision":saved["revision"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    let (status, withheld) = call(
        &router,
        "GET",
        &format!("{path}/events"),
        Some(&editor),
        None,
    )
    .await;
    assert!(
        matches!(status, StatusCode::CONFLICT | StatusCode::NOT_FOUND),
        "{withheld}"
    );
    assert!(!withheld.to_string().contains("private-tcp-payload"));
    tokio::time::timeout(Duration::from_secs(5), observed.recv())
        .await
        .unwrap()
        .unwrap();
    let (status, opened) = call(
        &router,
        "POST",
        &format!("{base}/sessions"),
        Some(&editor),
        Some(json!({"request_id":"r","expected_revision":updated["revision"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{opened}");
    let second = format!("{base}/sessions/{}", opened["id"].as_str().unwrap());
    until(&router, &second, &editor, |s| s["state"] == "open").await;
    assert_eq!(
        call(&router, "POST", "/api/auth/logout", Some(&editor), None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&router, "GET", &second, Some(&editor), None).await.0,
        StatusCode::UNAUTHORIZED
    );
    tokio::time::timeout(Duration::from_secs(5), observed.recv())
        .await
        .unwrap()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), fixture)
        .await
        .unwrap()
        .unwrap();
}
