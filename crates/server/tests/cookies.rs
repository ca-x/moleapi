mod common;
use common::*;
#[tokio::test]
async fn cookie_management_environment_isolation_execution_history_and_export_privacy() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("cookies.db")).await.unwrap();
    let (base, server) = serve(Router::new().route(
        "/",
        axum::routing::get(|headers: axum::http::HeaderMap| async move {
            headers
                .get("cookie")
                .and_then(|h| h.to_str().ok())
                .unwrap_or("missing")
                .to_owned()
        }),
    ))
    .await;
    let mut data = example_data();
    data["environments"] = json!([{"id":"dev","name":"Development","variables":[]},{"id":"prod","name":"Production","variables":[]}]);
    data["active_environment_id"] = json!("dev");
    data["collections"][0]["requests"][0]["url"] = json!(format!("{base}/"));
    data["collections"][0]["requests"][0]["assertions"] = json!([]);
    let (status, w) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"cookie","name":"Cookies","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    let path = "/api/workspaces/cookie/cookies?environment_id=dev";
    let (status, result) = call(&router, "PATCH", path, None, Some(json!({"enabled":true}))).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let (status, result) = call(
        &router,
        "POST",
        path,
        None,
        Some(json!({"url":base,"cookie":"sid=private-cookie-value; Path=/; HttpOnly"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["cookies"][0]["value"], "[REDACTED]");
    let (status, result) = call(
        &router,
        "GET",
        "/api/workspaces/cookie/cookies?environment_id=dev&reveal=true",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["cookies"][0]["value"], "private-cookie-value");
    let raw = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/workspaces/cookie/cookies?environment_id=dev&reveal=true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(raw.headers()["cache-control"], "no-store");
    for environment in ["dev", "prod"] {
        let (status,response)=call(&router,"POST","/api/execute",None,Some(json!({"workspace_id":"cookie","environment_id":environment,"request":data["collections"][0]["requests"][0]}))).await;
        assert_eq!(status, StatusCode::OK, "{response}");
        assert_eq!(
            response["body"],
            if environment == "dev" {
                "sid=private-cookie-value"
            } else {
                "missing"
            }
        );
    }
    let (status, history) =
        call(&router, "GET", "/api/workspaces/cookie/history", None, None).await;
    assert_eq!(status, StatusCode::OK, "{history}");
    assert!(
        !history.to_string().contains("private-cookie-value"),
        "{history}"
    );
    let (_, w) = call(&router, "GET", "/api/workspaces/cookie", None, None).await;
    assert!(!w.to_string().contains("private-cookie-value"));
    let (status, result) = call(
        &router,
        "GET",
        "/api/workspaces/cookie/cookies?environment_id=missing",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{result}");
    let (status, result) = call(
        &router,
        "DELETE",
        path,
        None,
        Some(json!({"domain":"127.0.0.1","path":"/","name":"sid"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["cookies"], json!([]));
    server.abort();
}
#[tokio::test]
async fn cookie_owner_isolation_and_logout_clear() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = config(
        format!(
            "sqlite://{}?mode=rwc",
            dir.path().join("hosted.db").display()
        ),
        true,
    );
    config.allow_private_network = true;
    let router = hosted(config).await.unwrap();
    let first = register(&router, "cookiefirst").await;
    let second = register(&router, "cookiesecond").await;
    for token in [&first, &second] {
        let (status, w) = call(
            &router,
            "POST",
            "/api/workspaces",
            Some(token),
            Some(json!({"id":"same","name":"Cookies","data":data()})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{w}");
    }
    let path = "/api/workspaces/same/cookies";
    let (status, result) = call(
        &router,
        "POST",
        path,
        Some(&first),
        Some(json!({"url":"https://example.com/","cookie":"private=first-owner; Path=/"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let (_, result) = call(&router, "GET", path, Some(&second), None).await;
    assert_eq!(result["cookies"], json!([]));
    let (status, result) = call(&router, "POST", "/api/auth/logout", Some(&first), None).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let (status, result) = call(
        &router,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({"username":"cookiefirst","password":"goodpassword123"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let (_, result) = call(&router, "GET", path, result["token"].as_str(), None).await;
    assert_eq!(result["cookies"], json!([]));
}
