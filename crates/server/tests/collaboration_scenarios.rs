mod common;
use common::*;
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn member_saves_and_runs_ordered_conditional_scenarios_with_actor_reports() {
    let directory = tempfile::tempdir().unwrap();
    let mut settings = config(
        moleapi_server::sqlite_database_url(&directory.path().join("scenarios.db")).unwrap(),
        true,
    );
    settings.allow_private_network = true;
    let router = hosted(settings).await.unwrap();
    let owner = register(&router, "scenarioowner").await;
    let editor = register(&router, "scenarioeditor").await;
    let viewer = register(&router, "scenarioviewer").await;
    let (status, _) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Scenarios","data":example_data()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let mut base = String::new();
    let mut editor_id = Value::Null;
    for (token, role) in [(&editor, "editor"), (&viewer, "viewer")] {
        let (_, invitation) = call(
            &router,
            "POST",
            "/api/workspaces/w/invitations",
            Some(&owner),
            Some(json!({"role":role,"expires_in_days":1})),
        )
        .await;
        let (status, accepted) = call(
            &router,
            "POST",
            "/api/workspace-invitations/accept",
            Some(token),
            Some(json!({"token":invitation["token"]})),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        base = format!(
            "/api/owners/{}/workspaces/w",
            accepted["owner_id"].as_str().unwrap()
        );
        if role == "editor" {
            editor_id = accepted["member_id"].clone();
        }
    }
    let calls = Arc::new(Mutex::new(Vec::<String>::new()));
    let observed = calls.clone();
    let (url, fixture) = serve(Router::new().route(
        "/{item}",
        axum::routing::get(
            move |axum::extract::Path(item): axum::extract::Path<String>| {
                let calls = observed.clone();
                async move {
                    calls.lock().unwrap().push(item.clone());
                    axum::Json(json!({"item":item}))
                }
            },
        ),
    ))
    .await;
    let (_, saved) = call(&router, "GET", &base, Some(&editor), None).await;
    let mut data = saved["data"].clone();
    data["script_mode"] = json!("v2");
    let mut first = data["collections"][0]["requests"][0].clone();
    first["id"] = json!("first");
    first["url"] = json!(format!("{url}/first"));
    let mut second = first.clone();
    second["id"] = json!("second");
    second["url"] = json!(format!("{url}/second"));
    data["collections"][0]["requests"] = json!([first, second]);
    data["collections"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"other","name":"Other","description":"","requests":[]}));
    data["scenarios"] = json!([{"id":"flow","name":"Ordered conditional flow","description":"","collection_id":"c","steps":[{"id":"repeat","request_id":"second","name":"Repeat second","group":"Repeat","repeat":2},{"id":"stop","request_id":"first","name":"Conditional stop","condition":"false","on_false":{"action":"stop"}},{"id":"never","request_id":"first","name":"Must not run"}]}]);
    let input = json!({"name":saved["name"],"data":data,"expected_revision":saved["revision"]});
    assert_eq!(
        call(&router, "PUT", &base, Some(&viewer), Some(input.clone()))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, updated) = call(&router, "PUT", &base, Some(&editor), Some(input.clone())).await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(
        call(&router, "PUT", &base, Some(&editor), Some(input))
            .await
            .0,
        StatusCode::CONFLICT
    );
    let run_path = format!("{base}/run");
    let options = json!({"collection_id":"c","scenario_id":"flow","job_id":"member-scenario"});
    assert_eq!(
        call(
            &router,
            "POST",
            &run_path,
            Some(&viewer),
            Some(options.clone())
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (status, result) = call(&router, "POST", &run_path, Some(&editor), Some(options)).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["scenario_id"], "flow");
    assert_eq!(result["job_id"], "member-scenario");
    assert_eq!(result["passed"], 2, "{result}");
    assert_eq!(*calls.lock().unwrap(), vec!["second", "second"]);
    let report_id = result["report_id"].as_str().unwrap();
    let (status, report) = call(
        &router,
        "GET",
        &format!("{base}/reports/{report_id}"),
        Some(&editor),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["scenario_id"], "flow");
    assert_eq!(
        call(
            &router,
            "GET",
            &format!("{base}/reports/{report_id}"),
            Some(&viewer),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &router,
            "POST",
            &run_path,
            Some(&editor),
            Some(json!({"collection_id":"other","scenario_id":"flow"}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (_, canonical) = call(&router, "GET", "/api/workspaces/w", Some(&owner), None).await;
    assert_eq!(canonical["data"], updated["data"]);
    let (_, audit) = call(&router, "GET", &format!("{base}/audit"), Some(&owner), None).await;
    assert!(
        audit
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["action"] == "workspace.run"
                && event["actor_id"] == editor_id
                && event["subject_id"] == "member-scenario")
    );
    fixture.abort();
}
