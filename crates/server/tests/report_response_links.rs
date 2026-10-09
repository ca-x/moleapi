mod common;
use common::*;
#[tokio::test]
async fn serial_links_use_stored_redaction_stable_ids_and_survive_source_edits() {
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::get(|| async {
            axum::Json(json!({"token":"linked-private-token","count":1}))
        }),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("links.db")).await.unwrap();
    let mut data = example_data();
    data["global_variables"] =
        json!([{"id":"secret","key":"private_digit","value":"1","secret":true,"enabled":true}]);
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = format!("{url}/").into();
    request["extractions"] = json!([{"id":"token","name":"Token","kind":"json","target":"/token","scope":"temporary","key":"token"}]);
    let mut skip = request.clone();
    skip["id"] = "skip".into();
    skip["pre_request_script"] = "pm.execution.skipRequest();".into();
    data["collections"][0]["requests"]
        .as_array_mut()
        .unwrap()
        .push(skip);
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"Links","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, run) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        None,
        Some(json!({"collection_id":"c"})),
    )
    .await;
    let id = run["report_id"].as_str().unwrap();
    let (_, report) = call(
        &router,
        "GET",
        &format!("/api/workspaces/w/reports/{id}"),
        None,
        None,
    )
    .await;
    let history_id = report["results"][0]["history_id"].as_str().unwrap();
    assert!(uuid::Uuid::parse_str(history_id).is_ok());
    assert_eq!(run["results"][0]["history_id"], history_id);
    assert!(report["results"][1].get("history_id").is_none());
    let (status, entry) = call(
        &router,
        "GET",
        &format!("/api/workspaces/w/reports/{id}/steps/0/response"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{entry}");
    assert_eq!(entry["id"], history_id);
    assert!(!entry.to_string().contains("linked-private-token"));
    assert_eq!(entry["response"]["variable_updates"], json!([]));
    assert_eq!(
        call(
            &router,
            "GET",
            &format!("/api/workspaces/w/reports/{id}/steps/1/response"),
            None,
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    data["collections"][0]["requests"] = json!([]);
    assert_eq!(
        call(
            &router,
            "PUT",
            "/api/workspaces/w",
            None,
            Some(json!({"name":"Edited","data":data,"expected_revision":1}))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &router,
            "GET",
            &format!("/api/workspaces/w/reports/{id}/steps/0/response"),
            None,
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&router, "DELETE", "/api/workspaces/w/history", None, None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &router,
            "GET",
            &format!("/api/workspaces/w/reports/{id}/steps/0/response"),
            None,
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &router,
            "GET",
            &format!("/api/workspaces/w/reports/{id}"),
            None,
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    fixture.abort();
}
#[tokio::test]
async fn parallel_links_keep_peer_taints_and_enforce_owner_and_workspace_boundaries() {
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::get(|| async { axum::Json(json!({"token":"peer-linked-secret"})) }),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let mut settings = config(
        format!(
            "sqlite://{}?mode=rwc",
            temp.path().join("parallel.db").display()
        ),
        true,
    );
    settings.allow_private_network = true;
    let router = hosted(settings).await.unwrap();
    let owner = register(&router, "link-owner").await;
    let other = register(&router, "link-other").await;
    let mut data = example_data();
    let first = &mut data["collections"][0]["requests"][0];
    first["url"] = format!("{url}/").into();
    first["extractions"] = json!([{"id":"token","name":"Token","kind":"json","target":"/token","scope":"temporary","key":"token"}]);
    let mut second = first.clone();
    second["id"] = "second".into();
    second["extractions"] = json!([]);
    data["collections"][0]["requests"]
        .as_array_mut()
        .unwrap()
        .push(second);
    data["scenarios"] = json!([{"id":"s","name":"Links","collection_id":"c","steps":[{"id":"a","request_id":"r"},{"id":"b","request_id":"second"}],"parallel":[{"id":"block","name":"Peers","step_ids":["a","b"],"concurrency":2}]}]);
    for workspace in ["w", "other"] {
        assert_eq!(
            call(
                &router,
                "POST",
                "/api/workspaces",
                Some(&owner),
                Some(json!({"id":workspace,"name":"Links","data":data}))
            )
            .await
            .0,
            StatusCode::OK
        );
    }
    let (_, run) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        Some(&owner),
        Some(json!({"collection_id":"c","scenario_id":"s"})),
    )
    .await;
    let id = run["report_id"].as_str().unwrap();
    let mut ids = std::collections::BTreeSet::new();
    for position in 0..2 {
        let path = format!("/api/workspaces/w/reports/{id}/steps/{position}/response");
        let (status, entry) = call(&router, "GET", &path, Some(&owner), None).await;
        assert_eq!(status, StatusCode::OK, "{entry}");
        assert!(ids.insert(entry["id"].as_str().unwrap().to_string()));
        assert!(!entry.to_string().contains("peer-linked-secret"));
        assert_eq!(
            call(&router, "GET", &path, Some(&other), None).await.0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            call(
                &router,
                "GET",
                &format!("/api/workspaces/other/reports/{id}/steps/{position}/response"),
                Some(&owner),
                None
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
    }
    fixture.abort();
}
