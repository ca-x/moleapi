mod common;
use base64::{Engine, engine::general_purpose::STANDARD};
use common::*;
#[tokio::test]
async fn native_execution_and_runner_send_uploaded_bytes_but_history_remains_live_only() {
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("bodies.db")).await.unwrap();
    let (url, server) = serve(Router::new().route(
        "/",
        axum::routing::post(|body: axum::body::Bytes| async move {
            axum::Json(json!({"bytes":STANDARD.encode(body)}))
        }),
    ))
    .await;
    let mut data = example_data();
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = json!(url);
    request["method"] = json!("POST");
    request["body_kind"] = json!("binary");
    request["body"] = json!(
        json!({"file_name":"fixture.bin","mime":"application/octet-stream","base64":"AP9B"})
            .to_string()
    );
    request["assertions"] = json!([]);
    request["examples"] = json!([]);
    let request = request.clone();
    let (status, value) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"bodies","name":"Body fixture","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    let (status, result) = call(
        &router,
        "POST",
        "/api/execute",
        None,
        Some(json!({"workspace_id":"bodies","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert!(result["body"].as_str().unwrap().contains("AP9B"));
    let (status, history) =
        call(&router, "GET", "/api/workspaces/bodies/history", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!history.to_string().contains("AP9B"));
    assert!(history.to_string().contains("live-only"));
    let (status, run) = call(
        &router,
        "POST",
        "/api/workspaces/bodies/run",
        None,
        Some(json!({"collection_id":"c"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{run}");
    assert_eq!(run["failed"], 0);
    server.abort();
}
