mod common;
use common::*;

#[tokio::test]
async fn member_creates_a_collection_tree_and_executes_inherited_settings() {
    let directory = tempfile::tempdir().unwrap();
    let mut settings = config(
        moleapi_server::sqlite_database_url(&directory.path().join("collections.db")).unwrap(),
        true,
    );
    settings.allow_private_network = true;
    let router = hosted(settings).await.unwrap();
    let owner = register(&router, "folderowner").await;
    let editor = register(&router, "foldereditor").await;
    let viewer = register(&router, "folderviewer").await;
    let (status, _) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Folders","data":data()})),
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
    let (url, fixture) = serve(Router::new().route("/inspect",axum::routing::get(|uri: axum::http::Uri, headers: axum::http::HeaderMap| async move {
        axum::Json(json!({"query":uri.query(),"auth_ok":headers.get("authorization").and_then(|value|value.to_str().ok())==Some("Bearer inherited-folder-secret"),"root_script":headers.get("x-root").and_then(|value|value.to_str().ok()),"child_script":headers.get("x-child").and_then(|value|value.to_str().ok())}))
    }))).await;
    let (_, saved) = call(&router, "GET", &base, Some(&editor), None).await;
    let mut next = example_data();
    next["script_mode"] = json!("v2");
    let mut request = next["collections"][0]["requests"][0].clone();
    request["url"] = json!(format!("{url}/inspect?folder={{{{folder}}}}"));
    request["auth"]["kind"] = json!("inherit");
    next["collections"] = json!([
        {"id":"root","name":"Parent","description":"","auth":{"kind":"bearer","token":"inherited-folder-secret","username":"","password":""},"variables":[{"id":"folder","key":"folder","value":"shared","enabled":true}],"pre_request_script":"pm.request.headers.upsert({key:'X-Root',value:'parent'});","requests":[]},
        {"id":"child","parent_id":"root","name":"Child","description":"","pre_request_script":"pm.request.headers.upsert({key:'X-Child',value:'child'});","requests":[request]}
    ]);
    let update = json!({"name":"Folders","data":next,"expected_revision":saved["revision"]});
    assert_eq!(
        call(&router, "PUT", &base, Some(&viewer), Some(update.clone()))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, updated) = call(&router, "PUT", &base, Some(&editor), Some(update.clone())).await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(
        call(&router, "PUT", &base, Some(&editor), Some(update))
            .await
            .0,
        StatusCode::CONFLICT
    );
    let (_, canonical) = call(&router, "GET", "/api/workspaces/w", Some(&owner), None).await;
    assert_eq!(canonical["revision"], updated["revision"]);
    assert_eq!(canonical["data"]["collections"][1]["parent_id"], "root");
    let (status, response) = call(
        &router,
        "POST",
        &format!("{base}/requests/r/execute"),
        Some(&editor),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], 200, "{response}");
    let body: Value = serde_json::from_str(response["body"].as_str().unwrap()).unwrap();
    assert_eq!(
        body,
        json!({"query":"folder=shared","auth_ok":true,"root_script":"parent","child_script":"child"})
    );
    next["collections"][0]["parent_id"] = json!("child");
    assert_eq!(
        call(
            &router,
            "PUT",
            &base,
            Some(&editor),
            Some(
                json!({"name":"Invalid cycle","data":next,"expected_revision":updated["revision"]})
            )
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (_, audit) = call(&router, "GET", &format!("{base}/audit"), Some(&owner), None).await;
    assert!(
        audit
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["action"] == "workspace.update" && event["actor_id"] == editor_id)
    );
    assert!(!audit.to_string().contains("inherited-folder-secret"));
    fixture.abort();
}
