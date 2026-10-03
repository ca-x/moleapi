mod common;
#[path = "../../protocols/tests/support/grpc.rs"]
mod fixture;
use common::*;
use std::time::Duration;
fn source() -> String {
    json!({"kind":"proto","files":[{"path":"service.proto","content":include_str!("../../protocols/tests/fixtures/service.proto")},{"path":"types.proto","content":include_str!("../../protocols/tests/fixtures/types.proto")}],"entry_files":["service.proto"]}).to_string()
}
fn grpc_data(url: &str, method: &str) -> Value {
    let mut data = example_data();
    data["specifications"] = json!([{"id":"proto","name":"Echo","kind":"protobuf","source":source(),"dialect":"proto3"}]);
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = json!(url);
    request["method"] = json!("POST");
    request["examples"] = json!([]);
    request["specification_id"] = json!("proto");
    request["protocol"] = json!({"kind":"grpc","service":"moleapi.fixture.EchoService","method":method,"message_source":"{\"text\":\"{{message}}\"}"});
    data["global_variables"] = json!([{"id":"v","key":"message","value":"global","enabled":true}]);
    data
}
async fn workspace(router: &Router, token: Option<&str>, data: Value) -> Value {
    let (status, value) = call(
        router,
        "POST",
        "/api/workspaces",
        token,
        Some(json!({"id":"w","name":"gRPC","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    value
}
async fn create(router: &Router, token: Option<&str>, request: &Value) -> Value {
    let (status, value) = call(
        router,
        "POST",
        "/api/sessions",
        token,
        Some(json!({"workspace_id":"w","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    value
}
async fn terminal(router: &Router, token: Option<&str>, id: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let (_, summary) =
                call(router, "GET", &format!("/api/sessions/{id}"), token, None).await;
            if summary["state"] == "closed" || summary["state"] == "error" {
                assert_eq!(summary["state"], "closed", "{summary}");
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    call(
        router,
        "GET",
        &format!("/api/sessions/{id}/events?after=0"),
        token,
        None,
    )
    .await
    .1
}
#[tokio::test]
async fn offline_ipc_schema_import_saved_reopen_reflection_and_actual_four_modes() {
    let (url, server) = fixture::start(false).await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("local.db")).await.unwrap();
    let mut data = grpc_data(&url, "Unary");
    data["environments"] = json!([{"id":"env","name":"selected","variables":[{"id":"ev","key":"message","value":"environment","enabled":true}]}]);
    data["active_environment_id"] = json!("env");
    data["collections"][0]["requests"][0]["pre_request_script"] =
        json!("pm.environment.set('message','pre-script');console.log('grpc-pre');");
    let w = workspace(&router, None, data).await;
    let (status, import) = call(
        &router,
        "POST",
        "/api/grpc/import",
        None,
        Some(json!({"workspace_id":"w","name":"Imported","source":source()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{import}");
    assert_eq!(
        import["schema"]["services"][0]["methods"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    let (status, reopened) = call(
        &router,
        "POST",
        "/api/grpc/schema",
        None,
        Some(json!({"workspace_id":"w","specification_id":"proto"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reopened}");
    assert_eq!(reopened["specification"]["source"], source());
    let request = w["data"]["collections"][0]["requests"][0].clone();
    let (status, reflected) = call(
        &router,
        "POST",
        "/api/grpc/reflect",
        None,
        Some(json!({"workspace_id":"w","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reflected}");
    assert_eq!(reflected["specification"]["kind"], "protobuf");
    // The existing script contract conservatively taints values written by scripts.
    // Private updates cannot escape into a persisted schema/request draft.
    assert!(reflected["variable_updates"].as_array().unwrap().is_empty());
    assert!(
        reflected["logs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|log| log["message"] == "grpc-pre")
    );
    for mode in ["Unary", "ServerStream", "ClientStream", "Bidi"] {
        let mut draft = request.clone();
        draft["protocol"]["method"] = json!(mode);
        let session = create(&router, None, &draft).await;
        let id = session["id"].as_str().unwrap();
        assert_eq!(session["protocol"], "grpc");
        assert_eq!(
            session["client_half_closed"],
            matches!(mode, "Unary" | "ServerStream")
        );
        if matches!(mode, "ClientStream" | "Bidi") {
            assert_eq!(
                call(
                    &router,
                    "POST",
                    &format!("/api/sessions/{id}/send"),
                    None,
                    Some(
                        json!({"kind":"grpc_message","message_source":"{\"text\":\"{{message}}\"}"})
                    )
                )
                .await
                .0,
                StatusCode::OK
            );
            assert_eq!(
                call(
                    &router,
                    "POST",
                    &format!("/api/sessions/{id}/send"),
                    None,
                    Some(json!({"kind":"grpc_half_close"}))
                )
                .await
                .0,
                StatusCode::OK
            );
            assert_eq!(
                call(
                    &router,
                    "POST",
                    &format!("/api/sessions/{id}/send"),
                    None,
                    Some(json!({"kind":"grpc_message","message_source":"{}"}))
                )
                .await
                .0,
                StatusCode::BAD_REQUEST
            );
        }
        let events = terminal(&router, None, id).await;
        let messages: Vec<_> = events["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| {
                event["direction"] == "incoming" && event["message"]["kind"] == "grpc_message"
            })
            .collect();
        assert_eq!(
            messages.len(),
            match mode {
                "ServerStream" => 3,
                "Bidi" => 2,
                _ => 1,
            },
            "{events}"
        );
        assert_eq!(messages[0]["message"]["message"], "[REDACTED]");
        assert!(
            events["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|event| event["message"]["kind"] == "grpc_status"
                    && event["message"]["code"] == 0)
        );
        assert_eq!(
            call(
                &router,
                "DELETE",
                &format!("/api/sessions/{id}"),
                None,
                None
            )
            .await
            .0,
            StatusCode::OK
        );
    }
    server.abort();
}
#[tokio::test]
async fn hosted_ownership_private_network_malformed_drafts_and_schema_import_fences() {
    let temp = tempfile::tempdir().unwrap();
    let router = hosted(config(
        format!(
            "sqlite://{}?mode=rwc",
            temp.path().join("hosted.db").display()
        ),
        true,
    ))
    .await
    .unwrap();
    let owner = register(&router, "grpc-owner").await;
    let intruder = register(&router, "grpc-intruder").await;
    let (url, server) = fixture::start(false).await;
    let w = workspace(&router, Some(&owner), grpc_data(&url, "Unary")).await;
    for endpoint in ["schema", "import", "reflect"] {
        let body = match endpoint {
            "schema" => json!({"workspace_id":"w","specification_id":"proto"}),
            "import" => json!({"workspace_id":"w","name":"Import","source":source()}),
            _ => json!({"workspace_id":"w","request":w["data"]["collections"][0]["requests"][0]}),
        };
        assert_eq!(
            call(
                &router,
                "POST",
                &format!("/api/grpc/{endpoint}"),
                Some(&intruder),
                Some(body)
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
    }
    let mut request = w["data"]["collections"][0]["requests"][0].clone();
    request["protocol"]["method"] = json!("does/not/exist");
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/sessions",
            Some(&owner),
            Some(json!({"workspace_id":"w","request":request}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    request["protocol"]["method"] = json!("Unary");
    request["protocol"]["message_source"] = json!("malformed saved draft");
    let mut saved = w["data"].clone();
    saved["collections"][0]["requests"][0] = request.clone();
    assert_eq!(
        call(
            &router,
            "PUT",
            "/api/workspaces/w",
            Some(&owner),
            Some(json!({"name":"Draft","data":saved,"expected_revision":w["revision"]}))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/sessions",
            Some(&owner),
            Some(json!({"workspace_id":"w","request":request}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    request["protocol"]["message_source"] = json!("{}");
    let session = create(&router, Some(&owner), &request).await;
    let id = session["id"].as_str().unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let (_, summary) = call(
                &router,
                "GET",
                &format!("/api/sessions/{id}"),
                Some(&owner),
                None,
            )
            .await;
            if summary["state"] == "error" {
                assert!(summary["reason"].as_str().unwrap().contains("Private"));
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        call(
            &router,
            "GET",
            &format!("/api/sessions/{id}"),
            Some(&intruder),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, reflection) = call(
        &router,
        "POST",
        "/api/grpc/reflect",
        Some(&owner),
        Some(json!({"workspace_id":"w","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(reflection["error"].as_str().unwrap().contains("Private"));
    let bad_source=json!({"kind":"proto","files":[{"path":"../bad.proto","content":""}],"entry_files":["../bad.proto"]}).to_string();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/grpc/import",
            Some(&owner),
            Some(json!({"workspace_id":"w","name":"Unsafe","source":bad_source}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    server.abort();
}
#[tokio::test]
async fn reflected_descriptors_and_metadata_never_export_known_private_source_values() {
    let (url, server) = fixture::start(false).await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("privacy.db")).await.unwrap();
    let mut data = grpc_data(&url, "Unary");
    data["global_variables"].as_array_mut().unwrap().push(json!({"id":"secret","key":"schema-secret","value":"EchoService","enabled":true,"secret":true}));
    let w = workspace(&router, None, data).await;
    let (status, reflected) = call(
        &router,
        "POST",
        "/api/grpc/reflect",
        None,
        Some(json!({"workspace_id":"w","request":w["data"]["collections"][0]["requests"][0]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(reflected["specification"].is_null());
    assert!(reflected["schema"].is_null());
    assert!(reflected["error"].as_str().unwrap().contains("withheld"));
    assert!(!reflected.to_string().contains("EchoService"));
    server.abort();
}
#[tokio::test]
async fn initial_message_script_edit_basic_metadata_and_reflection_target_status_are_usable() {
    let (url, server) = fixture::start(false).await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("scripts.db")).await.unwrap();
    let mut data = grpc_data(&url, "Unary");
    let request = &mut data["collections"][0]["requests"][0];
    request["protocol"]["message_source"] = json!("{}");
    request["pre_request_script"] = json!(
        "pm.request.body.update('{\"text\":\"script-edited initial message\",\"count\":\"4\"}');"
    );
    request["auth"] = json!({"kind":"basic","username":"user","password":"pass","token":""});
    request["headers"] = json!([{"id":"h","key":"x-require-auth","value":"basic","enabled":true},{"id":"bin","key":"x-client-bin","value":"AP8=","enabled":true}]);
    let w = workspace(&router, None, data).await;
    let mut request = w["data"]["collections"][0]["requests"][0].clone();
    let session = create(&router, None, &request).await;
    let id = session["id"].as_str().unwrap();
    assert!(
        session["request_updates"]
            .as_array()
            .unwrap()
            .iter()
            .any(|update| update["field"] == "body")
    );
    let events = terminal(&router, None, id).await;
    let message = events["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|event| {
            event["direction"] == "incoming" && event["message"]["kind"] == "grpc_message"
        })
        .unwrap();
    assert_eq!(
        message["message"]["message"]["text"],
        "script-edited initial message"
    );
    assert_eq!(message["message"]["message"]["count"], "4");
    request["auth"]["kind"] = json!("none");
    let (status, reflection) = call(
        &router,
        "POST",
        "/api/grpc/reflect",
        None,
        Some(json!({"workspace_id":"w","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reflection}");
    assert_eq!(reflection["status"]["code"], 16, "{reflection}");
    assert_eq!(reflection["status"]["name"], "Unauthenticated");
    assert!(reflection["specification"].is_null());
    server.abort();
}
#[tokio::test]
async fn authenticated_logout_cancels_real_bidi_and_fences_old_owner_token() {
    let temp = tempfile::tempdir().unwrap();
    let mut config = config(
        format!(
            "sqlite://{}?mode=rwc",
            temp.path().join("logout.db").display()
        ),
        true,
    );
    config.allow_private_network = true;
    let router = hosted(config).await.unwrap();
    let token = register(&router, "grpc-logout").await;
    let (url, server) = fixture::start(false).await;
    let w = workspace(&router, Some(&token), grpc_data(&url, "Bidi")).await;
    let session = create(
        &router,
        Some(&token),
        &w["data"]["collections"][0]["requests"][0],
    )
    .await;
    let id = session["id"].as_str().unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let (_, events) = call(
                &router,
                "GET",
                &format!("/api/sessions/{id}/events?after=0"),
                Some(&token),
                None,
            )
            .await;
            if events["events"].as_array().unwrap().iter().any(|event| {
                event["direction"] == "incoming" && event["message"]["kind"] == "grpc_message"
            }) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        call(&router, "POST", "/api/auth/logout", Some(&token), None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &router,
            "POST",
            &format!("/api/sessions/{id}/send"),
            Some(&token),
            Some(json!({"kind":"grpc_message","message_source":"{}"}))
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let (status, login) = call(
        &router,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({"username":"grpc-logout","password":"goodpassword123"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let replacement = login["token"].as_str().unwrap();
    let events = terminal(&router, Some(replacement), id).await;
    assert!(events["events"].as_array().unwrap().iter().any(|event|event["message"]["kind"]=="grpc_status" && event["message"]["code"]==1));
    assert_eq!(
        call(
            &router,
            "POST",
            &format!("/api/sessions/{id}/send"),
            Some(replacement),
            Some(json!({"kind":"grpc_half_close"}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    server.abort();
}
#[tokio::test]
async fn logout_cancels_inflight_reflection_socket_and_cannot_return_a_late_schema() {
    use tokio::io::AsyncReadExt;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let temp = tempfile::tempdir().unwrap();
    let mut config = config(
        format!(
            "sqlite://{}?mode=rwc",
            temp.path().join("reflect-cancel.db").display()
        ),
        true,
    );
    config.allow_private_network = true;
    let router = hosted(config).await.unwrap();
    let token = register(&router, "reflection-logout").await;
    let w = workspace(&router, Some(&token), grpc_data(&url, "Unary")).await;
    let request = w["data"]["collections"][0]["requests"][0].clone();
    let worker_router = router.clone();
    let worker_token = token.clone();
    let reflected = tokio::spawn(async move {
        call(
            &worker_router,
            "POST",
            "/api/grpc/reflect",
            Some(&worker_token),
            Some(json!({"workspace_id":"w","request":request})),
        )
        .await
    });
    let (mut target, _) = tokio::time::timeout(Duration::from_secs(3), listener.accept())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        call(&router, "POST", "/api/auth/logout", Some(&token), None)
            .await
            .0,
        StatusCode::OK
    );
    let (status, response) = tokio::time::timeout(Duration::from_secs(2), reflected)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{response}");
    assert!(response["specification"].is_null());
    // Dropping the real tonic connect/invocation future closes a pending target socket.
    tokio::time::timeout(Duration::from_secs(2), async {
        let mut bytes = [0u8; 4096];
        loop {
            match target.read(&mut bytes).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn reflection_keeps_private_resolved_request_labels_out_of_saved_schema_and_export() {
    let (url, server) = fixture::start(false).await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("schema-label.db")).await.unwrap();
    let mut data = grpc_data(&url, "Unary");
    data["collections"][0]["requests"][0]["name"] = json!("{{message}}");
    data["global_variables"] = json!([{"id":"private-title","key":"message","value":"private-schema-label-token","enabled":true,"secret":true}]);
    let w = workspace(&router, None, data).await;
    let request = w["data"]["collections"][0]["requests"][0].clone();
    let mut latest = w.clone();
    for locals in [
        json!([]),
        json!([{"scope":"project","key":"message","value":"local-schema-label-token"}]),
    ] {
        let (status, reflected) = call(
            &router,
            "POST",
            "/api/grpc/reflect",
            None,
            Some(json!({"workspace_id":"w","request":request,"locals":locals})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{reflected}");
        assert_eq!(reflected["specification"]["kind"], "protobuf");
        assert_eq!(reflected["specification"]["name"], "{{message}} schema");
        for private in ["private-schema-label-token", "local-schema-label-token"] {
            assert!(!reflected.to_string().contains(private), "{reflected}");
        }
        let mut data = latest["data"].clone();
        data["specifications"]
            .as_array_mut()
            .unwrap()
            .push(reflected["specification"].clone());
        let (status, saved) = call(
            &router,
            "PUT",
            "/api/workspaces/w",
            None,
            Some(json!({"name":"gRPC","data":data,"expected_revision":latest["revision"]})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{saved}");
        latest = saved;
        let (status, exported) = call(
            &router,
            "POST",
            "/api/workspaces/w/export",
            None,
            Some(json!({"format":"moleapi","include_secrets":false})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{exported}");
        for private in ["private-schema-label-token", "local-schema-label-token"] {
            assert!(!exported["content"].as_str().unwrap().contains(private));
        }
    }
    server.abort();
}
