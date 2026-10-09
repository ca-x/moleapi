mod common;
use common::*;
use std::sync::{Arc, Mutex};
#[tokio::test]
async fn conditions_branch_repeat_and_loop_without_applying_condition_mutations() {
    let calls = Arc::new(Mutex::new(Vec::<String>::new()));
    let seed = calls.clone();
    let work = calls.clone();
    let never = calls.clone();
    let (url, fixture) = serve(
        Router::new()
            .route(
                "/seed",
                axum::routing::get(move || {
                    let calls = seed.clone();
                    async move {
                        calls.lock().unwrap().push("seed".into());
                        axum::Json(json!({"ready":"yes"}))
                    }
                }),
            )
            .route(
                "/work",
                axum::routing::get(move |headers: axum::http::HeaderMap| {
                    let calls = work.clone();
                    async move {
                        let value = headers
                            .get("x-count")
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .to_string();
                        calls.lock().unwrap().push(value);
                        axum::Json(json!({"ok":true}))
                    }
                }),
            )
            .route(
                "/never",
                axum::routing::get(move || {
                    let calls = never.clone();
                    async move {
                        calls.lock().unwrap().push("NEVER".into());
                        "unexpected"
                    }
                }),
            ),
    )
    .await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("control.db")).await.unwrap();
    let mut data = example_data();
    let mut first = data["collections"][0]["requests"][0].clone();
    first["id"] = "seed".into();
    first["url"] = format!("{url}/seed").into();
    first["extractions"] = json!([{"id":"ready","name":"Ready","kind":"json","target":"/ready","scope":"temporary","key":"ready"}]);
    first["post_response_script"] = "pm.variables.set('n','0');".into();
    let mut work = first.clone();
    work["id"] = "work".into();
    work["url"] = format!("{url}/work").into();
    work["extractions"] = json!([]);
    work["headers"] = json!([{"id":"n","key":"X-Count","value":"{{n}}","enabled":true}]);
    work["post_response_script"] =
        "pm.variables.set('n',String(Number(pm.variables.get('n'))+1));".into();
    let mut never = first.clone();
    never["id"] = "never".into();
    never["url"] = format!("{url}/never").into();
    never["extractions"] = json!([]);
    never["pre_request_script"] = "throw new Error('must not run');".into();
    data["collections"][0]["requests"] = json!([never, work, first]);
    data["scenarios"] = json!([{"id":"s","name":"Control","collection_id":"c","steps":[{"id":"init","request_id":"seed","repeat":2,"on_true":{"action":"step","step_id":"false"}},{"id":"false","request_id":"never","condition":"(pm.variables.set('ready','no'), false)","on_false":{"action":"step","step_id":"loop"}},{"id":"loop","request_id":"work","condition":"pm.variables.get('ready') === 'yes' && Number(pm.variables.get('n')) < 2 && pm.response.code === 200","on_true":{"action":"step","step_id":"loop"},"on_false":{"action":"stop"}},{"id":"unreached","request_id":"never"}]}]);
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"Control","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (status, report) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        None,
        Some(json!({"collection_id":"c","scenario_id":"s","iterations":2})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["passed"], 8, "{report}");
    assert_eq!(report["failed"], 0);
    assert_eq!(report["skipped"], 4);
    assert_eq!(report["executed_steps"], 12);
    assert_eq!(report["completed_iterations"], 2);
    assert_eq!(report["iterations"][0]["scenario_stopped"], true);
    assert_eq!(report["results"][1]["step_repeat_index"], 1);
    assert_eq!(report["results"][2]["condition_skipped"], true);
    assert_eq!(
        *calls.lock().unwrap(),
        vec!["seed", "seed", "0", "1", "seed", "seed", "0", "1"]
    );
    let (_, current) = call(&router, "GET", "/api/workspaces/w", None, None).await;
    assert_eq!(current["data"], saved["data"]);
    let (_, history) = call(&router, "GET", "/api/workspaces/w/history", None, None).await;
    assert_eq!(history.as_array().unwrap().len(), 8);
    fixture.abort();
}
#[tokio::test]
async fn invalid_conditions_stop_without_network_or_private_diagnostics() {
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("fail.db")).await.unwrap();
    for (index, expression) in [
        "'private-condition-secret'",
        "(()=>{throw new Error('private-condition-secret')})()",
        "(()=>{while(true){}return true})()",
    ]
    .into_iter()
    .enumerate()
    {
        let mut data = example_data();
        data["scenarios"] = json!([{"id":"s","name":"Invalid","collection_id":"c","steps":[{"id":"step","request_id":"r","condition":expression}]}]);
        let id = format!("w{index}");
        assert_eq!(
            call(
                &router,
                "POST",
                "/api/workspaces",
                None,
                Some(json!({"id":id,"name":"Invalid","data":data}))
            )
            .await
            .0,
            StatusCode::OK
        );
        let (status, report) = call(
            &router,
            "POST",
            &format!("/api/workspaces/{id}/run"),
            None,
            Some(json!({"collection_id":"c","scenario_id":"s"})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{report}");
        assert_eq!(report["stopped_reason"], "condition_error");
        assert_eq!(report["failed"], 1);
        assert!(!report.to_string().contains("private-condition-secret"));
        let (_, history) = call(
            &router,
            "GET",
            &format!("/api/workspaces/{id}/history"),
            None,
            None,
        )
        .await;
        assert!(history.as_array().unwrap().is_empty());
    }
}
#[tokio::test]
async fn invalid_edges_and_repeat_limits_are_rejected_at_save() {
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("validate.db")).await.unwrap();
    for (index, control) in [
        json!({"on_true":{"action":"step","step_id":"missing"}}),
        json!({"repeat":0}),
        json!({"repeat":1001}),
        json!({"on_false":{"action":"step","step_id":"off"}}),
    ]
    .into_iter()
    .enumerate()
    {
        let mut data = example_data();
        let mut step = json!({"id":"step","request_id":"r"});
        step.as_object_mut()
            .unwrap()
            .extend(control.as_object().unwrap().clone());
        data["scenarios"] = json!([{"id":"s","name":"Bad","collection_id":"c","steps":[step,{"id":"off","request_id":"r","enabled":false}]}]);
        let (status, result) = call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":format!("w{index}"),"name":"Bad","data":data})),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{result}");
    }
}
#[tokio::test]
async fn explicit_back_edges_consume_the_shared_step_limit() {
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("limit.db")).await.unwrap();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["pre_request_script"] =
        "pm.execution.skipRequest();".into();
    data["scenarios"] = json!([{"id":"s","name":"Bounded loop","collection_id":"c","steps":[{"id":"loop","request_id":"r","on_true":{"action":"step","step_id":"loop"}}]}]);
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"Loop","data":data}))
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
    assert_eq!(report["stopped_reason"], "step_limit");
    assert_eq!(report["executed_steps"], 1000);
    assert_eq!(report["skipped"], 1000);
}
#[cfg(unix)]
#[tokio::test]
async fn logout_during_condition_cancels_before_any_request() {
    use std::os::unix::fs::PermissionsExt;
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = calls.clone();
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::get(move || {
            let calls = observed.clone();
            async move {
                calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                "unexpected"
            }
        }),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let marker = temp.path().join("condition-started");
    let worker = temp.path().join("slow-worker");
    std::fs::write(
        &worker,
        format!(
            "#!/bin/sh\nprintf ready > '{}'\n/bin/sleep 0.2\nexec '{}' \"$@\"\n",
            marker.display(),
            env!("CARGO_BIN_EXE_moleapi-server")
        ),
    )
    .unwrap();
    std::fs::set_permissions(&worker, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut settings = config(
        format!(
            "sqlite://{}?mode=rwc",
            temp.path().join("owner.db").display()
        ),
        true,
    );
    settings.allow_private_network = true;
    let router = moleapi_server::hosted_with_worker(settings, &worker)
        .await
        .unwrap();
    let owner = register(&router, "control-owner").await;
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = format!("{url}/").into();
    data["scenarios"] = json!([{"id":"s","name":"Wait","collection_id":"c","steps":[{"id":"step","request_id":"r","condition":"true"}]}]);
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            Some(&owner),
            Some(json!({"id":"w","name":"Wait","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let task_router = router.clone();
    let task_owner = owner.clone();
    let task = tokio::spawn(async move {
        call(
            &task_router,
            "POST",
            "/api/workspaces/w/run",
            Some(&task_owner),
            Some(json!({"collection_id":"c","scenario_id":"s"})),
        )
        .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while !marker.exists() {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        call(&router, "POST", "/api/auth/logout", Some(&owner), None)
            .await
            .0,
        StatusCode::OK
    );
    let (status, report) = task.await.unwrap();
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["cancelled"], true);
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    fixture.abort();
}
