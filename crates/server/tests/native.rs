mod common;
use common::*;
#[tokio::test]
async fn native_router_is_offline_api_with_private_database_permissions() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("native ?#%.db");
    let router = moleapi_server::local(&path).await.unwrap();
    assert_eq!(
        call(&router, "GET", "/api/auth/status", None, None).await.1["mode"],
        "desktop"
    );
    assert_eq!(
        call(&router, "GET", "/api/workspaces", None, None).await.0,
        StatusCode::OK
    );
    assert_eq!(
        call(&router, "GET", "/some-page", None, None).await.0,
        StatusCode::NOT_FOUND
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
#[tokio::test]
async fn all_api_errors_are_json_and_hosted_native_endpoints_are_missing() {
    let dir = tempfile::tempdir().unwrap();
    let native = moleapi_server::local(&dir.path().join("native.db"))
        .await
        .unwrap();
    let (status, value) = call(
        &native,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"name":123})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(value["error"].is_string());
    let hosted = moleapi_server::hosted(config(
        format!(
            "sqlite://{}?mode=rwc",
            dir.path().join("hosted.db").display()
        ),
        true,
    ))
    .await
    .unwrap();
    for path in [
        "/api/sync/status",
        "/api/sync/connect",
        "/api/workspaces/x/sync",
    ] {
        let method = if path.ends_with("status") {
            "GET"
        } else {
            "POST"
        };
        assert_eq!(
            call(&hosted, method, path, None, Some(json!({}))).await.0,
            StatusCode::NOT_FOUND
        );
    }
}
#[cfg(feature = "web")]
#[tokio::test]
async fn hosted_embedded_assets_support_spa_paths_while_api_never_falls_back() {
    let dir = tempfile::tempdir().unwrap();
    let router = moleapi_server::hosted(config(
        format!(
            "sqlite://{}?mode=rwc",
            dir.path().join("hosted.db").display()
        ),
        false,
    ))
    .await
    .unwrap();
    for path in ["/", "/workspace/view"] {
        let response = router
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            response.headers()["content-type"]
                .to_str()
                .unwrap()
                .starts_with("text/html")
        );
        let bytes = to_bytes(response.into_body(), MAX_UI_BYTES).await.unwrap();
        assert!(String::from_utf8_lossy(&bytes).contains("<!doctype html>"));
    }
    assert_eq!(
        call(&router, "GET", "/api/does-not-exist", None, None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}
#[cfg(feature = "web")]
const MAX_UI_BYTES: usize = 5 * 1024 * 1024;
