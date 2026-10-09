mod common;
use common::*;
#[tokio::test]
async fn saved_sources_are_owned_and_used_by_runner_without_mutating_data() {
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::get(
            |axum::extract::Query(value): axum::extract::Query<
                std::collections::HashMap<String, String>,
            >| async move { axum::Json(json!(value)) },
        ),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let mut settings = config(
        format!(
            "sqlite://{}?mode=rwc",
            temp.path().join("saved.db").display()
        ),
        true,
    );
    settings.allow_private_network = true;
    let router = hosted(settings).await.unwrap();
    let owner = register(&router, "dataset-owner").await;
    let other = register(&router, "dataset-other").await;
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = format!("{url}/?value={{{{value}}}}").into();
    let source = "value\nfirst-row\nsecond-row\n";
    data["datasets"] = json!([{"id":"d","name":"Saved CSV","source":{"format":"csv","source":source}},{"id":"missing","name":"Missing source"}]);
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Saved datasets","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let run = json!({"collection_id":"c","dataset_id":"d"});
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces/w/run",
            Some(&other),
            Some(run.clone())
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, result) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        Some(&owner),
        Some(run.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["passed"], 2);
    let body: Value =
        serde_json::from_str(result["results"][1]["response"]["body"].as_str().unwrap()).unwrap();
    assert_eq!(body["value"], "second-row");
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces/w/run",
            Some(&owner),
            Some(json!({"collection_id":"c","dataset_id":"missing"}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(call(&router,"POST","/api/workspaces/w/run",Some(&owner),Some(json!({"collection_id":"c","dataset_id":"d","dataset":{"format":"json","source":"[{\"id\":1}]"}}))).await.0,StatusCode::BAD_REQUEST);
    let (_, current) = call(&router, "GET", "/api/workspaces/w", Some(&owner), None).await;
    assert_eq!(current["data"], saved["data"]);
    assert_eq!(current["data"]["datasets"][0]["source"]["source"], source);
    let (status,updated)=call(&router,"PUT","/api/workspaces/w",Some(&owner),Some(json!({"name":"Saved datasets","expected_revision":saved["revision"],"data":current["data"]}))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(call(&router,"PUT","/api/workspaces/w",Some(&owner),Some(json!({"name":"Conflict","expected_revision":saved["revision"],"data":updated["data"]}))).await.0,StatusCode::CONFLICT);
    fixture.abort();
}
