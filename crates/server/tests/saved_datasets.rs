mod common;
use base64::{Engine, engine::general_purpose::STANDARD};
use common::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
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

#[tokio::test]
async fn query_dataset_refreshes_once_per_run_and_failure_sends_no_collection_requests() {
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::get(
            move |axum::extract::Query(value): axum::extract::Query<
                std::collections::HashMap<String, String>,
            >| {
                let count = count.clone();
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    axum::Json(json!(value))
                }
            },
        ),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let mut settings = config(
        format!(
            "sqlite://{}?mode=rwc",
            temp.path().join("live.db").display()
        ),
        true,
    );
    settings.allow_private_network = true;
    let router = moleapi_server::hosted_with_worker(
        settings,
        std::path::Path::new(env!("CARGO_BIN_EXE_moleapi-server")),
    )
    .await
    .unwrap();
    let owner = register(&router, "live-owner").await;
    let other = register(&router, "live-other").await;
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = format!("{url}/?value={{{{value}}}}").into();
    let mut query = data["collections"][0]["requests"][0].clone();
    query["id"] = "data-query".into();
    query["url"] = "".into();
    query["method"] = "GET".into();
    query["body_kind"] = "none".into();
    query["body"] = "".into();
    query["protocol"] = json!({"kind":"data","source":"local_file","file_name":"rows.csv","file_format":"csv","file_base64":STANDARD.encode(b"value\ndataset-secret-one\ndataset-secret-two\n"),"table_name":"data","sql":"SELECT value FROM data","read_only":true});
    data["collections"].as_array_mut().unwrap().push(
        json!({"id":"data-source","name":"Data sources","description":"","requests":[query]}),
    );
    data["datasets"] = json!([{"id":"fresh","name":"Fresh","query":{"request_id":"data-query"}}]);
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Live","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let run = json!({"collection_id":"c","dataset_id":"fresh"});
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
    let (status, first) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        Some(&owner),
        Some(run.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["passed"], 2, "{first}");
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    let first_value: Value =
        serde_json::from_str(first["results"][0]["response"]["body"].as_str().unwrap()).unwrap();
    assert_eq!(first_value["value"], "dataset-secret-one");
    let (status, persisted) = call(
        &router,
        "GET",
        &format!(
            "/api/workspaces/w/reports/{}",
            first["report_id"].as_str().unwrap()
        ),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{persisted}");
    assert!(!persisted.to_string().contains("dataset-secret-one"));
    assert!(!persisted.to_string().contains("dataset-secret-two"));

    let (_, mut current) = call(&router, "GET", "/api/workspaces/w", Some(&owner), None).await;
    let revision = current["revision"].clone();
    current["data"]["collections"][1]["requests"][0]["protocol"]["file_base64"] =
        STANDARD.encode(b"value\ndataset-secret-three\n").into();
    let (status, saved) = call(
        &router,
        "PUT",
        "/api/workspaces/w",
        Some(&owner),
        Some(json!({"name":"Live","expected_revision":revision,"data":current["data"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (status, second) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        Some(&owner),
        Some(run.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{second}");
    assert_eq!(second["passed"], 1, "{second}");
    let second_value: Value =
        serde_json::from_str(second["results"][0]["response"]["body"].as_str().unwrap()).unwrap();
    assert_eq!(second_value["value"], "dataset-secret-three");
    assert_eq!(calls.load(Ordering::SeqCst), 3);

    let (_, mut current) = call(&router, "GET", "/api/workspaces/w", Some(&owner), None).await;
    let revision = current["revision"].clone();
    let mut csv = String::from("value\n");
    for index in 0..100 {
        csv.push_str(&format!("dataset-secret-{index}\n"));
    }
    current["data"]["collections"][1]["requests"][0]["protocol"]["file_base64"] =
        STANDARD.encode(csv.as_bytes()).into();
    let request = current["data"]["collections"][0]["requests"][0].clone();
    for index in 0..10 {
        let mut extra = request.clone();
        extra["id"] = format!("extra-{index}").into();
        current["data"]["collections"][0]["requests"]
            .as_array_mut()
            .unwrap()
            .push(extra);
    }
    let (status, saved) = call(
        &router,
        "PUT",
        "/api/workspaces/w",
        Some(&owner),
        Some(json!({"name":"Live","expected_revision":revision,"data":current["data"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (status, over_limit) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        Some(&owner),
        Some(run.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{over_limit}");
    assert_eq!(over_limit["stopped_reason"], "dataset_execution_limit");
    assert_eq!(over_limit["executed_steps"], 0);
    assert!(over_limit["report_id"].is_string());
    assert_eq!(calls.load(Ordering::SeqCst), 3);

    let (_, mut current) = call(&router, "GET", "/api/workspaces/w", Some(&owner), None).await;
    let revision = current["revision"].clone();
    current["data"]["collections"][1]["requests"][0]["protocol"]["sql"] =
        "SELECT missing FROM data".into();
    let (status, saved) = call(
        &router,
        "PUT",
        "/api/workspaces/w",
        Some(&owner),
        Some(json!({"name":"Live","expected_revision":revision,"data":current["data"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (status, failed) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        Some(&owner),
        Some(run),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{failed}");
    assert_eq!(failed["stopped_reason"], "dataset_refresh_failed");
    assert_eq!(failed["executed_steps"], 0);
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    let (status, report) = call(
        &router,
        "GET",
        &format!(
            "/api/workspaces/w/reports/{}",
            failed["report_id"].as_str().unwrap()
        ),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(
        report["summary"]["stopped_reason"],
        "dataset_refresh_failed"
    );
    assert_eq!(report["summary"]["executed_steps"], 0);
    fixture.abort();
}

#[tokio::test]
async fn cancelling_a_slow_query_dataset_refresh_stops_before_collection_dispatch() {
    let started = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let entered = started.clone();
    let resume = release.clone();
    let (source_url, source_fixture) = serve(Router::new().route(
        "/source",
        axum::routing::get(move || {
            let entered = entered.clone();
            let resume = resume.clone();
            async move {
                entered.notify_one();
                resume.notified().await;
                "value\nlate\n"
            }
        }),
    ))
    .await;
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let (request_url, request_fixture) = serve(Router::new().route(
        "/",
        axum::routing::get(move || {
            let counter = counter.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                "unexpected"
            }
        }),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let router = moleapi_server::local_with_worker(
        &temp.path().join("cancel.db"),
        std::path::Path::new(env!("CARGO_BIN_EXE_moleapi-server")),
    )
    .await
    .unwrap();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = format!("{request_url}/").into();
    let mut query = data["collections"][0]["requests"][0].clone();
    query["id"] = "remote-query".into();
    query["url"] = format!("{source_url}/source").into();
    query["protocol"] = json!({"kind":"data","source":"remote_file","file_name":"rows.csv","file_format":"csv","table_name":"data","sql":"SELECT value FROM data","read_only":true});
    data["collections"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"sources","name":"Sources","description":"","requests":[query]}));
    data["datasets"] = json!([{"id":"fresh","name":"Fresh","query":{"request_id":"remote-query"}}]);
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"Cancel","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let running = {
        let router = router.clone();
        tokio::spawn(async move {
            call(
                &router,
                "POST",
                "/api/workspaces/w/run",
                None,
                Some(json!({"collection_id":"c","dataset_id":"fresh","job_id":"refresh-cancel"})),
            )
            .await
        })
    };
    tokio::time::timeout(std::time::Duration::from_secs(5), started.notified())
        .await
        .unwrap();
    let (status, cancelled) = call(
        &router,
        "POST",
        "/api/workspaces/w/run/cancel",
        None,
        Some(json!({"job_id":"refresh-cancel"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{cancelled}");
    let (status, report) = tokio::time::timeout(std::time::Duration::from_secs(5), running)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["stopped_reason"], "dataset_refresh_failed");
    assert_eq!(report["cancelled"], true);
    assert_eq!(report["executed_steps"], 0);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    release.notify_waiters();
    source_fixture.abort();
    request_fixture.abort();
}
