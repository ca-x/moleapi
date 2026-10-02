mod common;
use common::*;
#[tokio::test]
async fn execution_history_redacts_credentials_and_runner_counts_assertions() {
    let dir = tempfile::tempdir().unwrap();
    let router = moleapi_server::local(&dir.path().join("native.db"))
        .await
        .unwrap();
    let (url, server) = serve(Router::new().route(
        "/",
        axum::routing::get(|| async { ([("set-cookie", "private=1")], "{\"ok\":true}") }),
    ))
    .await;
    let mut workspace = example_data();
    let request = &mut workspace["collections"][0]["requests"][0];
    request["url"] = json!(format!("{url}/?api_key=private&lookup={{{{secret}}}}"));
    request["auth"]["kind"] = json!("bearer");
    request["auth"]["token"] = json!("auth-credential");
    request["assertions"] =
        json!([{"id":"a","name":"status","kind":"status","target":"","expected":"200"}]);
    workspace["environments"] = json!([{"id":"env","name":"Env","variables":[{"id":"v","key":"secret","value":"secret-value","enabled":true,"secret":true}]}]);
    workspace["active_environment_id"] = json!("env");
    let request = workspace["collections"][0]["requests"][0].clone();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"history","name":"History","data":workspace}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, response) = call(
        &router,
        "POST",
        "/api/execute",
        None,
        Some(json!({"workspace_id":"history","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["tests"][0]["passed"], true);
    let (_, history) = call(
        &router,
        "GET",
        "/api/workspaces/history/history",
        None,
        None,
    )
    .await;
    let text = history.to_string();
    assert!(!text.contains("auth-credential"));
    assert!(!text.contains("secret-value"));
    assert!(!text.contains("private=1"));
    assert!(!text.contains("api_key=private"));
    assert_eq!(history.as_array().unwrap().len(), 1);
    let (_, run) = call(
        &router,
        "POST",
        "/api/workspaces/history/run",
        None,
        Some(json!({"collection_id":"c"})),
    )
    .await;
    assert_eq!(run["passed"], 1);
    assert_eq!(run["failed"], 0);
    assert_eq!(
        call(
            &router,
            "DELETE",
            "/api/workspaces/history/history",
            None,
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &router,
            "GET",
            "/api/workspaces/history/history",
            None,
            None
        )
        .await
        .1,
        json!([])
    );
    server.abort();
}
