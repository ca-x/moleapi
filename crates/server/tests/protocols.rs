mod common;
use axum::{
    body::Body,
    extract::ws::{Message, WebSocketUpgrade},
    routing::get,
};
use common::*;
use std::time::Duration;
fn session_data(url: &str, kind: &str) -> Value {
    let mut d = example_data();
    d["collections"][0]["requests"][0]["url"] = json!(url);
    d["collections"][0]["requests"][0]["protocol"] = json!({"kind":kind});
    d
}
async fn workspace(router: &Router, token: Option<&str>, data: Value) -> Value {
    let (status, w) = call(
        router,
        "POST",
        "/api/workspaces",
        token,
        Some(json!({"id":"w","name":"Protocol tests","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    w
}
async fn create(router: &Router, token: Option<&str>, w: &Value) -> Value {
    let (status, s) = call(
        router,
        "POST",
        "/api/sessions",
        token,
        Some(json!({"workspace_id":"w","request":w["data"]["collections"][0]["requests"][0]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{s}");
    s
}
async fn wait(router: &Router, token: Option<&str>, id: &str, state: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let (status, s) =
                call(router, "GET", &format!("/api/sessions/{id}"), token, None).await;
            assert_eq!(status, StatusCode::OK, "{s}");
            if s["state"] == state {
                return s;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}
#[tokio::test]
async fn native_ipc_sessions_send_receive_close_and_workspace_cleanup() {
    let fixture = Router::new().route(
        "/socket",
        get(|upgrade: WebSocketUpgrade| async {
            upgrade.on_upgrade(|mut socket| async move {
                while let Some(Ok(message)) = socket.recv().await {
                    if matches!(message, Message::Close(_)) {
                        break;
                    }
                    socket.send(message).await.unwrap();
                }
            })
        }),
    );
    let (url, server) = serve(fixture).await;
    let url = url.replacen("http", "ws", 1);
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("local.db")).await.unwrap();
    let w = workspace(
        &router,
        None,
        session_data(&format!("{url}/socket"), "websocket"),
    )
    .await;
    let session = create(&router, None, &w).await;
    let id = session["id"].as_str().unwrap();
    wait(&router, None, id, "open").await;
    assert_eq!(
        call(
            &router,
            "POST",
            &format!("/api/sessions/{id}/send"),
            None,
            Some(json!({"kind":"text","text":"native IPC echo"}))
        )
        .await
        .0,
        StatusCode::OK
    );
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let (_, batch) = call(
                &router,
                "GET",
                &format!("/api/sessions/{id}/events?after=0"),
                None,
                None,
            )
            .await;
            if batch["events"].as_array().unwrap().iter().any(|event| {
                event["direction"] == "incoming" && event["message"]["text"] == "native IPC echo"
            }) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let (status, closed) = call(
        &router,
        "POST",
        &format!("/api/sessions/{id}/close"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(closed["state"], "closed");
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
    assert_eq!(
        call(&router, "GET", &format!("/api/sessions/{id}"), None, None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let session = create(&router, None, &w).await;
    let id = session["id"].as_str().unwrap();
    wait(&router, None, id, "open").await;
    assert_eq!(
        call(
            &router,
            "DELETE",
            "/api/workspaces/w",
            None,
            Some(json!({"expected_revision":1}))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&router, "GET", &format!("/api/sessions/{id}"), None, None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    server.abort();
}
#[tokio::test]
async fn hosted_owner_isolation_membership_post_script_rejection_and_private_deny() {
    let fixture = Router::new().route(
        "/events",
        get(|| async { ([("content-type", "text/event-stream")], "data: hello\n\n") }),
    );
    let (url, server) = serve(fixture).await;
    let temp = tempfile::tempdir().unwrap();
    let router = hosted(config(
        moleapi_server::sqlite_database_url(&temp.path().join("hosted.db")).unwrap(),
        true,
    ))
    .await
    .unwrap();
    let first = register(&router, "first").await;
    let second = register(&router, "second").await;
    let w = workspace(
        &router,
        Some(&first),
        session_data(&format!("{url}/events"), "sse"),
    )
    .await;
    let session = create(&router, Some(&first), &w).await;
    let id = session["id"].as_str().unwrap();
    assert!(
        wait(&router, Some(&first), id, "error").await["reason"]
            .as_str()
            .unwrap()
            .contains("blocked")
    );
    for (method, path, body) in [
        ("GET", format!("/api/sessions/{id}"), None),
        ("GET", format!("/api/sessions/{id}/events"), None),
        (
            "POST",
            format!("/api/sessions/{id}/send"),
            Some(json!({"kind":"text","text":"intruder"})),
        ),
        ("POST", format!("/api/sessions/{id}/close"), None),
        ("DELETE", format!("/api/sessions/{id}"), None),
    ] {
        assert_eq!(
            call(&router, method, &path, Some(&second), body).await.0,
            StatusCode::NOT_FOUND
        );
    }
    let mut r = w["data"]["collections"][0]["requests"][0].clone();
    r["id"] = json!("absent");
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/sessions",
            Some(&first),
            Some(json!({"workspace_id":"w","request":r}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    r = w["data"]["collections"][0]["requests"][0].clone();
    r["post_response_script"] = json!("console.log('unavailable')");
    let (status, error) = call(
        &router,
        "POST",
        "/api/sessions",
        Some(&first),
        Some(json!({"workspace_id":"w","request":r})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error["error"].as_str().unwrap().contains("per-event"));
    assert_eq!(
        call(
            &router,
            "GET",
            "/api/workspaces/w/history",
            Some(&first),
            None
        )
        .await
        .1,
        json!([])
    );
    server.abort();
}
#[tokio::test]
async fn live_pre_scripts_and_private_metadata_use_isolated_worker_and_old_json_defaults_http() {
    let fixture = Router::new().route(
        "/events",
        get(|| async {
            (
                [("content-type", "text/event-stream")],
                Body::from_stream(
                    futures_util::stream::once(async {
                        Ok::<_, std::io::Error>("data: arrived\n\n")
                    })
                    .chain(futures_util::stream::pending()),
                ),
            )
        }),
    );
    let (url, server) = serve(fixture).await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("scripts.db")).await.unwrap();
    let mut d = session_data(&format!("{url}/events?key={{{{secret}}}}"), "sse");
    d["pre_request_script"] = json!(
        "pm.variables.set('secret', 'private-sse-token'); console.log('project private-sse-token');"
    );
    d["collections"][0]["pre_request_script"] = json!("console.log('collection');");
    d["collections"][0]["requests"][0]["pre_request_script"] =
        json!("pm.test('pre passed', ()=>pm.expect(true).to.equal(true));");
    let w = workspace(&router, None, d).await;
    let s = create(&router, None, &w).await;
    assert!(!s.to_string().contains("private-sse-token"));
    let id = s["id"].as_str().unwrap();
    wait(&router, None, id, "open").await;
    let (_, batch) = call(
        &router,
        "GET",
        &format!("/api/sessions/{id}/events"),
        None,
        None,
    )
    .await;
    assert!(!batch.to_string().contains("private-sse-token"));
    assert!(
        batch["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["message"]["kind"] == "script_log")
    );
    assert!(
        batch["events"].as_array().unwrap().iter().any(
            |e| e["message"]["kind"] == "script_test" && e["message"]["test"]["passed"] == true
        )
    );
    call(
        &router,
        "POST",
        &format!("/api/sessions/{id}/close"),
        None,
        None,
    )
    .await;
    let old: moleapi_core::RequestSpec =
        serde_json::from_value(example_data()["collections"][0]["requests"][0].clone()).unwrap();
    assert_eq!(old.protocol, moleapi_core::Protocol::Http);
    server.abort();
}
use futures_util::StreamExt;

#[tokio::test]
async fn deleting_saved_request_and_logging_out_terminate_real_connections() {
    let (peer_closed, mut closed) = tokio::sync::mpsc::channel(4);
    let fixture = Router::new().route(
        "/socket",
        get(move |upgrade: WebSocketUpgrade| {
            let peer_closed = peer_closed.clone();
            async move {
                upgrade.on_upgrade(move |mut socket| async move {
                    while let Some(Ok(message)) = socket.recv().await {
                        if matches!(message, Message::Close(_)) {
                            break;
                        }
                    }
                    peer_closed.send(()).await.unwrap();
                })
            }
        }),
    );
    let (url, server) = serve(fixture).await;
    let url = url.replacen("http", "ws", 1);
    let temp = tempfile::tempdir().unwrap();
    let native = local(&temp.path().join("native.db")).await.unwrap();
    let w = workspace(
        &native,
        None,
        session_data(&format!("{url}/socket"), "websocket"),
    )
    .await;
    let s = create(&native, None, &w).await;
    wait(&native, None, s["id"].as_str().unwrap(), "open").await;
    let mut d = w["data"].clone();
    d["collections"][0]["requests"] = json!([]);
    assert_eq!(
        call(
            &native,
            "PUT",
            "/api/workspaces/w",
            None,
            Some(json!({"name":"Removed request","data":d,"expected_revision":1}))
        )
        .await
        .0,
        StatusCode::OK
    );
    tokio::time::timeout(Duration::from_secs(2), closed.recv())
        .await
        .unwrap()
        .unwrap();
    let mut cfg = config(
        moleapi_server::sqlite_database_url(&temp.path().join("hosted.db")).unwrap(),
        false,
    );
    cfg.allow_private_network = true;
    let hosted = hosted(cfg).await.unwrap();
    let token = register(&hosted, "logout-test").await;
    let w = workspace(
        &hosted,
        Some(&token),
        session_data(&format!("{url}/socket"), "websocket"),
    )
    .await;
    let s = create(&hosted, Some(&token), &w).await;
    wait(&hosted, Some(&token), s["id"].as_str().unwrap(), "open").await;
    assert_eq!(
        call(&hosted, "POST", "/api/auth/logout", Some(&token), None)
            .await
            .0,
        StatusCode::OK
    );
    tokio::time::timeout(Duration::from_secs(2), closed.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        call(
            &hosted,
            "GET",
            &format!("/api/sessions/{}", s["id"].as_str().unwrap()),
            Some(&token),
            None
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    server.abort();
}

#[tokio::test]
async fn original_and_mutated_request_credentials_and_secret_query_never_leak_feedback() {
    let fixture = Router::new().route(
        "/events",
        get(|| async { ([("content-type", "text/event-stream")], "data: arrived\n\n") }),
    );
    let (url, server) = serve(fixture).await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("privacy.db")).await.unwrap();
    let mut d = session_data(&format!("{url}/events"), "sse");
    d["environments"] = json!([{"id":"env","name":"Environment","variables":[{"id":"credential","key":"credential","value":"resolved-original-credential","enabled":true}]}]);
    d["active_environment_id"] = json!("env");
    let w = workspace(&router, None, d).await;
    let mut r = w["data"]["collections"][0]["requests"][0].clone();
    r["headers"] = json!([{"id":"authorization","key":"authorization","value":"Bearer original-header-credential","enabled":true}]);
    r["pre_request_script"] = json!("throw new Error(pm.request.headers.get('authorization'));");
    let (status, error) = call(
        &router,
        "POST",
        "/api/sessions",
        None,
        Some(json!({"workspace_id":"w","request":r})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        !error.to_string().contains("original-header-credential"),
        "{error}"
    );
    r["pre_request_script"] = json!(
        "console.log(pm.request.headers.get('authorization')); pm.request.headers.remove('authorization'); pm.test('credential test',()=>pm.expect('original-header-credential').to.equal('different'));"
    );
    let (status, s) = call(
        &router,
        "POST",
        "/api/sessions",
        None,
        Some(json!({"workspace_id":"w","request":r})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{s}");
    let id = s["id"].as_str().unwrap();
    let (_, events) = call(
        &router,
        "GET",
        &format!("/api/sessions/{id}/events"),
        None,
        None,
    )
    .await;
    assert!(
        !events.to_string().contains("original-header-credential"),
        "{events}"
    );
    assert!(
        events["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["message"]["kind"] == "script_log")
    );
    call(
        &router,
        "DELETE",
        &format!("/api/sessions/{id}"),
        None,
        None,
    )
    .await;
    r["headers"] = json!([{"id":"authorization","key":"authorization","value":"Bearer {{credential}}","enabled":true}]);
    r["pre_request_script"] = json!(
        "console.log(pm.environment.get('credential')); pm.request.headers.remove('authorization');"
    );
    r["query"] = json!([{"id":"secret-query","key":"opaque","value":"query secret-value","enabled":true,"secret":true}]);
    let (status, s) = call(
        &router,
        "POST",
        "/api/sessions",
        None,
        Some(json!({"workspace_id":"w","request":r})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{s}");
    assert!(
        !s["url"].as_str().unwrap().contains("query+secret-value"),
        "{s}"
    );
    let id = s["id"].as_str().unwrap();
    let (_, events) = call(
        &router,
        "GET",
        &format!("/api/sessions/{id}/events"),
        None,
        None,
    )
    .await;
    assert!(
        !events.to_string().contains("resolved-original-credential"),
        "{events}"
    );
    call(
        &router,
        "DELETE",
        &format!("/api/sessions/{id}"),
        None,
        None,
    )
    .await;
    r["headers"] = json!([]);
    r["query"] = json!([]);
    r["pre_request_script"] = json!(
        "pm.request.headers.add({key:'authorization',value:'Bearer mutated-secret-credential'}); console.log(pm.request.headers.get('authorization')); pm.request.headers.remove('authorization');"
    );
    let (status, s) = call(
        &router,
        "POST",
        "/api/sessions",
        None,
        Some(json!({"workspace_id":"w","request":r})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{s}");
    let id = s["id"].as_str().unwrap();
    let (_, events) = call(
        &router,
        "GET",
        &format!("/api/sessions/{id}/events"),
        None,
        None,
    )
    .await;
    assert!(
        !events.to_string().contains("mutated-secret-credential"),
        "{events}"
    );
    call(
        &router,
        "DELETE",
        &format!("/api/sessions/{id}"),
        None,
        None,
    )
    .await;
    r["pre_request_script"] = json!(
        "pm.request.headers.add({key:'authorization',value:'Bearer mutated-failure-credential'}); throw new Error(pm.request.headers.get('authorization'));"
    );
    let (status, error) = call(
        &router,
        "POST",
        "/api/sessions",
        None,
        Some(json!({"workspace_id":"w","request":r})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        !error.to_string().contains("mutated-failure-credential"),
        "{error}"
    );
    server.abort();
}

#[tokio::test]
async fn logout_fences_inflight_preparation_for_hosted_and_native_admission() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let connections = Arc::new(AtomicUsize::new(0));
    let count = connections.clone();
    let fixture = Router::new().route(
        "/events",
        get(move || {
            let count = count.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                (
                    [("content-type", "text/event-stream")],
                    Body::from_stream(
                        futures_util::stream::once(async {
                            Ok::<_, std::io::Error>("data: arrived\n\n")
                        })
                        .chain(futures_util::stream::pending()),
                    ),
                )
            }
        }),
    );
    let (url, server) = serve(fixture).await;
    let temp = tempfile::tempdir().unwrap();
    for hosted_mode in [true, false] {
        let router = if hosted_mode {
            let mut cfg = config(
                moleapi_server::sqlite_database_url(&temp.path().join("hosted.db")).unwrap(),
                false,
            );
            cfg.allow_private_network = true;
            hosted(cfg).await.unwrap()
        } else {
            local(&temp.path().join("native.db")).await.unwrap()
        };
        let token = if hosted_mode {
            Some(register(&router, "logout-fence").await)
        } else {
            None
        };
        let w = workspace(
            &router,
            token.as_deref(),
            session_data(&format!("{url}/events"), "sse"),
        )
        .await;
        let mut r = w["data"]["collections"][0]["requests"][0].clone();
        r["pre_request_script"] = json!("const until=Date.now()+150; while(Date.now()<until) {};");
        let create_router = router.clone();
        let create_token = token.clone();
        let before = connections.load(Ordering::SeqCst);
        let creating = tokio::spawn(async move {
            call(
                &create_router,
                "POST",
                "/api/sessions",
                create_token.as_deref(),
                Some(json!({"workspace_id":"w","request":r})),
            )
            .await
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(
            call(&router, "POST", "/api/auth/logout", token.as_deref(), None)
                .await
                .0,
            StatusCode::OK
        );
        let (status, result) = creating.await.unwrap();
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{result}");
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert_eq!(connections.load(Ordering::SeqCst), before);
        let fresh_token = if hosted_mode {
            Some(
                call(
                    &router,
                    "POST",
                    "/api/auth/login",
                    None,
                    Some(json!({"username":"logout-fence","password":"goodpassword123"})),
                )
                .await
                .1["token"]
                    .as_str()
                    .unwrap()
                    .to_owned(),
            )
        } else {
            None
        };
        let s = create(&router, fresh_token.as_deref(), &w).await;
        let id = s["id"].as_str().unwrap();
        wait(&router, fresh_token.as_deref(), id, "open").await;
        call(
            &router,
            "DELETE",
            &format!("/api/sessions/{id}"),
            fresh_token.as_deref(),
            None,
        )
        .await;
    }
    server.abort();
}

#[tokio::test]
async fn retained_none_body_is_not_sent_and_script_method_or_body_mutations_are_rejected() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let connections = Arc::new(AtomicUsize::new(0));
    let sse_count = connections.clone();
    let ws_count = connections.clone();
    let fixture = Router::new()
        .route(
            "/events",
            get(move |request: axum::extract::Request| {
                let count = sse_count.clone();
                async move {
                    assert!(
                        axum::body::to_bytes(request.into_body(), 1024)
                            .await
                            .unwrap()
                            .is_empty()
                    );
                    count.fetch_add(1, Ordering::SeqCst);
                    (
                        [("content-type", "text/event-stream")],
                        "data: bodyless\n\n",
                    )
                }
            }),
        )
        .route(
            "/socket",
            get(
                move |upgrade: WebSocketUpgrade, request: axum::extract::Request| {
                    let count = ws_count.clone();
                    async move {
                        assert!(
                            axum::body::to_bytes(request.into_body(), 1024)
                                .await
                                .unwrap()
                                .is_empty()
                        );
                        count.fetch_add(1, Ordering::SeqCst);
                        upgrade.on_upgrade(|mut socket| async move {
                            while socket.recv().await.is_some() {}
                        })
                    }
                },
            ),
        );
    let (url, server) = serve(fixture).await;
    let temp = tempfile::tempdir().unwrap();
    for (kind, path) in [("sse", "events"), ("websocket", "socket")] {
        let endpoint = if kind == "websocket" {
            url.replacen("http", "ws", 1)
        } else {
            url.clone()
        };
        let router = local(&temp.path().join(format!("{kind}.db")))
            .await
            .unwrap();
        let mut d = session_data(&format!("{endpoint}/{path}"), kind);
        let draft = "{\"saved\":\"draft\",\"hidden\":\"{{unresolved-draft-variable}}\"}";
        d["collections"][0]["requests"][0]["body"] = json!(draft);
        let w = workspace(&router, None, d).await;
        let before = connections.load(Ordering::SeqCst);
        let s = create(&router, None, &w).await;
        let id = s["id"].as_str().unwrap();
        wait(
            &router,
            None,
            id,
            if kind == "sse" { "closed" } else { "open" },
        )
        .await;
        assert_eq!(connections.load(Ordering::SeqCst), before + 1);
        assert_eq!(
            call(&router, "GET", "/api/workspaces/w", None, None)
                .await
                .1["data"]["collections"][0]["requests"][0]["body"],
            draft
        );
        assert!(s.get("request_updates").is_none());
        call(
            &router,
            "DELETE",
            &format!("/api/sessions/{id}"),
            None,
            None,
        )
        .await;
        for script in [
            "pm.request.method='POST';",
            "pm.request.body.update({raw:'{\"wire\":\"forbidden\"}',mode:'json'});",
        ] {
            let mut r = w["data"]["collections"][0]["requests"][0].clone();
            r["pre_request_script"] = json!(script);
            let (status, error) = call(
                &router,
                "POST",
                "/api/sessions",
                None,
                Some(json!({"workspace_id":"w","request":r})),
            )
            .await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
            assert!(error["error"].as_str().unwrap().contains("GET"));
        }
        assert_eq!(connections.load(Ordering::SeqCst), before + 1);
    }
    server.abort();
}

#[tokio::test]
async fn templated_sensitive_keys_capture_original_and_intermediate_credentials() {
    let fixture = Router::new().route(
        "/events",
        get(|| async { ([("content-type", "text/event-stream")], "data: arrived\n\n") }),
    );
    let (url, server) = serve(fixture).await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("templated-privacy.db"))
        .await
        .unwrap();
    let mut d = session_data(&format!("{url}/events"), "sse");
    d["global_variables"] = json!([
        {"id":"header_name","key":"header_name","value":"Authorization","enabled":true},
        {"id":"query_name","key":"query_name","value":"access_token","enabled":true},
        {"id":"credential","key":"credential","value":"public-scope-template-credential","enabled":true},
        {"id":"base_url","key":"base_url","value":url,"enabled":true}
    ]);
    let w = workspace(&router, None, d).await;
    let base = w["data"]["collections"][0]["requests"][0].clone();
    let mut r = base.clone();
    r["headers"] = json!([{"id":"h","key":"{{header_name}}","value":"Bearer templated-key-private-credential","enabled":true}]);
    r["pre_request_script"] = json!(
        "console.log(pm.request.headers.get('{{header_name}}')); pm.request.headers.remove('{{header_name}}');"
    );
    for (mut request, private, script) in [
        (r, "templated-key-private-credential", None),
        (
            base.clone(),
            "intermediate-templated-key-credential",
            Some(
                "pm.request.headers.add({key:'{{header_name}}',value:'Bearer intermediate-templated-key-credential'}); console.log(pm.request.headers.get('{{header_name}}')); pm.request.headers.remove('{{header_name}}');",
            ),
        ),
        (
            base.clone(),
            "public-scope-template-credential",
            Some(
                "pm.request.headers.add({key:'{{header_name}}',value:'Bearer {{credential}}'}); console.log(pm.variables.get('credential')); pm.request.headers.remove('{{header_name}}');",
            ),
        ),
        (
            base.clone(),
            "changed-alias-credential",
            Some(
                "pm.variables.set('header_name','X-API-Key'); pm.request.headers.add({key:'{{header_name}}',value:'changed-alias-credential'}); console.log(pm.request.headers.get('{{header_name}}')); pm.request.headers.remove('{{header_name}}');",
            ),
        ),
        (
            base.clone(),
            "renamed-after-add-credential",
            Some(
                "pm.variables.set('header_name','X-Public'); pm.request.headers.add({key:'{{header_name}}',value:'renamed-after-add-credential'}); pm.variables.set('header_name','Authorization'); console.log('renamed-after-add-credential'); pm.request.headers.remove('{{header_name}}');",
            ),
        ),
    ] {
        if let Some(script) = script {
            request["pre_request_script"] = json!(script)
        }
        let (status, s) = call(
            &router,
            "POST",
            "/api/sessions",
            None,
            Some(json!({"workspace_id":"w","request":request})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{s}");
        let id = s["id"].as_str().unwrap();
        let (_, events) = call(
            &router,
            "GET",
            &format!("/api/sessions/{id}/events"),
            None,
            None,
        )
        .await;
        assert!(!events.to_string().contains(private), "{events}");
        assert!(
            events["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["message"]["kind"] == "script_log"
                    && e["message"]["message"] == "[REDACTED]")
        );
        call(
            &router,
            "DELETE",
            &format!("/api/sessions/{id}"),
            None,
            None,
        )
        .await;
    }
    let mut original_failure = base.clone();
    original_failure["headers"] = json!([{"id":"h","key":"{{header_name}}","value":"Bearer original-templated-literal-credential","enabled":true}]);
    for (mut request, private, script) in [
        (
            original_failure,
            "original-templated-literal-credential",
            "throw new Error('original-templated-literal-credential');",
        ),
        (
            base.clone(),
            "failed-templated-credential",
            "pm.request.headers.add({key:'{{header_name}}',value:'Bearer failed-templated-credential'}); throw new Error(pm.request.headers.get('{{header_name}}'));",
        ),
        (
            base.clone(),
            "query-aliased-credential",
            "throw new Error('query-aliased-credential');",
        ),
        (
            base.clone(),
            "url-query-aliased-credential",
            "throw new Error('url-query-aliased-credential');",
        ),
    ] {
        request["pre_request_script"] = json!(script);
        if private == "query-aliased-credential" {
            request["query"] =
                json!([{"id":"q","key":"{{query_name}}","value":private,"enabled":true}]);
        }
        if private == "url-query-aliased-credential" {
            request["url"] = json!(format!(
                "{{{{base_url}}}}/events?{{{{query_name}}}}={private}"
            ));
        }
        let (status, error) = call(
            &router,
            "POST",
            "/api/sessions",
            None,
            Some(json!({"workspace_id":"w","request":request})),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(!error.to_string().contains(private), "{error}");
    }
    server.abort();
}

#[tokio::test]
async fn scope_mutations_preserve_current_header_and_url_credentials_before_direct_throw() {
    let fixture = Router::new().route(
        "/events",
        get(|| async { ([("content-type", "text/event-stream")], "data: arrived\n\n") }),
    );
    let (url, server) = serve(fixture).await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("mutable-privacy.db"))
        .await
        .unwrap();
    let mut d = session_data(&format!("{url}/events"), "sse");
    d["global_variables"] = json!([
        {"id":"header_name","key":"header_name","value":"Authorization","enabled":true},
        {"id":"query_name","key":"query_name","value":"access_token","enabled":true},
        {"id":"credential","key":"credential","value":"url-value-alias-unset-credential","enabled":true}
    ]);
    d["global_variables"].as_array_mut().unwrap().extend([
        json!({"id":"query_row_alias","key":"query_row_alias","value":"opaque","enabled":true}),
        json!({"id":"auth_alias","key":"auth_alias","value":"auth-unset-credential","enabled":true})
    ]);
    d["environments"] = json!([{"id":"shadow","name":"Shadow","variables":[{"id":"auth_alias_shadow","key":"auth_alias","value":"initial-public-shadow","enabled":true}]}]);
    d["active_environment_id"] = json!("shadow");
    let w = workspace(&router, None, d).await;
    let base = w["data"]["collections"][0]["requests"][0].clone();
    for (secret,source) in [
        ("alias-before-throw-credential","pm.variables.set('header_name','X-Public'); pm.request.headers.add({key:'{{header_name}}',value:'Bearer alias-before-throw-credential'}); pm.variables.set('header_name','Authorization'); throw new Error('alias-before-throw-credential');".to_string()),
        ("sensitive-to-public-credential","pm.variables.set('header_name','X-Public'); pm.request.headers.add({key:'{{header_name}}',value:'Bearer sensitive-to-public-credential'}); pm.variables.set('header_name','Authorization'); pm.variables.set('header_name','X-Public'); throw new Error('sensitive-to-public-credential');".into()),
        ("unset-reveals-credential","pm.variables.set('header_name','X-Public'); pm.request.headers.add({key:'{{header_name}}',value:'Bearer unset-reveals-credential'}); pm.variables.unset('header_name'); throw new Error('unset-reveals-credential');".into()),
        ("clear-reveals-credential","pm.variables.set('header_name','X-Public'); pm.request.headers.add({key:'{{header_name}}',value:'Bearer clear-reveals-credential'}); pm.variables.clear(); throw new Error('clear-reveals-credential');".into()),
        ("url-alias-credential",format!("pm.variables.set('query_name','opaque'); pm.request.url='{url}/events?{{{{query_name}}}}=url-alias-credential'; pm.variables.set('query_name','access_token'); throw new Error('url-alias-credential');")),
        ("url-removed-credential",format!("pm.request.url='{url}/events?access_token=url-removed-credential'; pm.request.url='{url}/events'; throw new Error('url-removed-credential');")),
        ("url-public-credential",format!("pm.variables.set('query_name','access_token'); pm.request.url='{url}/events?{{{{query_name}}}}=url-public-credential'; pm.variables.set('query_name','opaque'); pm.request.url='{url}/events'; throw new Error('url-public-credential');")),
        ("url-unset-credential",format!("pm.variables.set('query_name','opaque'); pm.request.url='{url}/events?{{{{query_name}}}}=url-unset-credential'; pm.variables.unset('query_name'); throw new Error('url-unset-credential');")),
        ("url-value-alias-unset-credential",format!("pm.variables.set('credential','temporary-public'); pm.request.url='{url}/events?access_token={{{{credential}}}}'; pm.variables.unset('credential'); throw new Error('url-value-alias-unset-credential');")),
        ("userinfo-before-throw-credential",format!("pm.request.url='http://user:userinfo-before-throw-credential@{}/events'; pm.request.url='{url}/events'; throw new Error('userinfo-before-throw-credential');",url.trim_start_matches("http://"))),
    ] {
        let mut request=base.clone();request["pre_request_script"]=json!(source);
        let (status,error)=call(&router,"POST","/api/sessions",None,Some(json!({"workspace_id":"w","request":request}))).await;
        assert_eq!(status,StatusCode::BAD_REQUEST,"{error}");assert!(!error.to_string().contains(secret),"{error}");
    }
    for (mut request, secret, script) in [
        (
            base.clone(),
            "query-row-alias-credential",
            "pm.globals.set('query_row_alias','access_token'); throw new Error('query-row-alias-credential');",
        ),
        (
            base.clone(),
            "auth-unset-credential",
            "pm.environment.unset('auth_alias'); throw new Error('auth-unset-credential');",
        ),
    ] {
        request["pre_request_script"] = json!(script);
        if secret == "query-row-alias-credential" {
            request["query"] =
                json!([{"id":"query","key":"{{query_row_alias}}","value":secret,"enabled":true}]);
        } else {
            request["auth"] =
                json!({"kind":"bearer","token":"{{auth_alias}}","username":"","password":""});
        }
        let (status, error) = call(
            &router,
            "POST",
            "/api/sessions",
            None,
            Some(json!({"workspace_id":"w","request":request})),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(!error.to_string().contains(secret), "{error}");
    }
    let mut request = base.clone();
    request["pre_request_script"] = json!(
        "pm.request.headers.add({key:'{{initially_missing_alias}}',value:'Bearer prefilled-alias-credential'}); pm.variables.set('initially_missing_alias','Authorization'); console.log('prefilled-alias-credential');"
    );
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
    let (_, events) = call(
        &router,
        "GET",
        &format!("/api/sessions/{id}/events"),
        None,
        None,
    )
    .await;
    assert!(
        !events.to_string().contains("prefilled-alias-credential"),
        "{events}"
    );
    call(
        &router,
        "DELETE",
        &format!("/api/sessions/{id}"),
        None,
        None,
    )
    .await;
    let mut request = base.clone();
    request["query"] =
        json!([{"id":"public","key":"opaque","value":"ordinary-public-query","enabled":true}]);
    request["pre_request_script"] = json!(
        "pm.request.headers.add({key:'x-public',value:'ordinary-public-header'}); pm.variables.set('other','unrelated-transient'); console.log(pm.request.headers.get('x-public')); console.log('ordinary-public-query');"
    );
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
    let (_, events) = call(
        &router,
        "GET",
        &format!("/api/sessions/{id}/events"),
        None,
        None,
    )
    .await;
    for public in ["ordinary-public-header", "ordinary-public-query"] {
        assert!(events.to_string().contains(public), "{events}");
    }
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
