mod common;
use common::*;

#[tokio::test]
async fn v2_dataset_queries_wrap_real_request_and_keep_private_rows_out_of_history() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("script-datasets.db")).await.unwrap();
    let (url, server) = serve(Router::new().route(
        "/",
        axum::routing::get(|headers: axum::http::HeaderMap| async move {
            assert_eq!(headers["x-dataset"], "private-row");
            "response"
        }),
    ))
    .await;
    let mut data = example_data();
    data["script_mode"] = json!("v2");
    data["datasets"] = json!([{"id":"saved","name":"Saved","table_name":"rows","secret":true,"source":{"format":"json","source":"[{\"token\":\"private-row\",\"flag\":true}]"}}]);
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = json!(format!("{url}/"));
    request["pre_request_script"] = json!(
        "const result=await pm.datasets('saved').executeQuery('SELECT token FROM rows WHERE flag = ?', ['1']); for await (const row of result.rows) pm.request.headers.upsert({key:'x-dataset',value:row.token});"
    );
    request["post_response_script"] = json!(
        "const result=await pm.datasets('saved').executeQuery('SELECT flag FROM rows'); for await (const row of result.rows) pm.test('boolean row',()=>pm.expect(row.flag).to.equal(true));"
    );
    let request = request.clone();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"W","data":data}))
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
        Some(json!({"workspace_id":"w","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert!(
        response["tests"]
            .as_array()
            .unwrap()
            .iter()
            .any(|test| test["name"] == "boolean row" && test["passed"] == true),
        "{response}"
    );
    let (_, history) = call(&router, "GET", "/api/workspaces/w/history", None, None).await;
    assert!(!history.to_string().contains("private-row"), "{history}");
    let mut rejected = request.clone();
    rejected["pre_request_script"] = json!(
        "const result=await pm.datasets('saved').executeQuery('SELECT token FROM rows'); for await (const row of result.rows) throw new Error(row.token);"
    );
    let (status, error) = call(
        &router,
        "POST",
        "/api/execute",
        None,
        Some(json!({"workspace_id":"w","request":rejected})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
    assert!(!error.to_string().contains("private-row"), "{error}");
    server.abort();
}

#[tokio::test]
async fn dataset_script_snapshot_is_owned_and_missing_sources_fail_before_request() {
    let dir = tempfile::tempdir().unwrap();
    let (url, server) = serve(Router::new().route(
        "/",
        axum::routing::get(|headers: axum::http::HeaderMap| async move {
            assert_eq!(headers["x-owner-dataset"], "private-owner");
            "ok"
        }),
    ))
    .await;
    let mut cfg = config(
        format!(
            "sqlite://{}?mode=rwc",
            dir.path().join("hosted.db").display()
        ),
        true,
    );
    cfg.allow_private_network = true;
    let hosted = hosted(cfg).await.unwrap();
    let alice = register(&hosted, "datasetalice").await;
    let bob = register(&hosted, "datasetbobby").await;
    let mut data = example_data();
    data["script_mode"] = json!("v2");
    data["datasets"] = json!([{"id":"saved","name":"Private","secret":true,"source":{"format":"json","source":"[{\"token\":\"private-owner\"}]"}}]);
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = json!(format!("{url}/"));
    request["pre_request_script"] = json!(
        "const result=await pm.datasets('saved').executeQuery('SELECT token FROM data'); for await (const row of result.rows) pm.request.headers.upsert({key:'x-owner-dataset',value:row.token});"
    );
    let request = request.clone();
    let (status, result) = call(
        &hosted,
        "POST",
        "/api/workspaces",
        Some(&alice),
        Some(json!({"id":"w","name":"W","data":data.clone()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(
        call(
            &hosted,
            "POST",
            "/api/execute",
            Some(&bob),
            Some(json!({"workspace_id":"w","request":request}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, result) = call(
        &hosted,
        "POST",
        "/api/execute",
        Some(&alice),
        Some(json!({"workspace_id":"w","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    data["datasets"][0]
        .as_object_mut()
        .unwrap()
        .remove("source");
    let (status, result) = call(
        &hosted,
        "PUT",
        "/api/workspaces/w",
        Some(&alice),
        Some(json!({"name":"W","data":data,"expected_revision":1})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let (status, error) = call(
        &hosted,
        "POST",
        "/api/execute",
        Some(&alice),
        Some(json!({"workspace_id":"w","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
    assert!(
        error.to_string().contains("snapshot unavailable"),
        "{error}"
    );
    server.abort();
}
