mod common;
use common::*;
use std::sync::{Arc, Mutex};
#[tokio::test]
async fn saved_scenario_reorders_repeats_and_transfers_extractions() {
    let calls = Arc::new(Mutex::new(Vec::<String>::new()));
    let read = calls.clone();
    let echo = calls.clone();
    let (url, fixture) = serve(
        Router::new()
            .route(
                "/seed",
                axum::routing::get(move || {
                    let calls = read.clone();
                    async move {
                        calls.lock().unwrap().push("seed".into());
                        axum::Json(json!({"token":"scenario-private-token"}))
                    }
                }),
            )
            .route(
                "/echo",
                axum::routing::get(move |headers: axum::http::HeaderMap| {
                    let calls = echo.clone();
                    async move {
                        let token = headers
                            .get("x-token")
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .to_string();
                        calls.lock().unwrap().push(token.clone());
                        axum::Json(json!({"token":token}))
                    }
                }),
            ),
    )
    .await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("scenarios.db")).await.unwrap();
    let mut data = example_data();
    let mut seed = data["collections"][0]["requests"][0].clone();
    seed["id"] = "seed".into();
    seed["url"] = format!("{url}/seed").into();
    seed["extractions"] = json!([{"id":"extract","name":"Token","kind":"json","target":"/token","scope":"temporary","key":"token"}]);
    let mut next = seed.clone();
    next["id"] = "echo".into();
    next["url"] = format!("{url}/echo").into();
    next["extractions"] = json!([]);
    next["headers"] = json!([{"id":"h","key":"X-Token","value":"{{token}}","enabled":true}]);
    data["collections"][0]["requests"] = json!([next, seed]);
    data["scenarios"] = json!([{"id":"scenario","name":"Login flow","collection_id":"c","steps":[{"id":"a","request_id":"seed","name":"Start","group":"Auth"},{"id":"off","request_id":"echo","enabled":false},{"id":"b","request_id":"echo","name":"scenario-private-token","group":"scenario-private-token"},{"id":"c","request_id":"echo","name":"Again"}]}]);
    let (status, workspace) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"Scenarios","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{workspace}");
    let (status, report) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        None,
        Some(json!({"collection_id":"c","scenario_id":"scenario"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["passed"], 3);
    assert_eq!(report["scenario_id"], "scenario");
    assert_eq!(report["results"][0]["step_id"], "a");
    assert_eq!(report["results"][1]["step_id"], "b");
    assert!(
        !report["results"][1]["step_name"]
            .as_str()
            .unwrap()
            .contains("scenario-private-token")
    );
    assert_eq!(
        *calls.lock().unwrap(),
        vec!["seed", "scenario-private-token", "scenario-private-token"]
    );
    let (_, saved) = call(&router, "GET", "/api/workspaces/w", None, None).await;
    assert_eq!(saved["data"], workspace["data"]);
    let (_, history) = call(&router, "GET", "/api/workspaces/w/history", None, None).await;
    assert!(!history.to_string().contains("scenario-private-token"));
    let (status, _) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        None,
        Some(json!({"collection_id":"c","scenario_id":"missing"})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    fixture.abort();
}
#[tokio::test]
async fn repeated_jump_targets_are_ambiguous_and_bad_references_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("jump.db")).await.unwrap();
    let mut data = example_data();
    let request_id = data["collections"][0]["requests"][0]["id"].clone();
    data["collections"][0]["requests"][0]["pre_request_script"] =
        format!("pm.execution.setNextRequest({request_id});pm.execution.skipRequest();").into();
    data["scenarios"] = json!([{"id":"s","name":"Repeated","collection_id":"c","steps":[{"id":"a","request_id":request_id},{"id":"b","request_id":request_id}]}]);
    let (status, w) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"Repeated","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    let (status, report) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        None,
        Some(json!({"collection_id":"c","scenario_id":"s"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["stopped_reason"], "next_request_ambiguous");
    assert_eq!(report["skipped"], 1);
    data["scenarios"][0]["steps"][0]["request_id"] = "missing".into();
    let (status, _) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"bad","name":"Bad","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
#[tokio::test]
async fn hosted_scenarios_enforce_owner_root_and_nonempty_enabled_plan() {
    let temp = tempfile::tempdir().unwrap();
    let router = hosted(config(
        format!(
            "sqlite://{}?mode=rwc",
            temp.path().join("owned.db").display()
        ),
        true,
    ))
    .await
    .unwrap();
    let owner = register(&router, "scenario-owner").await;
    let other = register(&router, "scenario-other").await;
    let mut data = example_data();
    data["collections"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"other","name":"Other","description":"","requests":[]}));
    data["scenarios"] = json!([{"id":"s","name":"Disabled","collection_id":"c","steps":[{"id":"a","request_id":"r","enabled":false}]}]);
    let (status, w) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Owned","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    for (token, root, expected) in [
        (&other, "c", StatusCode::NOT_FOUND),
        (&owner, "other", StatusCode::BAD_REQUEST),
        (&owner, "c", StatusCode::BAD_REQUEST),
    ] {
        let (status, result) = call(
            &router,
            "POST",
            "/api/workspaces/w/run",
            Some(token),
            Some(json!({"collection_id":root,"scenario_id":"s"})),
        )
        .await;
        assert_eq!(status, expected, "{result}");
    }
    data["scenarios"][0]["steps"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"a","request_id":"r"}));
    let (status, _) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"bad","name":"Duplicate steps","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
