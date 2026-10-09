mod common;
use common::*;
use std::sync::Arc;
fn block() -> Value {
    json!([{"id":"block","name":"Concurrent","step_ids":["a","b"],"concurrency":2}])
}
#[tokio::test]
async fn real_parallel_barrier_merges_equal_writes_and_masks_peer_history() {
    let barrier = Arc::new(tokio::sync::Barrier::new(2));
    let a = barrier.clone();
    let b = barrier.clone();
    let (url,fixture)=serve(Router::new().route("/a",axum::routing::get(move||{let barrier=a.clone();async move{barrier.wait().await;tokio::time::sleep(std::time::Duration::from_millis(40)).await;axum::Json(json!({"token":"parallel-private-token"}))}})).route("/b",axum::routing::get(move||{let barrier=b.clone();async move{barrier.wait().await;axum::Json(json!({"copy":"parallel-private-token"}))}})).route("/echo",axum::routing::get(|headers:axum::http::HeaderMap|async move{axum::Json(json!({"token":headers.get("x-token").unwrap().to_str().unwrap(),"counter":headers.get("x-counter").unwrap().to_str().unwrap()}))}))).await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("parallel.db")).await.unwrap();
    let mut data = example_data();
    data["environments"] = json!([{"id":"dev","name":"Dev","variables":[{"id":"count","key":"counter","value":"0","enabled":true}]}]);
    let mut first = data["collections"][0]["requests"][0].clone();
    first["id"] = "first".into();
    first["url"] = format!("{url}/a").into();
    first["pre_request_script"]="pm.test('snapshot',()=>pm.expect(pm.environment.get('counter')).to.equal('0'));pm.environment.set('counter','1');".into();
    first["extractions"] = json!([{"id":"token","name":"Token","kind":"json","target":"/token","scope":"environment","key":"token"}]);
    let mut second = first.clone();
    second["id"] = "second".into();
    second["url"] = format!("{url}/b").into();
    second["extractions"] = json!([]);
    let mut echo = first.clone();
    echo["id"] = "echo".into();
    echo["url"] = format!("{url}/echo").into();
    echo["pre_request_script"] = "".into();
    echo["post_response_script"] = "pm.environment.set('counter','2');".into();
    echo["extractions"] = json!([]);
    echo["headers"] = json!([{"id":"token","key":"X-Token","value":"{{token}}","enabled":true},{"id":"count","key":"X-Counter","value":"{{counter}}","enabled":true}]);
    data["collections"][0]["requests"] = json!([echo, second, first]);
    data["scenarios"] = json!([{"id":"s","name":"Parallel","collection_id":"c","steps":[{"id":"a","request_id":"first"},{"id":"b","request_id":"second"},{"id":"after","request_id":"echo"}],"parallel":block()}]);
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"Parallel","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (status, report) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        None,
        Some(json!({"collection_id":"c","environment_id":"dev","scenario_id":"s"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["passed"], 3, "{report}");
    assert_eq!(report["failed"], 0);
    assert!(report["stopped_reason"].is_null());
    assert_eq!(report["results"][0]["step_id"], "a");
    assert_eq!(report["results"][1]["step_id"], "b");
    assert_eq!(report["results"][0]["variable_updates_applied"], false);
    let body: Value =
        serde_json::from_str(report["results"][2]["response"]["body"].as_str().unwrap()).unwrap();
    assert_eq!(body["token"], "parallel-private-token");
    assert_eq!(body["counter"], "1");
    assert!(
        report["parallel_variable_updates"]
            .as_array()
            .unwrap()
            .iter()
            .any(|update| update["key"] == "counter" && update["value"] == "2")
    );
    let (_, history) = call(&router, "GET", "/api/workspaces/w/history", None, None).await;
    assert_eq!(history.as_array().unwrap().len(), 3);
    assert!(!history.to_string().contains("parallel-private-token"));
    let (_, current) = call(&router, "GET", "/api/workspaces/w", None, None).await;
    assert_eq!(current["data"], saved["data"]);
    fixture.abort();
}
#[tokio::test]
async fn independent_repeats_and_false_conditions_join_before_next_step() {
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::get(|| async { axum::Json(json!({"ok":true})) }),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("repeat.db")).await.unwrap();
    let mut data = example_data();
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = format!("{url}/").into();
    request["post_response_script"] =
        "pm.variables.set('n',String(Number(pm.variables.get('n')||0)+1));".into();
    data["scenarios"] = json!([{"id":"s","name":"Repeat","collection_id":"c","steps":[{"id":"a","request_id":"r","repeat":3,"condition":"Number(pm.variables.get('n')||0)<2"},{"id":"b","request_id":"r","condition":"(pm.variables.set('n','99'),false)"},{"id":"after","request_id":"r","condition":"pm.variables.get('n') === '2'"}],"parallel":block()}]);
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"Repeat","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, report) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        None,
        Some(json!({"collection_id":"c","scenario_id":"s"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["passed"], 3, "{report}");
    assert_eq!(report["failed"], 0);
    assert_eq!(report["skipped"], 2);
    assert_eq!(report["executed_steps"], 5);
    assert_eq!(report["results"][1]["condition_skipped"], true);
    assert_eq!(report["results"][2]["step_repeat_index"], 1);
    fixture.abort();
}
#[tokio::test]
async fn divergent_or_deletion_writes_stop_without_shared_variable_updates() {
    let (url, fixture) =
        serve(Router::new().route("/", axum::routing::get(|| async { "ok" }))).await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("conflict.db")).await.unwrap();
    for (index, script) in [
        "pm.variables.set('same','left');",
        "pm.variables.unset('same');",
    ]
    .into_iter()
    .enumerate()
    {
        let mut data = example_data();
        let mut first = data["collections"][0]["requests"][0].clone();
        first["url"] = format!("{url}/").into();
        first["pre_request_script"] = script.into();
        let mut second = first.clone();
        second["id"] = "other".into();
        second["pre_request_script"] = "pm.variables.set('same','right');".into();
        data["collections"][0]["requests"] = json!([first, second]);
        data["scenarios"] = json!([{"id":"s","name":"Conflict","collection_id":"c","steps":[{"id":"a","request_id":"r"},{"id":"b","request_id":"other"}],"parallel":block()}]);
        let id = format!("w{index}");
        assert_eq!(
            call(
                &router,
                "POST",
                "/api/workspaces",
                None,
                Some(json!({"id":id,"name":"Conflict","data":data}))
            )
            .await
            .0,
            StatusCode::OK
        );
        let (_,report)=call(&router,"POST",&format!("/api/workspaces/{id}/run"),None,Some(json!({"collection_id":"c","scenario_id":"s","variables":[{"id":"v","key":"same","value":"initial","enabled":true}]}))).await;
        assert_eq!(
            report["stopped_reason"], "parallel_variable_conflict",
            "{report}"
        );
        assert_eq!(report["failed"], 1);
        assert!(report.get("parallel_variable_updates").is_none());
        assert!(
            report["results"]
                .as_array()
                .unwrap()
                .iter()
                .all(|item| item["variable_updates_applied"] == false)
        );
    }
    fixture.abort();
}
#[tokio::test]
async fn invalid_membership_redirects_and_interior_entries_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("invalid.db")).await.unwrap();
    for (index, block) in [
        json!({"id":"p","name":"Bad","step_ids":["a","c"],"concurrency":2}),
        json!({"id":"p","name":"Bad","step_ids":["a","a"],"concurrency":2}),
        json!({"id":"p","name":"Bad","step_ids":["a","b"],"concurrency":5}),
    ]
    .into_iter()
    .enumerate()
    {
        let mut data = example_data();
        data["scenarios"] = json!([{"id":"s","name":"Bad","collection_id":"c","steps":[{"id":"a","request_id":"r"},{"id":"b","request_id":"r"},{"id":"c","request_id":"r"}],"parallel":[block]}]);
        assert_eq!(
            call(
                &router,
                "POST",
                "/api/workspaces",
                None,
                Some(json!({"id":format!("bad{index}"),"name":"Bad","data":data}))
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
    let mut data = example_data();
    data["scenarios"] = json!([{"id":"s","name":"Bad","collection_id":"c","steps":[{"id":"a","request_id":"r"},{"id":"b","request_id":"r"},{"id":"c","request_id":"r","on_true":{"action":"step","step_id":"b"}}],"parallel":block()}]);
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"edge","name":"Bad","data":data}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
}
#[tokio::test]
async fn collection_namespaces_merge_independently_and_script_redirects_fail() {
    let (url,fixture)=serve(Router::new().route("/",axum::routing::get(|headers:axum::http::HeaderMap|async move{axum::Json(json!({"value":headers.get("x-value").and_then(|value|value.to_str().ok()).unwrap_or("none")}))}))).await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("namespace.db")).await.unwrap();
    let mut data = example_data();
    let mut root = data["collections"][0]["requests"][0].clone();
    root["url"] = format!("{url}/").into();
    root["pre_request_script"] = "pm.collectionVariables.set('value','root');".into();
    let mut child = root.clone();
    child["id"] = "child-request".into();
    child["pre_request_script"] = "pm.collectionVariables.set('value','leaf');".into();
    let mut root_echo = root.clone();
    root_echo["id"] = "root-echo".into();
    root_echo["pre_request_script"] = "".into();
    root_echo["headers"] = json!([{"id":"v","key":"X-Value","value":"{{value}}","enabled":true}]);
    let mut child_echo = root_echo.clone();
    child_echo["id"] = "child-echo".into();
    data["collections"][0]["requests"] = json!([root, root_echo]);
    data["collections"].as_array_mut().unwrap().push(json!({"id":"leaf","parent_id":"c","name":"Leaf","description":"","requests":[child,child_echo]}));
    data["scenarios"] = json!([{"id":"s","name":"Namespaces","collection_id":"c","steps":[{"id":"a","request_id":"r"},{"id":"b","request_id":"child-request"},{"id":"root-after","request_id":"root-echo"},{"id":"child-after","request_id":"child-echo"}],"parallel":block()}]);
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"Namespaces","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, report) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        None,
        Some(json!({"collection_id":"c","scenario_id":"s"})),
    )
    .await;
    assert_eq!(report["passed"], 4, "{report}");
    for (index, value) in [(2, "root"), (3, "leaf")] {
        let body: Value = serde_json::from_str(
            report["results"][index]["response"]["body"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(body["value"], value);
    }
    assert_eq!(
        report["parallel_variable_updates"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    data["collections"][0]["requests"][0]["pre_request_script"] =
        "pm.execution.setNextRequest(null);".into();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"redirect","name":"Redirect","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, report) = call(
        &router,
        "POST",
        "/api/workspaces/redirect/run",
        None,
        Some(json!({"collection_id":"c","scenario_id":"s"})),
    )
    .await;
    assert_eq!(report["stopped_reason"], "parallel_control");
    assert_eq!(report["failed"], 1);
    assert!(report.get("parallel_variable_updates").is_none());
    fixture.abort();
}
#[tokio::test]
async fn parallel_requests_cancel_together_and_cannot_be_run_or_cancelled_by_another_owner() {
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = calls.clone();
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::get(move || {
            let calls = observed.clone();
            async move {
                calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                std::future::pending::<String>().await
            }
        }),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let mut settings = config(
        format!(
            "sqlite://{}?mode=rwc",
            temp.path().join("cancel.db").display()
        ),
        true,
    );
    settings.allow_private_network = true;
    let router = hosted(settings).await.unwrap();
    let owner = register(&router, "parallel-owner").await;
    let other = register(&router, "parallel-other").await;
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = format!("{url}/").into();
    data["collections"][0]["requests"][0]["timeout_ms"] = 10000.into();
    data["scenarios"] = json!([{"id":"s","name":"Cancel","collection_id":"c","steps":[{"id":"a","request_id":"r"},{"id":"b","request_id":"r"}],"parallel":block()}]);
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            Some(&owner),
            Some(json!({"id":"w","name":"Cancel","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let payload = json!({"collection_id":"c","scenario_id":"s","job_id":"parallel-cancel"});
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces/w/run",
            Some(&other),
            Some(payload.clone())
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let task_router = router.clone();
    let task_owner = owner.clone();
    let task = tokio::spawn(async move {
        call(
            &task_router,
            "POST",
            "/api/workspaces/w/run",
            Some(&task_owner),
            Some(payload),
        )
        .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while calls.load(std::sync::atomic::Ordering::SeqCst) < 2 {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces/w/run/cancel",
            Some(&other),
            Some(json!({"job_id":"parallel-cancel"}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces/w/run/cancel",
            Some(&owner),
            Some(json!({"job_id":"parallel-cancel"}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, report) = task.await.unwrap();
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["cancelled"], true);
    assert_eq!(report["executed_steps"], 2);
    let (_, history) = call(
        &router,
        "GET",
        "/api/workspaces/w/history",
        Some(&owner),
        None,
    )
    .await;
    assert!(history.as_array().unwrap().is_empty());
    fixture.abort();
}
#[tokio::test]
async fn merged_collection_quota_is_checked_before_applying_any_branch_update() {
    let (url, fixture) =
        serve(Router::new().route("/", axum::routing::get(|| async { "ok" }))).await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("quota.db")).await.unwrap();
    let mut data = example_data();
    let mut first = data["collections"][0]["requests"][0].clone();
    first["url"] = format!("{url}/").into();
    first["post_response_script"] =
        "for(let i=0;i<600;i++)pm.collectionVariables.set('a'+i,'value');".into();
    let mut second = first.clone();
    second["id"] = "second".into();
    second["post_response_script"] =
        "for(let i=0;i<600;i++)pm.collectionVariables.set('b'+i,'value');".into();
    data["collections"][0]["requests"] = json!([first, second]);
    data["scenarios"] = json!([{"id":"s","name":"Quota","collection_id":"c","steps":[{"id":"a","request_id":"r"},{"id":"b","request_id":"second"}],"parallel":block()}]);
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"Quota","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, report) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        None,
        Some(json!({"collection_id":"c","scenario_id":"s"})),
    )
    .await;
    assert_eq!(report["stopped_reason"], "variable_limit", "{report}");
    assert_eq!(report["failed"], 1);
    assert!(report.get("parallel_variable_updates").is_none());
    fixture.abort();
}
