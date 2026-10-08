mod common;
use common::*;
#[tokio::test]
async fn parent_environment_edgegrid_credentials_and_generated_headers_are_private() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("edgegrid.db")).await.unwrap();
    let(provider,task)=serve(Router::new().route("/echo",axum::routing::get(|headers:axum::http::HeaderMap|async move{axum::Json(json!({"auth":headers["authorization"].to_str().unwrap(),"copied_secret":"private-edgegrid-secret"}))}))).await;
    let mut source = example_data();
    source["collections"][0]["auth"] = json!({"kind":"edgegrid","token":"","username":"","password":"","edgegrid":{"access_token":"{{access}}","client_token":"{{client}}","client_secret":"{{key}}"}});
    source["global_variables"] = json!([{"id":"a","key":"access","value":"private-edgegrid-access","enabled":true},{"id":"c","key":"client","value":"private-edgegrid-client","enabled":true},{"id":"k","key":"key","value":"private-edgegrid-secret","enabled":true}]);
    source["collections"][0]["requests"][0]["url"] = json!(format!("{provider}/echo"));
    source["collections"][0]["requests"][0]["auth"]["kind"] = json!("inherit");
    source["collections"][0]["requests"][0]["assertions"] = json!([]);
    let (status, w) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"edgegrid","name":"EdgeGrid","data":source})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    let (status, result) = call(
        &router,
        "POST",
        "/api/execute",
        None,
        Some(
            json!({"workspace_id":"edgegrid","request":w["data"]["collections"][0]["requests"][0]}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["status"], 200);
    let (_, history) = call(
        &router,
        "GET",
        "/api/workspaces/edgegrid/history",
        None,
        None,
    )
    .await;
    for value in [
        "private-edgegrid-access",
        "private-edgegrid-client",
        "private-edgegrid-secret",
    ] {
        assert!(!history.to_string().contains(value), "{history}");
    }
    task.abort();
}
