mod common;
#[path = "fixtures/a2a/server.rs"]
mod fixture;
#[path = "fixtures/a2a/legacy.rs"]
mod legacy;
use common::*;
use std::time::Duration;
async fn event(router: &Router, id: &str, invocation: &str, kind: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let (status, batch) = call(
                router,
                "GET",
                &format!("/api/sessions/{id}/events"),
                None,
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{batch}");
            if let Some(event) = batch["events"].as_array().unwrap().iter().find(|event| {
                event["message"]["kind"] == kind
                    && (invocation.is_empty() || event["message"]["request_id"] == invocation)
            }) {
                return event["message"].clone();
            }
            if let Some(error) = batch["events"].as_array().unwrap().iter().find(|event| {
                event["message"]["kind"] == "a2a_error"
                    && event["message"]["request_id"] == invocation
            }) {
                panic!("Expected {kind} for {invocation}: {error}");
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("A2A event missing {invocation}/{kind}"))
}
async fn send(router: &Router, id: &str, invocation: &str, method: &str, params: Value) {
    let (status,value)=call(router,"POST",&format!("/api/sessions/{id}/send"),None,Some(json!({"kind":"a2a_request","request_id":invocation,"method":method,"params_source":params.to_string()}))).await;
    assert_eq!(status, StatusCode::OK, "{value}");
}
async fn setup(
    dialect: &str,
    transport: &str,
    url: &str,
) -> (tempfile::TempDir, Router, Value, String) {
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("a2a.db")).await.unwrap();
    let mut data = example_data();
    let request = &mut data["collections"][0]["requests"][0];
    request["protocol"] =
        json!({"kind":"a2a","dialect":dialect,"transport":transport,"params_source":"{unfinished"});
    request["url"] = json!(url);
    request["timeout_ms"] = json!(5000);
    request["examples"] = json!([]);
    let (status, w) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"A2A","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    let (status, session) = call(
        &router,
        "POST",
        "/api/sessions",
        None,
        Some(json!({"workspace_id":"w","request":w["data"]["collections"][0]["requests"][0]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{session}");
    let id = session["id"].as_str().unwrap().to_owned();
    event(&router, &id, "", "a2a_ready").await;
    (temp, router, w, id)
}
async fn mature_fixture() -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let app = fixture::router(&url);
    let handle = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, handle)
}
#[tokio::test]
async fn actual_current_sdk_jsonrpc_and_rest_message_task_context_card_and_errors() {
    for transport in ["jsonrpc", "http-json"] {
        let (url, server) = mature_fixture().await;
        let (_temp, router, w, id) = setup("1.0", transport, &url).await;
        let request = w["data"]["collections"][0]["requests"][0].clone();
        let (status, candidate) = call(
            &router,
            "POST",
            "/api/a2a/cards/discover",
            None,
            Some(json!({"workspace_id":"w","request":request})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{candidate}");
        assert_eq!(candidate["specification"]["kind"], "a2a-agent-card");
        assert_eq!(candidate["interfaces"].as_array().unwrap().len(), 2);
        let params = json!({"message":{"messageId":"first","role":"ROLE_USER","parts":[{"text":"Hello SDK"}]},"configuration":{"historyLength":20}});
        send(&router, &id, "send", "message/send", params).await;
        let response = event(&router, &id, "send", "a2a_result").await;
        let task = &response["result"]["task"];
        assert_eq!(
            task["status"]["state"], "TASK_STATE_COMPLETED",
            "{response}"
        );
        let task_id = task["id"].as_str().unwrap();
        let context = task["contextId"].as_str().unwrap();
        assert!(!context.is_empty());
        send(
            &router,
            &id,
            "get",
            "tasks/get",
            json!({"id":task_id,"historyLength":20}),
        )
        .await;
        let result = event(&router, &id, "get", "a2a_result").await;
        assert_eq!(result["result"]["id"], task_id);
        assert!(!result["result"]["history"].as_array().unwrap().is_empty());
        send(&router,&id,"follow","message/send",json!({"message":{"messageId":"follow","contextId":context,"role":"ROLE_USER","parts":[{"text":"Follow-up"}]}})).await;
        let next = event(&router, &id, "follow", "a2a_result").await;
        assert_eq!(next["result"]["task"]["contextId"], context);
        send(&router,&id,"stream","message/stream",json!({"message":{"messageId":"stream","role":"ROLE_USER","parts":[{"text":"Streaming SDK"}]}})).await;
        event(&router, &id, "stream", "a2a_stream").await;
        event(&router, &id, "stream", "a2a_finished").await;
        send(
            &router,
            &id,
            "resume",
            "tasks/resubscribe",
            json!({"id":task_id}),
        )
        .await;
        event(&router, &id, "resume", "a2a_error").await;
        event(&router, &id, "resume", "a2a_finished").await;
        send(
            &router,
            &id,
            "missing",
            "tasks/get",
            json!({"id":"missing"}),
        )
        .await;
        let error = event(&router, &id, "missing", "a2a_error").await;
        assert!(
            error["error"]["message"]
                .as_str()
                .unwrap()
                .contains("not found"),
            "{error}"
        );
        let (status, _) = call(
            &router,
            "DELETE",
            &format!("/api/sessions/{id}"),
            None,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        server.abort();
    }
}
#[tokio::test]
async fn version_transport_mismatch_and_card_original_bounds() {
    let (url, server) = mature_fixture().await;
    let (_temp, router, w, id) = setup("1.0", "jsonrpc", &url).await;
    let mut request = w["data"]["collections"][0]["requests"][0].clone();
    request["protocol"]["transport"] = json!("grpc");
    let (status, _) = call(
        &router,
        "POST",
        "/api/sessions",
        None,
        Some(json!({"workspace_id":"w","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let source=json!({"name":"original","description":"","version":"1","supportedInterfaces":[{"url":url,"protocolBinding":"GRPC","protocolVersion":"9.0"}],"unknown":{"preserved":true}}).to_string();
    let (status, candidate) = call(
        &router,
        "POST",
        "/api/a2a/cards/import",
        None,
        Some(json!({"workspace_id":"w","source":source,"dialect":"1.0"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{candidate}");
    assert_eq!(candidate["specification"]["source"], source);
    assert_eq!(candidate["interfaces"][0]["supported"], false);
    send(&router,&id,"implicit","message/send",json!({"message":{"messageId":"m","role":"ROLE_USER","parts":[{"text":"Hello"}]},"configuration":{"taskPushNotificationConfig":{"url":"http://127.0.0.1:1/callback"}}})).await;
    assert!(
        event(&router, &id, "implicit", "a2a_error").await["error"]["message"]
            .as_str()
            .unwrap()
            .contains("explicit")
    );
    server.abort();
}

#[tokio::test]
async fn legacy_sdk_parts_stream_artifacts_continuation_cancel_and_local_stop() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let original = legacy::card(&url).to_string();
    let app = legacy::router(&url);
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let (_temp, router, w, id) = setup("0.3", "jsonrpc", &url).await;
    let (status, candidate) = call(
        &router,
        "POST",
        "/api/a2a/cards/import",
        None,
        Some(json!({"workspace_id":"w","source":original,"dialect":"0.3"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{candidate}");
    assert_eq!(candidate["specification"]["source"], original);
    let params = |message_id: &str| json!({"message":{"kind":"message","messageId":message_id,"role":"user","parts":[{"kind":"text","text":"Hello legacy"},{"kind":"data","data":{"test":true}},{"kind":"file","file":{"uri":"http://127.0.0.1:1/no-fetch"}}]}});
    send(&router, &id, "input", "message/send", params("input")).await;
    let task = event(&router, &id, "input", "a2a_result").await;
    assert_eq!(task["result"]["status"]["state"], "input-required");
    assert_eq!(
        task["result"]["history"][0]["parts"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    let mut follow = params("follow");
    follow["message"]["taskId"] = json!("legacy-task");
    follow["message"]["contextId"] = json!("ctx-legacy");
    send(&router, &id, "follow", "message/send", follow).await;
    assert_eq!(
        event(&router, &id, "follow", "a2a_result").await["result"]["status"]["state"],
        "completed"
    );
    send(&router, &id, "stream", "message/stream", params("stream")).await;
    let result = event(&router, &id, "stream", "a2a_stream").await;
    assert_eq!(
        result["result"]["artifacts"][0]["parts"][0]["text"],
        "Artifact content"
    );
    event(&router, &id, "stream", "a2a_finished").await;
    send(
        &router,
        &id,
        "resubscribe",
        "tasks/resubscribe",
        json!({"id":"legacy-task"}),
    )
    .await;
    event(&router, &id, "resubscribe", "a2a_stream").await;
    event(&router, &id, "resubscribe", "a2a_finished").await;
    send(
        &router,
        &id,
        "cancel",
        "tasks/cancel",
        json!({"id":"legacy-task"}),
    )
    .await;
    assert_eq!(
        event(&router, &id, "cancel", "a2a_result").await["result"]["status"]["state"],
        "canceled"
    );
    send(&router, &id, "wait", "message/send", params("wait")).await;
    let (status, _) = call(
        &router,
        "POST",
        &format!("/api/sessions/{id}/send"),
        None,
        Some(json!({"kind":"a2a_stop","request_id":"wait"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        event(&router, &id, "wait", "a2a_error").await["error"]["message"]
            .as_str()
            .unwrap()
            .contains("stopped locally")
    );
    let (_, sessions) = call(&router, "GET", &format!("/api/sessions/{id}"), None, None).await;
    assert_eq!(sessions["state"], "open");
    assert_eq!(
        w["data"]["collections"][0]["requests"][0]["protocol"]["params_source"],
        "{unfinished"
    );
    server.abort();
}

#[tokio::test]
async fn discovery_stop_target_auth_redirect_body_bounds_and_scoped_privacy() {
    use axum::{Json, response::IntoResponse, routing::get};
    let app = Router::new()
        .route(
            "/slow.json",
            get(|| async {
                tokio::time::sleep(Duration::from_secs(30)).await;
                Json(json!({}))
            }),
        )
        .route("/auth.json", get(|| async { StatusCode::UNAUTHORIZED }))
        .route(
            "/redirect.json",
            get(|| async {
                (
                    StatusCode::TEMPORARY_REDIRECT,
                    [("location", "http://127.0.0.1:1/must-not-follow")],
                )
            }),
        )
        .route(
            "/large.json",
            get(|| async { ("x".repeat(1024 * 1024 + 1)).into_response() }),
        );
    let (url, server) = serve(app).await;
    let (_temp, router, w, _id) = setup("1.0", "jsonrpc", &url).await;
    let mut request = w["data"]["collections"][0]["requests"][0].clone();
    request["url"] = json!(format!("{url}/slow.json"));
    let payload = json!({"workspace_id":"w","request":request,"discovery_id":"cancel-me"});
    let task = tokio::spawn({
        let router = router.clone();
        async move {
            call(
                &router,
                "POST",
                "/api/a2a/cards/discover",
                None,
                Some(payload),
            )
            .await
        }
    });
    tokio::time::sleep(Duration::from_millis(40)).await;
    let (status, _) = call(
        &router,
        "POST",
        "/api/a2a/cards/discover/cancel",
        None,
        Some(json!({"workspace_id":"w","discovery_id":"cancel-me"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, error) = tokio::time::timeout(Duration::from_secs(1), task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error["error"].as_str().unwrap().contains("stopped locally"));
    for (path, expected) in [
        ("auth.json", "401"),
        ("redirect.json", "307"),
        ("large.json", "1 MiB"),
    ] {
        request["url"] = json!(format!("{url}/{path}"));
        let (status, error) = call(
            &router,
            "POST",
            "/api/a2a/cards/discover",
            None,
            Some(json!({"workspace_id":"w","request":request})),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
        assert!(
            error["error"].as_str().unwrap().contains(expected),
            "{error}"
        );
    }
    let (status, _) = call(&router, "GET", "/api/workspaces", None, None).await;
    assert_eq!(status, StatusCode::OK);
    server.abort();
    let (url, server) = mature_fixture().await;
    let (_temp, router, w, _id) = setup("1.0", "jsonrpc", &url).await;
    let locals = json!([{"scope":"project","key":"private-key","value":"987654321"},{"scope":"project","key":"private-name","value":"secret-data-key"}]);
    let(status,session)=call(&router,"POST","/api/sessions",None,Some(json!({"workspace_id":"w","request":w["data"]["collections"][0]["requests"][0],"locals":locals}))).await;
    assert_eq!(status, StatusCode::OK, "{session}");
    let id = session["id"].as_str().unwrap();
    event(&router, id, "", "a2a_ready").await;
    send(&router,id,"private","message/send",json!({"message":{"messageId":"private","role":"ROLE_USER","parts":[{"data":{"secret-data-key":987654321}},{"text":"{{private-key}}"}]}})).await;
    let result = event(&router, id, "private", "a2a_result")
        .await
        .to_string();
    assert!(
        !result.contains("secret-data-key") && !result.contains("987654321"),
        "{result}"
    );
    assert!(result.contains("REDACTED"));
    let source=json!({"name":"Original","description":"","version":"1","supportedInterfaces":[{"url":url,"protocolBinding":"JSONRPC","protocolVersion":"1.0"}],"metadata":{"secret-data-key":987654321}}).to_string();
    let (status, error) = call(
        &router,
        "POST",
        "/api/a2a/cards/import",
        None,
        Some(json!({"workspace_id":"w","dialect":"1.0","source":source,"locals":locals})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error["error"].as_str().unwrap().contains("private"));
    server.abort();
}

#[tokio::test]
async fn owner_session_and_sources_are_fenced_and_private_targets_blocked() {
    let temp = tempfile::tempdir().unwrap();
    let router = moleapi_server::hosted(config(
        format!(
            "sqlite://{}?mode=rwc",
            temp.path().join("owners.db").display()
        ),
        true,
    ))
    .await
    .unwrap();
    let alice = register(&router, "a2a-alice").await;
    let bob = register(&router, "a2a-bob").await;
    let mut data = example_data();
    let request = &mut data["collections"][0]["requests"][0];
    request["protocol"] = json!({"kind":"a2a","dialect":"1.0","transport":"jsonrpc"});
    request["url"] = json!("http://127.0.0.1:1/");
    request["examples"] = json!([]);
    let (status, w) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&alice),
        Some(json!({"id":"w","name":"A2A","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    let request = w["data"]["collections"][0]["requests"][0].clone();
    let (status, error) = call(
        &router,
        "POST",
        "/api/a2a/cards/discover",
        Some(&alice),
        Some(json!({"workspace_id":"w","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        error["error"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("network policy"),
        "{error}"
    );
    let (status, _) = call(
        &router,
        "POST",
        "/api/a2a/cards/import",
        Some(&bob),
        Some(json!({"workspace_id":"w","dialect":"1.0","source":"{}"})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, session) = call(
        &router,
        "POST",
        "/api/sessions",
        Some(&alice),
        Some(json!({"workspace_id":"w","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{session}");
    let id = session["id"].as_str().unwrap();
    let (status, _) = call(
        &router,
        "GET",
        &format!("/api/sessions/{id}"),
        Some(&bob),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    tokio::time::sleep(Duration::from_millis(20)).await;
    let(status,_)=call(&router,"POST",&format!("/api/sessions/{id}/send"),Some(&alice),Some(json!({"kind":"a2a_request","request_id":"denied","method":"message/send","params_source":"{\"message\":{\"messageId\":\"deny\",\"role\":\"ROLE_USER\",\"parts\":[{\"text\":\"deny\"}]}}"}))).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(&router, "POST", "/api/auth/logout", Some(&alice), None).await;
    assert_eq!(status, StatusCode::OK);
    let newtoken = register(&router, "a2a-owner-check").await;
    let (status, _) = call(
        &router,
        "GET",
        &format!("/api/sessions/{id}"),
        Some(&newtoken),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn actual_sdk_tls_verified_and_explicit_opt_out() {
    use std::sync::Arc;
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(
        vec![cert.cert.der().clone()],
        rustls::pki_types::PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der()).into(),
    )
    .unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "https://localhost:{}",
        listener.local_addr().unwrap().port()
    );
    let app = fixture::router(&url);
    let server = tokio::spawn(async move {
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let acceptor = acceptor.clone();
            let app = app.clone();
            tokio::spawn(async move {
                if let Ok(stream) = acceptor.accept(stream).await {
                    let service = hyper_util::service::TowerToHyperService::new(app);
                    let _ = hyper_util::server::conn::auto::Builder::new(
                        hyper_util::rt::TokioExecutor::new(),
                    )
                    .serve_connection(hyper_util::rt::TokioIo::new(stream), service)
                    .await;
                }
            });
        }
    });
    let (_temp, router, w, id) = setup("1.0", "jsonrpc", &url).await;
    let params =
        json!({"message":{"messageId":"tls","role":"ROLE_USER","parts":[{"text":"TLS SDK"}]}});
    send(&router, &id, "verified", "message/send", params.clone()).await;
    assert!(
        event(&router, &id, "verified", "a2a_error").await["error"]["message"]
            .as_str()
            .unwrap()
            .contains("error")
    );
    let mut request = w["data"]["collections"][0]["requests"][0].clone();
    request["verify_tls"] = json!(false);
    let (status, session) = call(
        &router,
        "POST",
        "/api/sessions",
        None,
        Some(json!({"workspace_id":"w","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let id = session["id"].as_str().unwrap();
    event(&router, id, "", "a2a_ready").await;
    send(&router, id, "opt-out", "message/send", params).await;
    assert_eq!(
        event(&router, id, "opt-out", "a2a_result").await["result"]["task"]["status"]["state"],
        "TASK_STATE_COMPLETED"
    );
    server.abort();
}

#[tokio::test]
async fn sdk_body_sse_json_and_work_limits_are_enforced_before_protocol_decode() {
    use axum::{Json, response::IntoResponse, routing::post};
    let app=Router::new().route("/large",post(||async{([( "content-type","application/json")],"x".repeat(1024*1024+1))})).route("/stream",post(||async{([( "content-type","text/event-stream")],format!("data: {}","x".repeat(1024*1024+1)))})).route("/deep",post(|Json(rpc):Json<Value>|async move{let mut data=json!(null);for _ in 0..40{data=json!([data]);}Json(json!({"jsonrpc":"2.0","id":rpc["id"],"result":{"message":{"messageId":"m","role":"ROLE_AGENT","parts":[{"data":data}]}}})).into_response()}));
    let (url, server) = serve(app).await;
    for (path, method, expected) in [
        ("large", "message/send", "1 MiB"),
        ("stream", "message/stream", "1 MiB"),
        ("deep", "message/send", "structure"),
    ] {
        let (_temp, router, _w, id) = setup("1.0", "jsonrpc", &format!("{url}/{path}")).await;
        send(
            &router,
            &id,
            "bound",
            method,
            json!({"message":{"messageId":"m","role":"ROLE_USER","parts":[{"text":"bounded"}]}}),
        )
        .await;
        let error = event(&router, &id, "bound", "a2a_error").await;
        assert!(
            error["error"]["message"]
                .as_str()
                .unwrap()
                .contains(expected),
            "{error}"
        );
    }
    let (_temp, router, _w, id) = setup("1.0", "jsonrpc", &format!("{url}/large")).await;
    send(
        &router,
        &id,
        "history",
        "tasks/get",
        json!({"id":"t","historyLength":1001}),
    )
    .await;
    assert!(
        event(&router, &id, "history", "a2a_error").await["error"]["message"]
            .as_str()
            .unwrap()
            .contains("historyLength")
    );
    send(&router,&id,"parts","message/send",json!({"message":{"messageId":"m","role":"ROLE_USER","parts":vec![json!({"text":"x"});129]}})).await;
    assert!(
        event(&router, &id, "parts", "a2a_error").await["error"]["message"]
            .as_str()
            .unwrap()
            .contains("128")
    );
    server.abort();
}

#[tokio::test]
async fn negotiated_current_sdk_task_listing_and_explicit_push_config_crud() {
    for transport in ["jsonrpc", "http-json"] {
        let (url, server) = mature_fixture().await;
        let (_temp, router, w, _id) = setup("1.0", transport, &url).await;
        let request = w["data"]["collections"][0]["requests"][0].clone();
        let (status, candidate) = call(
            &router,
            "POST",
            "/api/a2a/cards/discover",
            None,
            Some(json!({"workspace_id":"w","request":request})),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let mut request = request;
        request["protocol"]["card_source"] = candidate["specification"]["source"].clone();
        let (status, session) = call(
            &router,
            "POST",
            "/api/sessions",
            None,
            Some(json!({"workspace_id":"w","request":request})),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let id = session["id"].as_str().unwrap();
        event(&router, id, "", "a2a_ready").await;
        send(&router,id,"send","message/send",json!({"message":{"messageId":"push","role":"ROLE_USER","parts":[{"text":"Push CRUD SDK"}]}})).await;
        let result = event(&router, id, "send", "a2a_result").await;
        let task_id = result["result"]["task"]["id"].as_str().unwrap();
        let context = result["result"]["task"]["contextId"].as_str().unwrap();
        send(
            &router,
            id,
            "list",
            "tasks/list",
            json!({"contextId":context,"pageSize":5,"historyLength":10}),
        )
        .await;
        let list = event(&router, id, "list", "a2a_result").await;
        assert!(
            list["result"]["tasks"]
                .as_array()
                .unwrap()
                .iter()
                .any(|task| task["id"] == task_id)
        );
        send(
        &router,
        id,
        "push-set",
        "tasks/pushNotificationConfig/set",
        json!({"taskId":task_id,"id":"config-one","url":"http://127.0.0.1:1/explicit-callback"}),
    )
    .await;
        let config = event(&router, id, "push-set", "a2a_result").await;
        let config_id = config["result"]["id"].as_str().unwrap();
        assert_eq!(
            config["result"]["url"],
            "http://127.0.0.1:1/explicit-callback"
        );
        send(
            &router,
            id,
            "push-get",
            "tasks/pushNotificationConfig/get",
            json!({"taskId":task_id,"id":config_id}),
        )
        .await;
        assert_eq!(
            event(&router, id, "push-get", "a2a_result").await["result"]["id"],
            config_id
        );
        send(
            &router,
            id,
            "push-list",
            "tasks/pushNotificationConfig/list",
            json!({"taskId":task_id,"pageSize":10}),
        )
        .await;
        assert_eq!(
            event(&router, id, "push-list", "a2a_result").await["result"]["configs"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        send(
            &router,
            id,
            "push-delete",
            "tasks/pushNotificationConfig/delete",
            json!({"taskId":task_id,"id":config_id}),
        )
        .await;
        event(&router, id, "push-delete", "a2a_result").await;
        send(&router,id,"implicit-snake","message/send",json!({"message":{"message_id":"snake","role":"ROLE_USER","parts":[{"text":"No implicit registration"}]},"configuration":{"task_push_notification_config":{"url":"http://127.0.0.1:1/no"}}})).await;
        assert!(
            event(&router, id, "implicit-snake", "a2a_error").await["error"]["message"]
                .as_str()
                .unwrap()
                .contains("explicit")
        );
        server.abort();
    }
}

#[tokio::test]
async fn http_json_stream_rejects_complex_unknown_fields_before_sdk_typed_decode() {
    let payload=json!({"statusUpdate":{"taskId":"task","contextId":"context","status":{"state":"TASK_STATE_WORKING"}},"discarded":vec![Value::Null;10001]}).to_string();
    let body = format!("data: {payload}\n\n");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new().fallback(axum::routing::any(move || {
        let body = body.clone();
        async move { ([("content-type", "text/event-stream")], body) }
    }));
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let (_temp, router, _workspace, session) =
        setup("1.0", "http-json", &format!("http://{address}")).await;
    send(
        &router,
        &session,
        "complex-stream",
        "message/stream",
        json!({"message":{"messageId":"bounded","role":"ROLE_USER","parts":[{"text":"bounded"}]}}),
    )
    .await;
    let error = event(&router, &session, "complex-stream", "a2a_error").await;
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("structure exceeds limits"),
        "{error}"
    );
    server.abort();
}

#[tokio::test]
async fn a2a_sdk_and_card_discovery_share_network_dns_settings() {
    let (url, server) = mature_fixture().await;
    let port = url::Url::parse(&url).unwrap().port().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("a2a-network.db")).await.unwrap();
    let mut data = example_data();
    let request = &mut data["collections"][0]["requests"][0];
    request["protocol"] =
        json!({"kind":"a2a","dialect":"1.0","transport":"jsonrpc","params_source":"{unfinished"});
    request["url"] = json!(format!("http://a2a-network.test:{port}/"));
    request["timeout_ms"] = json!(5000);
    request["examples"] = json!([]);
    request["network"] = json!({"dns":[{"hostname":"a2a-network.test","addresses":["127.0.0.1"]}]});
    let (status, w) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"A2A network","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    let request = w["data"]["collections"][0]["requests"][0].clone();
    let (status, card) = call(
        &router,
        "POST",
        "/api/a2a/cards/discover",
        None,
        Some(json!({"workspace_id":"w","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{card}");
    let (status, session) = call(
        &router,
        "POST",
        "/api/sessions",
        None,
        Some(json!({"workspace_id":"w","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{session}");
    let id = session["id"].as_str().unwrap();
    event(&router, id, "", "a2a_ready").await;
    send(&router,id,"network-send","message/send",json!({"message":{"messageId":"network","role":"ROLE_USER","parts":[{"text":"Network SDK"}]}})).await;
    let response = event(&router, id, "network-send", "a2a_result").await;
    assert_eq!(
        response["result"]["task"]["status"]["state"], "TASK_STATE_COMPLETED",
        "{response}"
    );
    call(
        &router,
        "DELETE",
        &format!("/api/sessions/{id}"),
        None,
        None,
    )
    .await;
    server.abort();
}
