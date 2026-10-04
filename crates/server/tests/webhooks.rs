mod common;
use base64::{Engine, engine::general_purpose::STANDARD};
use common::*;
use futures_util::StreamExt;
async fn workspace(router: &Router, token: Option<&str>) -> Value {
    let (status, w) = call(
        router,
        "POST",
        "/api/workspaces",
        token,
        Some(json!({"id":"w","name":"Webhooks","data":data()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    w
}
async fn inbox(router: &Router, token: Option<&str>, response: Value) -> Value {
    let (status, i) = call(
        router,
        "POST",
        "/api/workspaces/w/webhooks",
        token,
        Some(json!({"name":"Receiver","response":response})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{i}");
    i
}
async fn ingest(
    router: &Router,
    path: &str,
    bytes: &[u8],
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let request = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .header("authorization", "Bearer private-header")
        .header("x-copy", "private-header")
        .body(Body::from(bytes.to_vec()))
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), 2097152)
        .await
        .unwrap()
        .to_vec();
    (status, headers, bytes)
}
#[tokio::test]
async fn persistent_callback_response_and_default_full_capture_privacy() {
    let temp = tempfile::tempdir().unwrap();
    let db = temp.path().join("webhooks.db");
    let router = local(&db).await.unwrap();
    workspace(&router, None).await;
    let i=inbox(&router,None,json!({"status":422,"body":"custom failure","headers":[{"id":"c","key":"content-type","value":"text/html","enabled":true}]})).await;
    let path = format!(
        "{}?token=private-query&copy=private-query",
        i["receiver_path"].as_str().unwrap()
    );
    let body = format!(
        "{{\"password\":\"only-in-body\",\"copy\":\"only-in-body / private-header / {}\"}}",
        i["token"].as_str().unwrap()
    );
    let (status, headers, bytes) = ingest(&router, &path, body.as_bytes()).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(bytes, b"custom failure");
    assert!(
        headers["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("sandbox")
    );
    assert_eq!(headers["x-content-type-options"], "nosniff");
    let id = i["id"].as_str().unwrap();
    let (status, safe) = call(
        &router,
        "GET",
        &format!("/api/webhooks/{id}/captures"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{safe}");
    assert_eq!(safe["captures"].as_array().unwrap().len(), 1);
    for secret in [
        "private-header",
        "private-query",
        "only-in-body",
        i["token"].as_str().unwrap(),
    ] {
        assert!(
            !safe.to_string().contains(secret),
            "leaked {secret}: {safe}"
        );
    }
    let (status, full) = call(
        &router,
        "GET",
        &format!("/api/webhooks/{id}/captures?include_secrets=true"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        STANDARD
            .decode(full["captures"][0]["body_base64"].as_str().unwrap())
            .unwrap(),
        body.as_bytes()
    );
    drop(router);
    let restarted = local(&db).await.unwrap();
    let (status, loaded) = call(
        &restarted,
        "GET",
        &format!("/api/webhooks/{id}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(loaded["receiver_path"], i["receiver_path"]);
    assert_eq!(loaded["received"], 1);
}
#[tokio::test]
async fn paused_receivers_forbid_callbacks_and_workspace_delete_cascades() {
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("pause.db")).await.unwrap();
    workspace(&router, None).await;
    let i = inbox(&router, None, json!({})).await;
    let id = i["id"].as_str().unwrap();
    let path = i["receiver_path"].as_str().unwrap();
    assert_eq!(ingest(&router, path, b"x").await.0, StatusCode::OK);
    let (_, current) = call(&router, "GET", &format!("/api/webhooks/{id}"), None, None).await;
    let(status,paused)=call(&router,"PUT",&format!("/api/webhooks/{id}"),None,Some(json!({"name":"Paused","active":false,"response":{},"expected_revision":current["revision"]}))).await;
    assert_eq!(status, StatusCode::OK, "{paused}");
    assert_eq!(
        ingest(&router, path, b"late").await.0,
        StatusCode::NOT_FOUND
    );
    let (status, _) = call(
        &router,
        "DELETE",
        "/api/workspaces/w",
        None,
        Some(json!({"expected_revision":1})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        ingest(&router, path, b"deleted").await.0,
        StatusCode::NOT_FOUND
    );
    let (status, _) = call(
        &router,
        "GET",
        &format!("/api/webhooks/{id}/captures?include_secrets=true"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
#[tokio::test]
async fn foreign_owner_cannot_discover_secret_url_or_private_capture() {
    let temp = tempfile::tempdir().unwrap();
    let router = hosted(config(
        format!(
            "sqlite://{}?mode=rwc",
            temp.path().join("owned.db").display()
        ),
        true,
    ))
    .await
    .unwrap();
    let owner = register(&router, "owner").await;
    let foreign = register(&router, "foreign").await;
    workspace(&router, Some(&owner)).await;
    let i = inbox(&router, Some(&owner), json!({})).await;
    let id = i["id"].as_str().unwrap();
    let path = i["receiver_path"].as_str().unwrap();
    assert_eq!(ingest(&router, path, b"owned").await.0, StatusCode::OK);
    for uri in [
        format!("/api/webhooks/{id}"),
        format!("/api/webhooks/{id}/captures?include_secrets=true"),
    ] {
        let (status, _) = call(&router, "GET", &uri, Some(&foreign), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}
#[tokio::test]
async fn independent_native_bridge_is_explicit_and_stops() {
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("native.db")).await.unwrap();
    workspace(&router, None).await;
    let i = inbox(&router, None, json!({})).await;
    let (status, s) = call(&router, "GET", "/api/webhooks/listener", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(s["active"], false);
    let (status, s) = call(
        &router,
        "POST",
        "/api/webhooks/listener/start",
        None,
        Some(json!({"host":"127.0.0.1","port":0})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{s}");
    let url = format!(
        "{}{}",
        s["origin"].as_str().unwrap(),
        i["receiver_path"].as_str().unwrap()
    );
    let response = reqwest::Client::new()
        .post(&url)
        .body(vec![0, 1, 255, 65])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let (_, captured) = call(
        &router,
        "GET",
        &format!(
            "/api/webhooks/{}/captures?include_secrets=true",
            i["id"].as_str().unwrap()
        ),
        None,
        None,
    )
    .await;
    assert_eq!(
        STANDARD
            .decode(captured["captures"][0]["body_base64"].as_str().unwrap())
            .unwrap(),
        [0, 1, 255, 65]
    );
    let (status, _) = call(
        &router,
        "POST",
        "/api/webhooks/listener/stop",
        None,
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(reqwest::Client::new().post(&url).send().await.is_err());
}

#[tokio::test]
async fn pause_during_slow_body_fences_late_capture_commit() {
    use futures_util::stream;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("late.db")).await.unwrap();
    workspace(&router, None).await;
    let i = inbox(&router, None, json!({})).await;
    let id = i["id"].as_str().unwrap().to_owned();
    let path = i["receiver_path"].as_str().unwrap().to_owned();
    let (started, ready) = tokio::sync::oneshot::channel();
    let (release, wait) = tokio::sync::oneshot::channel();
    let chunks = stream::once(async move {
        let _ = started.send(());
        Ok::<_, std::io::Error>(bytes::Bytes::from_static(b"a"))
    })
    .chain(stream::once(async move {
        let _ = wait.await;
        Ok::<_, std::io::Error>(bytes::Bytes::from_static(b"b"))
    }));
    let request = Request::builder()
        .method("POST")
        .uri(path)
        .body(Body::from_stream(chunks))
        .unwrap();
    let pending = tokio::spawn(router.clone().oneshot(request));
    ready.await.unwrap();
    let (status, _) = call(
        &router,
        "PUT",
        &format!("/api/webhooks/{id}"),
        None,
        Some(json!({"name":"Paused","active":false,"response":{},"expected_revision":1})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _ = release.send(());
    assert_eq!(
        pending.await.unwrap().unwrap().status(),
        StatusCode::NOT_FOUND
    );
    let (_, batch) = call(
        &router,
        "GET",
        &format!("/api/webhooks/{id}/captures?include_secrets=true"),
        None,
        None,
    )
    .await;
    assert!(batch["captures"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn edited_binary_replay_uses_checked_target_and_excludes_credentials_by_default() {
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("replay.db")).await.unwrap();
    workspace(&router, None).await;
    let i = inbox(&router, None, json!({})).await;
    let id = i["id"].as_str().unwrap();
    ingest(&router, i["receiver_path"].as_str().unwrap(), b"capture").await;
    let (_, batch) = call(
        &router,
        "GET",
        &format!("/api/webhooks/{id}/captures?include_secrets=true"),
        None,
        None,
    )
    .await;
    let capture_id = batch["captures"][0]["id"].as_str().unwrap();
    let fixture = axum::Router::new().route(
        "/edited",
        axum::routing::post(|request: axum::extract::Request| async move {
            assert!(request.headers().get("authorization").is_none());
            assert_eq!(request.headers()["x-edit"], "yes");
            assert_eq!(request.uri().query(), Some("public=one"));
            let bytes = to_bytes(request.into_body(), 100).await.unwrap();
            assert_eq!(bytes.as_ref(), [0, 1, 255, 65]);
            (StatusCode::CREATED, "actual replay")
        }),
    );
    let (url, server) = serve(fixture).await;
    let(status,result)=call(&router,"POST",&format!("/api/webhooks/{id}/replay"),None,Some(json!({"capture_id":capture_id,"replay_id":"edited","destination":format!("{url}/edited"),"method":"POST","headers":[{"id":"a","key":"Authorization","value":"Bearer must-not-send","enabled":true},{"id":"e","key":"X-Edit","value":"yes","enabled":true}],"query":[{"id":"q","key":"public","value":"one","enabled":true},{"id":"p","key":"token","value":"hidden","enabled":true}],"body_base64":"AAH/QQ==","timeout_ms":5000,"verify_tls":true,"follow_redirects":false,"include_credentials":false}))).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["status"], 201);
    assert_eq!(result["body"], "actual replay");
    server.abort();
}

#[tokio::test]
async fn safe_export_screens_response_copies_and_retention_keeps_stable_cursors() {
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("export.db")).await.unwrap();
    workspace(&router, None).await;
    let i = inbox(
        &router,
        None,
        json!({"status":200,"headers":[
        {"id":"a","key":"Authorization","value":"Bearer response-private","enabled":true},
        {"id":"b","key":"X-Copy","value":"response-private","enabled":true}],
        "body":"{\"password\":\"body-private\",\"copy\":\"body-private response-private\"}"}),
    )
    .await;
    let id = i["id"].as_str().unwrap();
    let path = i["receiver_path"].as_str().unwrap();
    let (status, _) = call(&router, "PUT", &format!("/api/webhooks/{id}"), None,
        Some(json!({"name":"Receiver response-private","active":true,"response":i["response"],"expected_revision":1}))).await;
    assert_eq!(status, StatusCode::OK);
    for n in 0..67 {
        assert_eq!(
            ingest(&router, path, n.to_string().as_bytes()).await.0,
            StatusCode::OK
        );
    }
    let (status, batch) = call(
        &router,
        "GET",
        &format!("/api/webhooks/{id}/captures?after=64"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(batch["captures"].as_array().unwrap().len(), 3);
    assert_eq!(batch["captures"][0]["cursor"], 65);
    assert_eq!(batch["received"], 67);
    assert_eq!(batch["dropped"], 3);
    for full in [false, true] {
        let (status, export) = call(
            &router,
            "POST",
            &format!("/api/webhooks/{id}/export"),
            None,
            Some(json!({"include_secrets":full})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{export}");
        let content = export["content"].as_str().unwrap();
        for secret in [
            "response-private",
            "body-private",
            i["token"].as_str().unwrap(),
        ] {
            assert_eq!(content.contains(secret), full, "{content}");
        }
        let value: Value = serde_json::from_str(content).unwrap();
        assert_eq!(value["captures"].as_array().unwrap().len(), 64);
        assert_eq!(value["captures"][0]["cursor"], 4);
    }
}

#[tokio::test]
async fn concurrent_callbacks_preserve_all_receipts_and_enforce_limits() {
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("limits.db")).await.unwrap();
    workspace(&router, None).await;
    let i = inbox(&router, None, json!({})).await;
    let path = i["receiver_path"].as_str().unwrap();
    let calls = (0..12).map(|_| ingest(&router, path, b"parallel"));
    for (status, _, _) in futures_util::future::join_all(calls).await {
        assert_eq!(status, StatusCode::OK);
    }
    let (_, batch) = call(
        &router,
        "GET",
        &format!("/api/webhooks/{}/captures", i["id"].as_str().unwrap()),
        None,
        None,
    )
    .await;
    assert_eq!(batch["received"], 12);
    assert_eq!(batch["captures"].as_array().unwrap().len(), 12);
    for (n, capture) in batch["captures"].as_array().unwrap().iter().enumerate() {
        assert_eq!(capture["cursor"], n + 1);
    }
    assert_eq!(
        ingest(&router, path, &vec![0; 1048577]).await.0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    for _ in 0..3 {
        inbox(&router, None, json!({})).await;
    }
    let (status, _) = call(
        &router,
        "POST",
        "/api/workspaces/w/webhooks",
        None,
        Some(json!({"name":"exceeds"})),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn deleting_receiver_cancels_inflight_replay_and_url_credentials_are_excluded() {
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("cancel.db")).await.unwrap();
    workspace(&router, None).await;
    let i = inbox(&router, None, json!({})).await;
    let id = i["id"].as_str().unwrap().to_owned();
    ingest(&router, i["receiver_path"].as_str().unwrap(), b"source").await;
    let (_, batch) = call(
        &router,
        "GET",
        &format!("/api/webhooks/{id}/captures"),
        None,
        None,
    )
    .await;
    let capture_id = batch["captures"][0]["id"].clone();
    let started = std::sync::Arc::new(tokio::sync::Notify::new());
    let notified = started.clone();
    let fixture = Router::new().route(
        "/slow",
        axum::routing::post(move |request: axum::extract::Request| {
            let started = started.clone();
            async move {
                assert!(request.headers().get("authorization").is_none());
                assert_eq!(request.uri().query(), Some("public=ok"));
                started.notify_one();
                std::future::pending::<()>().await;
                "unreachable"
            }
        }),
    );
    let (url, server) = serve(fixture).await;
    let destination =
        url.replace("http://", "http://private:password@") + "/slow?token=hidden&public=ok";
    let replay_router = router.clone();
    let replay_id = id.clone();
    let pending = tokio::spawn(async move {
        call(&replay_router,"POST",&format!("/api/webhooks/{replay_id}/replay"),None,Some(json!({"capture_id":capture_id,"replay_id":"cancel","destination":destination,"method":"POST","headers":[],"query":[],"body_base64":"","timeout_ms":10000,"verify_tls":true,"follow_redirects":false}))).await
    });
    tokio::time::timeout(std::time::Duration::from_secs(3), notified.notified())
        .await
        .unwrap();
    let (_, current) = call(&router, "GET", &format!("/api/webhooks/{id}"), None, None).await;
    let (status, _) = call(
        &router,
        "DELETE",
        &format!("/api/webhooks/{id}"),
        None,
        Some(json!({"expected_revision":current["revision"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, error) = tokio::time::timeout(std::time::Duration::from_secs(3), pending)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error["error"].as_str().unwrap().contains("cancelled"));
    server.abort();
}
