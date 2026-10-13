mod common;
use axum::{Json, http::HeaderMap, routing::get};
use common::*;

#[tokio::test]
async fn native_gateway_uses_saved_login_only_for_exact_hosted_management_routes() {
    let upstream = Router::new()
        .route(
            "/api/auth/login",
            axum::routing::post(|| async { Json(json!({"token":"private-hosted-session"})) }),
        )
        .route(
            "/api/runners",
            get(|headers: HeaderMap| async move {
                assert_eq!(
                    headers.get("authorization").unwrap(),
                    "Bearer private-hosted-session"
                );
                Json(json!([{"id":"runner","name":"LAN","labels":["lan"],"enabled":true}]))
            })
            .post(|headers: HeaderMap, Json(body): Json<Value>| async move {
                assert_eq!(
                    headers.get("authorization").unwrap(),
                    "Bearer private-hosted-session"
                );
                Json(json!({"id":"runner","name":body["name"]}))
            }),
        )
        .route(
            "/api/runners/runner",
            axum::routing::patch(|| async {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"error":"private-hosted-session"})),
                )
            }),
        )
        .route(
            "/api/workspaces/w",
            get(|| async {
                Json(json!({"id":"w","name":"Remote","revision":9,"data":example_data()}))
            }),
        );
    let (server, task) = serve(upstream).await;
    let temporary = tempfile::tempdir().unwrap();
    let local = moleapi_server::local(&temporary.path().join("native.db"))
        .await
        .unwrap();
    assert_eq!(
        call(&local, "GET", "/api/hosted/runners", None, None)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let (status, connected) = call(
        &local,
        "POST",
        "/api/sync/connect",
        None,
        Some(json!({"server_url":server,"username":"owner","password":"fixture-password-123"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{connected}");
    assert_eq!(connected["connected"], true);
    assert!(
        connected["session_id"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    assert!(!connected.to_string().contains("private-hosted-session"));
    let (status, runners) = call(&local, "GET", "/api/hosted/runners", None, None).await;
    assert_eq!(status, StatusCode::OK, "{runners}");
    assert_eq!(runners[0]["id"], "runner");
    let (status, created) = call(
        &local,
        "POST",
        "/api/hosted/runners",
        None,
        Some(json!({"name":"LAN"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let (_, remote) = call(&local, "GET", "/api/hosted/workspaces/w", None, None).await;
    assert_eq!(remote["revision"], 9);
    for (method, path) in [
        ("POST", "/api/hosted/runners/runner/claim"),
        ("GET", "/api/hosted/auth/tokens"),
        ("GET", "/api/hosted/runners?unexpected=1"),
        ("GET", "/api/hosted/workspaces/w/runner-tasks/task/complete"),
    ] {
        assert_eq!(
            call(&local, method, path, None, None).await.0,
            StatusCode::NOT_FOUND
        );
    }
    let (status, error) = call(
        &local,
        "PATCH",
        "/api/hosted/runners/runner",
        None,
        Some(json!({"name":"LAN","enabled":true,"expected_revision":1})),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(!error.to_string().contains("private-hosted-session"));
    assert_eq!(
        call(&local, "DELETE", "/api/sync/connect", None, None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&local, "GET", "/api/hosted/runners", None, None)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    task.abort();
}

#[tokio::test]
async fn hosted_server_does_not_expose_the_native_gateway() {
    let temporary = tempfile::tempdir().unwrap();
    let url = format!(
        "sqlite://{}?mode=rwc",
        temporary.path().join("hosted.db").display()
    );
    let router = hosted(config(url, true)).await.unwrap();
    let owner = register(&router, "nativegatewayowner").await;
    assert_eq!(
        call(&router, "GET", "/api/hosted/runners", Some(&owner), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}
