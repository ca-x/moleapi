mod common;
use axum::{
    extract::{OriginalUri, ws::WebSocketUpgrade},
    http::HeaderMap,
    response::IntoResponse,
    routing::get,
};
use common::*;
use futures_util::StreamExt;
use std::time::Duration;

#[tokio::test]
async fn member_authored_live_requests_preserve_settings_and_use_canonical_handshakes() {
    let (seen, mut wire) = tokio::sync::mpsc::channel(4);
    let ws_seen = seen.clone();
    let fixture = Router::new()
        .route("/socket", get(move |upgrade: WebSocketUpgrade, OriginalUri(uri): OriginalUri, headers: HeaderMap| {
            let seen = ws_seen.clone();
            async move {
                seen.send(json!({"uri":uri.to_string(),"auth":headers["authorization"].to_str().unwrap(),"request":headers["x-request"].to_str().unwrap(),"script":headers["x-pre"].to_str().unwrap()})).await.unwrap();
                let mut response = upgrade.on_upgrade(|mut socket| async move { while let Some(Ok(message)) = socket.recv().await { if socket.send(message).await.is_err() { break; } } }).into_response();
                response.headers_mut().insert("x-fixture", "shared-websocket".parse().unwrap());
                response
            }
        }))
        .route("/events", get(move |OriginalUri(uri): OriginalUri, headers: HeaderMap| {
            let seen = seen.clone();
            async move {
                seen.send(json!({"uri":uri.to_string(),"auth":headers["authorization"].to_str().unwrap(),"request":headers["x-request"].to_str().unwrap(),"script":headers["x-pre"].to_str().unwrap()})).await.unwrap();
                ([("content-type","text/event-stream"),("x-fixture","shared-sse")],Body::from_stream(futures_util::stream::once(async { Ok::<_,std::io::Error>("data: authored-event\n\n") }).chain(futures_util::stream::pending())))
            }
        }));
    let (url, fixture) = serve(fixture).await;
    let dir = tempfile::tempdir().unwrap();
    let mut settings = config(
        moleapi_server::sqlite_database_url(&dir.path().join("authoring.db")).unwrap(),
        true,
    );
    settings.allow_private_network = true;
    let router = hosted(settings).await.unwrap();
    let owner = register(&router, "authorowner").await;
    let editor = register(&router, "authoreditor").await;
    let mut source = example_data();
    let template = source["collections"][0]["requests"][0].clone();
    source["collections"][0]["requests"] = json!([]);
    source["collections"][0]["auth"] =
        json!({"kind":"bearer","token":"owner-key","username":"","password":""});
    source["global_variables"] =
        json!([{"id":"region","key":"region","value":"owner-region","enabled":true}]);
    source["script_mode"] = "v2".into();
    let (status, mut saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Live authoring","data":source})),
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
    let (status, accepted) = call(
        &router,
        "POST",
        "/api/workspace-invitations/accept",
        Some(&editor),
        Some(json!({"token":invite["token"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{accepted}");
    let base = format!(
        "/api/owners/{}/workspaces/w",
        accepted["owner_id"].as_str().unwrap()
    );
    for kind in ["websocket", "sse"] {
        let mut request = template.clone();
        request["id"] = kind.into();
        request["name"] = format!("Authored {kind}").into();
        request["examples"] = json!([]);
        request["url"] = if kind == "websocket" {
            format!("{}/socket", url.replace("http:", "ws:"))
        } else {
            format!("{url}/events")
        }
        .into();
        request["method"] = "GET".into();
        request["protocol"] = json!({"kind":kind});
        request["auth"]["kind"] = "inherit".into();
        request["query"] = json!([{"id":"q","key":"region","value":"{{region}}","enabled":true}]);
        request["headers"] =
            json!([{"id":"h","key":"x-request","value":"{{region}}","enabled":true}]);
        request["pre_request_script"] =
            "pm.request.headers.add({key:'X-Pre',value:'member-script'});".into();
        request["timeout_ms"] = 17000.into();
        request["verify_tls"] = false.into();
        request["network"] = serde_json::to_value(moleapi_core::RequestNetwork::default()).unwrap();
        saved["data"]["collections"][0]["requests"]
            .as_array_mut()
            .unwrap()
            .push(request.clone());
        let (status, updated) = call(&router,"PUT",&base,Some(&editor),Some(json!({"name":saved["name"],"data":saved["data"],"expected_revision":saved["revision"]}))).await;
        assert_eq!(status, StatusCode::OK, "{updated}");
        saved = updated;
        let retained = saved["data"]["collections"][0]["requests"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == kind)
            .unwrap();
        for field in [
            "method",
            "protocol",
            "query",
            "headers",
            "pre_request_script",
            "timeout_ms",
            "verify_tls",
            "network",
        ] {
            assert_eq!(retained[field], request[field], "{field}");
        }
        let (status, opened) = call(
            &router,
            "POST",
            &format!("{base}/sessions"),
            Some(&editor),
            Some(json!({"request_id":kind,"expected_revision":saved["revision"]})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{opened}");
        let observed = tokio::time::timeout(Duration::from_secs(5), wire.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(observed["auth"], "Bearer owner-key");
        assert_eq!(observed["request"], "owner-region");
        assert_eq!(observed["script"], "member-script");
        assert_eq!(
            observed["uri"],
            if kind == "sse" {
                "/events?region=owner-region"
            } else {
                "/socket?region=owner-region"
            }
        );
        let path = format!("{base}/sessions/{}", opened["id"].as_str().unwrap());
        let summary = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let (status, summary) = call(&router, "GET", &path, Some(&editor), None).await;
                assert_eq!(status, StatusCode::OK, "{summary}");
                if summary["handshake"].is_object() {
                    break summary;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            summary["handshake"]["status"],
            if kind == "sse" { 200 } else { 101 }
        );
        assert!(
            summary["handshake"]["headers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|h| h["key"] == "x-fixture" && h["value"] == format!("shared-{kind}"))
        );
        assert!(!summary.to_string().contains("owner-key"));
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
    fixture.abort();
}
