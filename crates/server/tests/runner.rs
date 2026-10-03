mod common;
use common::*;
#[tokio::test]
async fn runner_records_assertion_failures_and_request_errors_then_continues() {
    let dir = tempfile::tempdir().unwrap();
    let router = common::local(&dir.path().join("local.db")).await.unwrap();
    let (url, server) =
        serve(Router::new().route("/", axum::routing::get(|| async { "response" }))).await;
    let mut data = example_data();
    let mut good = data["collections"][0]["requests"][0].clone();
    good["url"] = json!(format!("{url}/"));
    let mut failed = good.clone();
    failed["id"] = json!("failed");
    failed["assertions"] =
        json!([{"id":"a","name":"wrong status","kind":"status","target":"","expected":"500"}]);
    let mut invalid = good.clone();
    invalid["id"] = json!("invalid");
    invalid["url"] = json!("{{missing}}/endpoint");
    data["collections"][0]["requests"] = json!([failed, invalid, good]);
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"runner","name":"Runner","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, result) = call(
        &router,
        "POST",
        "/api/workspaces/runner/run",
        None,
        Some(json!({"collection_id":"c"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["passed"], 1);
    assert_eq!(result["failed"], 2);
    assert_eq!(
        result["results"][0]["response"]["tests"][0]["passed"],
        false
    );
    assert!(
        result["results"][1]["error"]
            .as_str()
            .unwrap()
            .contains("variable")
    );
    assert_eq!(result["results"][2]["response"]["status"], 200);
    let (_, history) = call(&router, "GET", "/api/workspaces/runner/history", None, None).await;
    assert_eq!(history.as_array().unwrap().len(), 2);
    server.abort();
}
