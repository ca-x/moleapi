mod common;
use common::*;
#[tokio::test]
async fn saved_mock_example_uses_status_body_and_only_safe_headers() {
    let dir = tempfile::tempdir().unwrap();
    let router = common::local(&dir.path().join("local.db")).await.unwrap();
    call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"mock","name":"Mock","data":example_data()})),
    )
    .await;
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/mock/mock/r/e")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.headers()["x-example"], "yes");
    assert!(!response.headers().contains_key("connection"));
    assert!(!response.headers().contains_key("x-hop"));
    assert_eq!(
        to_bytes(response.into_body(), 100).await.unwrap().as_ref(),
        b"example body"
    );
    assert_eq!(
        call(&router, "GET", "/api/mock/mock/r/missing", None, None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn saved_error_status_example_retains_body_and_headers() {
    let directory = tempfile::tempdir().unwrap();
    let router = common::local(&directory.path().join("errors.db"))
        .await
        .unwrap();
    let mut payload = example_data();
    payload["collections"][0]["requests"][0]["examples"][0]["status"] = json!(404);
    payload["collections"][0]["requests"][0]["examples"][0]["body"] = json!("custom mock failure");
    let (status, _) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"mock-error","name":"Errors","data":payload})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/mock/mock-error/r/e")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(response.headers().get("x-example").unwrap(), "yes");
    assert_eq!(
        to_bytes(response.into_body(), 1000).await.unwrap().as_ref(),
        b"custom mock failure"
    );
}
