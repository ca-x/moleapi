mod common;
use common::*;
#[cfg(unix)]
#[tokio::test]
async fn assertion_worker_timeout_returns_failed_checks_and_reaps_the_process() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let worker = temp.path().join("timeout-worker");
    std::fs::write(&worker, "#!/bin/sh\nexec /bin/sleep 30\n").unwrap();
    std::fs::set_permissions(&worker, std::fs::Permissions::from_mode(0o755)).unwrap();
    let response:moleapi_core::Response=serde_json::from_value(json!({"status":200,"status_text":"OK","headers":[],"body":"{}","elapsed_ms":1,"size_bytes":2,"truncated":false,"url":"https://example.test","tests":[]})).unwrap();
    let checks = vec![moleapi_core::Assertion {
        id: "a".into(),
        name: "Timeout".into(),
        kind: "schema".into(),
        target: String::new(),
        expected: "{}".into(),
    }];
    let start = std::time::Instant::now();
    let result = moleapi_core::assertion_worker(&worker, &checks, &response).await;
    assert!(start.elapsed() < std::time::Duration::from_secs(5));
    assert_eq!(result.len(), 1);
    assert!(!result[0].passed);
    assert!(result[0].actual.contains("timed out"));
}
#[tokio::test]
async fn network_response_checks_share_bounded_worker_with_collection_runs() {
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::get(|| async {
            axum::Json(json!({"items":[{"id":3},{"id":7}],"value":"id=42"}))
        }),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("checks.db")).await.unwrap();
    let mut data = example_data();
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = format!("{url}/").into();
    request["assertions"] = json!([{"id":"path","name":"Path","kind":"jsonpath","target":"$.items[*].id","expected":"[3,7]"},{"id":"regex","name":"Regex","kind":"regex","target":"id=(\\d+)","expected":"42"},{"id":"schema","name":"Schema","kind":"schema","target":"","expected":"{\"type\":\"object\",\"required\":[\"items\"]}"}]);
    let (status, w) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"Checks","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    let (status, response) = call(
        &router,
        "POST",
        "/api/execute",
        None,
        Some(json!({"workspace_id":"w","request":w["data"]["collections"][0]["requests"][0]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["tests"].as_array().unwrap().len(), 3);
    assert!(
        response["tests"]
            .as_array()
            .unwrap()
            .iter()
            .all(|test| test["passed"] == true)
    );
    let (status, run) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        None,
        Some(json!({"collection_id":"c"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{run}");
    assert_eq!(run["passed"], 1);
    fixture.abort();
}
