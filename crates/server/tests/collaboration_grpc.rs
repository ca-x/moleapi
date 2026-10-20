mod common;
#[path = "../../protocols/tests/support/grpc.rs"]
mod fixture;
use common::*;
use std::time::Duration;
fn source() -> String {
    json!({"kind":"proto","files":[{"path":"service.proto","content":include_str!("../../protocols/tests/fixtures/service.proto")},{"path":"types.proto","content":include_str!("../../protocols/tests/fixtures/types.proto")}],"entry_files":["service.proto"]}).to_string()
}
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
    member_id: String,
}
async fn setup(url: &str) -> Shared {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(
        moleapi_server::sqlite_database_url(&dir.path().join("grpc.db")).unwrap(),
        true,
    );
    cfg.allow_private_network = true;
    let router = hosted(cfg).await.unwrap();
    let owner = register(&router, "grpcowner").await;
    let editor = register(&router, "grpceditor").await;
    let other = register(&router, "grpcother").await;
    let viewer = register(&router, "grpcviewer").await;
    let foreign = register(&router, "grpcforeign").await;
    let mut data = example_data();
    data["script_mode"] = "v2".into();
    data["specifications"] = json!([{"id":"proto","name":"Owner Proto","kind":"protobuf","source":source(),"dialect":"proto3"}]);
    data["global_variables"] =
        json!([{"id":"v","key":"message","value":"owner-value","enabled":true}]);
    let r = &mut data["collections"][0]["requests"][0];
    r["url"] = url.into();
    r["method"] = "POST".into();
    r["examples"] = json!([]);
    r["specification_id"] = "proto".into();
    r["auth"] = json!({"kind":"bearer","token":"fixture-secret","username":"","password":""});
    r["pre_request_script"] =
        "pm.request.headers.add({key:'x-require-auth',value:'bearer'});".into();
    r["protocol"] = json!({"kind":"grpc","service":"moleapi.fixture.EchoService","method":"Unary","message_source":"{\"text\":\"{{message}}\"}"});
    r["timeout_ms"] = 30000.into();
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"gRPC","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let mut collision = data;
    collision["global_variables"][0]["value"] = "wrong-member".into();
    collision["specifications"][0]["source"] =
        source().replace("EchoService", "WrongService").into();
    collision["collections"][0]["requests"][0]["protocol"]["service"] =
        "moleapi.fixture.WrongService".into();
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
    let mut member_id = String::new();
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
            member_id = accepted["member_id"].as_str().unwrap().into();
        }
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
        member_id,
    }
}
async fn put(s: &mut Shared) {
    let (status,saved)=call(&s.router,"PUT",&s.base,Some(&s.editor),Some(json!({"name":s.saved["name"],"data":s.saved["data"],"expected_revision":s.saved["revision"]}))).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    s.saved = saved;
}
async fn until(s: &Shared, path: &str, predicate: impl Fn(&Value) -> bool) -> Value {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let (status, value) = call(&s.router, "GET", path, Some(&s.editor), None).await;
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
async fn send(s: &Shared, path: &str, message: Value) {
    let (status, value) = call(
        &s.router,
        "POST",
        &format!("{path}/send"),
        Some(&s.editor),
        Some(message),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
}
#[tokio::test]
async fn shared_grpc_stages_definitions_and_uses_canonical_owner_for_all_four_modes() {
    let (url, fixture) = fixture::start(false).await;
    let mut s = setup(&url).await;
    let input = json!({"request_id":"r","specification_id":"proto","expected_revision":s.saved["revision"]});
    let (status, loaded) = call(
        &s.router,
        "POST",
        &format!("{}/grpc/schema", s.base),
        Some(&s.editor),
        Some(input.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{loaded}");
    assert_eq!(
        loaded["schema"]["services"][0]["name"],
        "moleapi.fixture.EchoService"
    );
    for token in [&s.viewer, &s.foreign] {
        assert!(
            !call(
                &s.router,
                "POST",
                &format!("{}/grpc/schema", s.base),
                Some(token),
                Some(input.clone())
            )
            .await
            .0
            .is_success()
        );
    }
    assert_eq!(call(&s.router,"POST",&format!("{}/grpc/schema",s.base),Some(&s.editor),Some(json!({"request_id":"r","specification_id":"wrong","expected_revision":s.saved["revision"]}))).await.0,StatusCode::NOT_FOUND);
    let (status,candidate)=call(&s.router,"POST",&format!("{}/grpc/import",s.base),Some(&s.editor),Some(json!({"request_id":"r","expected_revision":s.saved["revision"],"name":"Imported","source":source()}))).await;
    assert_eq!(status, StatusCode::OK, "{candidate}");
    assert_eq!(candidate["specification"]["source"], source());
    let (status, reflected) = call(
        &s.router,
        "POST",
        &format!("{}/grpc/reflect", s.base),
        Some(&s.editor),
        Some(json!({"request_id":"r","expected_revision":s.saved["revision"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reflected}");
    assert_eq!(reflected["specification"]["kind"], "protobuf");
    assert_eq!(
        reflected["schema"]["services"][0]["methods"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert!(!reflected.to_string().contains("fixture-secret"));
    let (_, unchanged) = call(&s.router, "GET", "/api/workspaces/w", Some(&s.owner), None).await;
    assert_eq!(unchanged["revision"], s.saved["revision"]);
    assert_eq!(unchanged["data"], s.saved["data"]);
    s.saved["data"]["specifications"]
        .as_array_mut()
        .unwrap()
        .push(candidate["specification"].clone());
    s.saved["data"]["collections"][0]["requests"][0]["specification_id"] =
        candidate["specification"]["id"].clone();
    put(&mut s).await;
    assert_eq!(
        call(
            &s.router,
            "POST",
            &format!("{}/grpc/schema", s.base),
            Some(&s.editor),
            Some(input)
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    for method in ["Unary", "ServerStream", "ClientStream", "Bidi"] {
        s.saved["data"]["collections"][0]["requests"][0]["protocol"]["method"] = method.into();
        put(&mut s).await;
        let create = json!({"request_id":"r","expected_revision":s.saved["revision"]});
        assert_eq!(
            call(
                &s.router,
                "POST",
                &format!("{}/sessions", s.base),
                Some(&s.viewer),
                Some(create.clone())
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
        let (status, opened) = call(
            &s.router,
            "POST",
            &format!("{}/sessions", s.base),
            Some(&s.editor),
            Some(create),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{opened}");
        let path = format!("{}/sessions/{}", s.base, opened["id"].as_str().unwrap());
        for token in [&s.other, &s.viewer, &s.foreign] {
            assert_eq!(
                call(
                    &s.router,
                    "POST",
                    &format!("{path}/send"),
                    Some(token),
                    Some(json!({"kind":"grpc_half_close"}))
                )
                .await
                .0,
                StatusCode::NOT_FOUND
            );
        }
        assert_eq!(
            call(
                &s.router,
                "POST",
                &format!("{path}/send"),
                Some(&s.editor),
                Some(json!({"kind":"text","text":"wrong-family"}))
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
        if method == "ClientStream" || method == "Bidi" {
            until(&s, &path, |v| v["state"] == "open").await;
            send(
                &s,
                &path,
                json!({"kind":"grpc_message","message_source":"{\"text\":\"second\"}"}),
            )
            .await;
            send(&s, &path, json!({"kind":"grpc_half_close"})).await;
        }
        until(&s, &path, |v| {
            v["state"] == "closed" || v["state"] == "error"
        })
        .await;
        let (_, batch) = call(
            &s.router,
            "GET",
            &format!("{path}/events"),
            Some(&s.editor),
            None,
        )
        .await;
        let messages = batch["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["direction"] == "incoming" && e["message"]["kind"] == "grpc_message")
            .map(|e| e["message"]["message"]["text"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            messages,
            match method {
                "Unary" => vec!["owner-value"],
                "ServerStream" => vec!["owner-value"; 3],
                "ClientStream" => vec!["owner-valuesecond"],
                _ => vec!["owner-value", "second"],
            },
            "{batch}"
        );
        assert!(
            batch["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["message"]["kind"] == "grpc_status" && e["message"]["code"] == 0),
            "{batch}"
        );
        let (_, canonical) =
            call(&s.router, "GET", "/api/workspaces/w", Some(&s.owner), None).await;
        assert_eq!(canonical["data"], s.saved["data"]);
        assert_eq!(canonical["revision"], s.saved["revision"]);
    }
    let (_, audit) = call(
        &s.router,
        "GET",
        &format!("{}/audit", s.base),
        Some(&s.owner),
        None,
    )
    .await;
    for action in [
        "grpc.schema.read",
        "grpc.import",
        "grpc.reflect",
        "protocol.grpc.message",
        "protocol.grpc.half_close",
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
    assert!(!audit.to_string().contains("fixture-secret"));
    assert!(!audit.to_string().contains("syntax ="));
    fixture.abort();
}

#[tokio::test]
async fn shared_reflection_drops_pending_socket_after_role_revision_or_credential_change() {
    use tokio::io::AsyncReadExt;
    for boundary in ["role", "revision", "logout"] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let mut s = setup(&url).await;
        let router = s.router.clone();
        let actor = s.editor.clone();
        let path = format!("{}/grpc/reflect", s.base);
        let revision = s.saved["revision"].clone();
        let reflected = tokio::spawn(async move {
            call(
                &router,
                "POST",
                &path,
                Some(&actor),
                Some(json!({"request_id":"r","expected_revision":revision})),
            )
            .await
        });
        let (mut socket, _) = tokio::time::timeout(Duration::from_secs(3), listener.accept())
            .await
            .unwrap()
            .unwrap();
        let expected = match boundary {
            "role" => {
                assert_eq!(
                    call(
                        &s.router,
                        "PATCH",
                        &format!("{}/members/{}", s.base, s.member_id),
                        Some(&s.owner),
                        Some(json!({"role":"viewer","expected_revision":1}))
                    )
                    .await
                    .0,
                    StatusCode::OK
                );
                StatusCode::NOT_FOUND
            }
            "revision" => {
                s.saved["name"] = "Changed".into();
                put(&mut s).await;
                StatusCode::CONFLICT
            }
            _ => {
                assert_eq!(
                    call(&s.router, "POST", "/api/auth/logout", Some(&s.editor), None)
                        .await
                        .0,
                    StatusCode::OK
                );
                StatusCode::UNAUTHORIZED
            }
        };
        let (status, value) = tokio::time::timeout(Duration::from_secs(2), reflected)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(status, expected, "{boundary}: {value}");
        assert!(value["specification"].is_null());
        tokio::time::timeout(Duration::from_secs(2), async {
            let mut bytes = [0u8; 4096];
            loop {
                match socket.read(&mut bytes).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
            }
        })
        .await
        .unwrap();
    }
}
