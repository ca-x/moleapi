mod common;
use common::*;

#[tokio::test]
async fn shared_dataset_preview_checks_role_revision_limits_and_audits_without_source() {
    let directory = tempfile::tempdir().unwrap();
    let router = hosted(config(
        moleapi_server::sqlite_database_url(&directory.path().join("datasets.db")).unwrap(),
        true,
    ))
    .await
    .unwrap();
    let owner = register(&router, "datasetowner").await;
    let editor = register(&router, "dataseteditor").await;
    let viewer = register(&router, "datasetviewer").await;
    let foreign = register(&router, "datasetforeign").await;
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Datasets","data":example_data()})),
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
    let endpoint = format!("{base}/dataset/preview");
    let mut input = json!({"expected_revision":saved["revision"],"dataset":{"format":"json","source":"[{\"id\":7,\"value\":\"private-dataset-row\"}]"}});
    assert_eq!(
        call(
            &router,
            "POST",
            &endpoint,
            Some(&viewer),
            Some(input.clone())
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &router,
            "POST",
            &endpoint,
            Some(&foreign),
            Some(input.clone())
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, preview) = call(
        &router,
        "POST",
        &endpoint,
        Some(&editor),
        Some(input.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(
        preview["rows"],
        json!([{"id":7,"value":"private-dataset-row"}])
    );
    input["expected_revision"] = json!(0);
    assert_eq!(
        call(
            &router,
            "POST",
            &endpoint,
            Some(&editor),
            Some(input.clone())
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    input["expected_revision"] = saved["revision"].clone();
    input["dataset"] = json!({"format":"csv","source":"id,value\n8,CSV\n"});
    let (status, preview) = call(
        &router,
        "POST",
        &endpoint,
        Some(&editor),
        Some(input.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(preview["rows"][0]["value"], "CSV");
    input["dataset"] = json!({"format":"json","source":json!((0..101).map(|id|json!({"id":id})).collect::<Vec<_>>()).to_string()});
    assert_eq!(
        call(
            &router,
            "POST",
            &endpoint,
            Some(&editor),
            Some(input.clone())
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    input["dataset"]["source"] = json!("x".repeat(1024 * 1024 + 1));
    assert_eq!(
        call(&router, "POST", &endpoint, Some(&editor), Some(input))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/testing/dataset/preview",
            Some(&editor),
            Some(json!({"workspace_id":"w","dataset":{"format":"json","source":"[{}]"}}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (_, audit) = call(&router, "GET", &format!("{base}/audit"), Some(&owner), None).await;
    assert!(
        audit
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["action"] == "dataset.preview" && event["actor_id"] == editor_id)
    );
    assert!(!audit.to_string().contains("private-dataset-row"));
}

#[tokio::test]
async fn member_dataset_views_drive_saved_mocks_and_reject_dangling_binding_changes() {
    let directory = tempfile::tempdir().unwrap();
    let router = hosted(config(
        moleapi_server::sqlite_database_url(&directory.path().join("views.db")).unwrap(),
        true,
    ))
    .await
    .unwrap();
    let owner = register(&router, "viewowner").await;
    let editor = register(&router, "vieweditor").await;
    let mut data = example_data();
    data["script_mode"] = json!("v2");
    let (_, created) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Dataset views","data":data})),
    )
    .await;
    let (_, invitation) = call(
        &router,
        "POST",
        "/api/workspaces/w/invitations",
        Some(&owner),
        Some(json!({"role":"editor","expires_in_days":1})),
    )
    .await;
    let (_, accepted) = call(
        &router,
        "POST",
        "/api/workspace-invitations/accept",
        Some(&editor),
        Some(json!({"token":invitation["token"]})),
    )
    .await;
    let base = format!(
        "/api/owners/{}/workspaces/w",
        accepted["owner_id"].as_str().unwrap()
    );
    data["datasets"] = json!([{"id":"rows","name":"Member rows","table_name":"records","secret":true,"source":{"format":"json","source":"[{\"id\":7}]"},"views":[{"id":"all","name":"All","sql":"SELECT id FROM records"}]}]);
    data["collections"][0]["requests"][0]["examples"][0]["mock_data"] =
        json!({"dataset_id":"rows","view_id":"all","shape":"rows"});
    let (status, saved) = call(
        &router,
        "PUT",
        &base,
        Some(&editor),
        Some(json!({"name":created["name"],"expected_revision":created["revision"],"data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (status, response) = call(
        &router,
        "GET",
        &format!("{base}/mock/r/e"),
        Some(&editor),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{response}");
    assert_eq!(response, json!([{"id":"7"}]));
    for change in ["dataset", "view", "sql"] {
        let mut candidate = data.clone();
        match change {
            "dataset" => candidate["datasets"] = json!([]),
            "view" => candidate["datasets"][0]["views"] = json!([]),
            _ => candidate["datasets"][0]["views"][0]["sql"] = json!("DELETE FROM records"),
        }
        assert_eq!(call(&router,"PUT",&base,Some(&editor),Some(json!({"name":saved["name"],"expected_revision":saved["revision"],"data":candidate}))).await.0,StatusCode::BAD_REQUEST);
    }
    let (_, canonical) = call(&router, "GET", "/api/workspaces/w", Some(&owner), None).await;
    assert_eq!(canonical["revision"], saved["revision"]);
    assert_eq!(canonical["data"]["datasets"][0]["table_name"], "records");
    data["datasets"][0]["source"]["source"] = json!("[{\"id\":8}]");
    let (status, updated) = call(
        &router,
        "PUT",
        &base,
        Some(&editor),
        Some(json!({"name":saved["name"],"expected_revision":saved["revision"],"data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(
        call(
            &router,
            "GET",
            &format!("{base}/mock/r/e"),
            Some(&editor),
            None
        )
        .await
        .1,
        json!([{"id":"8"}])
    );
    data["collections"][0]["requests"][0]["examples"][0]
        .as_object_mut()
        .unwrap()
        .remove("mock_data");
    data["datasets"] = json!([]);
    let (status, _) = call(
        &router,
        "PUT",
        &base,
        Some(&editor),
        Some(json!({"name":updated["name"],"expected_revision":updated["revision"],"data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, canonical) = call(&router, "GET", "/api/workspaces/w", Some(&owner), None).await;
    assert!(
        canonical["data"]["datasets"]
            .as_array()
            .is_none_or(|datasets| datasets.is_empty())
    );
    assert_eq!(
        canonical["data"]["collections"][0]["requests"][0]["examples"][0]["body"],
        "example body"
    );
}

#[tokio::test]
async fn member_dataset_preview_stays_closed_for_an_existing_local_workspace() {
    let directory = tempfile::tempdir().unwrap();
    let router = local(&directory.path().join("local.db")).await.unwrap();
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"Local","data":data()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let dataset = json!({"format":"json","source":"[{\"id\":7}]"});
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/testing/dataset/preview",
            None,
            Some(json!({"workspace_id":"w","dataset":dataset}))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/owners/local/workspaces/w/dataset/preview",
            None,
            Some(json!({"expected_revision":saved["revision"],"dataset":dataset}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}
