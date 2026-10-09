mod common;
use common::*;
use std::sync::Arc;
use tokio::sync::Notify;
#[tokio::test]
async fn folder_requests_inherit_parent_mutations_for_each_dataset_iteration() {
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::get(
            |axum::extract::Query(query): axum::extract::Query<
                std::collections::HashMap<String, String>,
            >| async move { axum::Json(json!(query)) },
        ),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("hierarchy.db")).await.unwrap();
    let mut data = example_data();
    let root = &mut data["collections"][0];
    root["variables"] = json!([{"id":"chain","key":"chain","value":"initial","enabled":true}]);
    root["requests"][0]["url"] = format!("{url}/").into();
    root["requests"][0]["post_response_script"] =
        "pm.collectionVariables.set('chain','parent-'+pm.iterationData.get('value'));".into();
    let mut child = root["requests"][0].clone();
    child["id"] = "child-request".into();
    child["url"] = format!("{url}/?chain={{{{chain}}}}").into();
    child["post_response_script"] = "".into();
    data["collections"].as_array_mut().unwrap().push(
        json!({"id":"child","parent_id":"c","name":"Folder","description":"","requests":[child]}),
    );
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"Hierarchy","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, result) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        None,
        Some(json!({"collection_id":"c","dataset":{"format":"csv","source":"value\none\ntwo\n"}})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["passed"], 4);
    for (index, expected) in [(1, "parent-one"), (3, "parent-two")] {
        assert_eq!(result["results"][index]["collection_id"], "child");
        let body: Value = serde_json::from_str(
            result["results"][index]["response"]["body"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(body["chain"], expected);
    }
    fixture.abort();
}
#[tokio::test]
async fn later_dataset_rows_are_prevalidated_before_any_network_request() {
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = calls.clone();
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::get(move || {
            let counter = counter.clone();
            async move {
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                "ok"
            }
        }),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("preflight.db")).await.unwrap();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = format!("{url}/").into();
    data["global_variables"] =
        json!([{"id":"big","key":"big","value":"x".repeat(960*1024),"enabled":true}]);
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"Preflight","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let source =
        json!([{"id":1},{"id":2,"extra1":"a".repeat(60*1024),"extra2":"b".repeat(60*1024)}])
            .to_string();
    let (status, _) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        None,
        Some(json!({"collection_id":"c","dataset":{"format":"json","source":source}})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    fixture.abort();
}
#[tokio::test]
async fn report_limit_omits_response_details_without_losing_counts_or_requests() {
    let (url, fixture) = serve(Router::new().route(
        "/large",
        axum::routing::get(|| async { "x".repeat(3 * 1024 * 1024) }),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("report.db")).await.unwrap();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = format!("{url}/large").into();
    data["collections"][0]["requests"][0]["timeout_ms"] = 10000.into();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"Report limit","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, result) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        None,
        Some(json!({"collection_id":"c","iterations":3})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{status}");
    assert_eq!(result["passed"], 3);
    assert_eq!(result["results"].as_array().unwrap().len(), 3);
    assert_eq!(result["omitted_responses"], 1);
    assert_eq!(result["results"][2]["response_omitted"], true);
    assert!(serde_json::to_vec(&result).unwrap().len() <= 8 * 1024 * 1024);
    fixture.abort();
}

#[tokio::test]
async fn typed_dataset_iterations_chain_variables_and_keep_nested_values_out_of_history() {
    let (url,fixture)=serve(Router::new().route("/echo/{id}",axum::routing::get(|axum::extract::Path(id):axum::extract::Path<String>,headers:axum::http::HeaderMap|async move{axum::Json(json!({"id":id,"counter":headers.get("x-counter").unwrap().to_str().unwrap(),"secret":headers.get("x-private-copy").unwrap().to_str().unwrap()}))}))).await;
    let temp = tempfile::tempdir().unwrap();
    let mut settings = config(
        format!(
            "sqlite://{}?mode=rwc",
            temp.path().join("data-runner.db").display()
        ),
        true,
    );
    settings.allow_private_network = true;
    let router = hosted(settings).await.unwrap();
    let owner = register(&router, "data-owner").await;
    let other = register(&router, "data-other").await;
    let mut data = example_data();
    data["collections"][0]["variables"] =
        json!([{"id":"counter","key":"counter","value":"0","enabled":true}]);
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = format!("{url}/echo/{{{{id}}}}").into();
    request["pre_request_script"]=r#"pm.request.headers.upsert({key:'X-Counter',value:pm.collectionVariables.get('counter')});pm.request.headers.upsert({key:'X-Private-Copy',value:pm.iterationData.get('auth').secret});pm.test('typed data',()=>pm.expect(typeof pm.iterationData.get('id')).to.equal('number'));"#.into();
    request["post_response_script"]=r#"pm.test('iteration response',()=>pm.expect(pm.response.json().id).to.equal(String(pm.iterationData.get('id'))));pm.collectionVariables.set('counter',String(Number(pm.collectionVariables.get('counter'))+1));"#.into();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            Some(&owner),
            Some(json!({"id":"w","name":"Data run","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, baseline) = call(&router, "GET", "/api/workspaces/w", Some(&owner), None).await;
    let source = r#"[{"id":10,"auth":{"secret":"nested-private-one"}},{"id":20,"auth":{"secret":"nested-private-two"}}]"#;
    let preview = json!({"workspace_id":"w","dataset":{"format":"json","source":source}});
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/testing/dataset/preview",
            Some(&other),
            Some(preview.clone())
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, parsed) = call(
        &router,
        "POST",
        "/api/testing/dataset/preview",
        Some(&owner),
        Some(preview),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(parsed["rows"][0]["id"], 10);
    let (status,result)=call(&router,"POST","/api/workspaces/w/run",Some(&owner),Some(json!({"collection_id":"c","job_id":"typed-data-run","dataset":{"format":"json","source":source}}))).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["passed"], 2);
    assert_eq!(result["failed"], 0);
    assert_eq!(result["completed_iterations"], 2);
    assert_eq!(result["results"][1]["iteration"], 1);
    let first: Value =
        serde_json::from_str(result["results"][0]["response"]["body"].as_str().unwrap()).unwrap();
    let second: Value =
        serde_json::from_str(result["results"][1]["response"]["body"].as_str().unwrap()).unwrap();
    assert_eq!(first["counter"], "0");
    assert_eq!(second["counter"], "1");
    let (_, history) = call(
        &router,
        "GET",
        "/api/workspaces/w/history",
        Some(&owner),
        None,
    )
    .await;
    assert!(
        !history.to_string().contains("nested-private-one")
            && !history.to_string().contains("nested-private-two")
    );
    let (_, saved) = call(&router, "GET", "/api/workspaces/w", Some(&owner), None).await;
    assert_eq!(saved["data"], baseline["data"]);
    fixture.abort();
}
#[tokio::test]
async fn cancellation_stops_inflight_requests_and_prestart_cancellation_is_owned() {
    let started = Arc::new(Notify::new());
    let signal = started.clone();
    let (url, fixture) = serve(Router::new().route(
        "/wait",
        axum::routing::get(move || {
            let signal = signal.clone();
            async move {
                signal.notify_one();
                std::future::pending::<String>().await
            }
        }),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("cancel.db")).await.unwrap();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = format!("{url}/wait").into();
    data["collections"][0]["requests"][0]["timeout_ms"] = 30000.into();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"Cancel","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let running = router.clone();
    let task = tokio::spawn(async move {
        call(
            &running,
            "POST",
            "/api/workspaces/w/run",
            None,
            Some(json!({"collection_id":"c","iterations":3,"job_id":"cancel-run"})),
        )
        .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), started.notified())
        .await
        .unwrap();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces/w/run/cancel",
            None,
            Some(json!({"job_id":"cancel-run"}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, result) = tokio::time::timeout(std::time::Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["cancelled"], true);
    assert_eq!(result["results"].as_array().unwrap().len(), 0);
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces/w/run/cancel",
            None,
            Some(json!({"job_id":"early-run"}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, result) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        None,
        Some(json!({"collection_id":"c","job_id":"early-run"})),
    )
    .await;
    assert_eq!(result["cancelled"], true);
    assert_eq!(result["passed"], 0);
    fixture.abort();
}
