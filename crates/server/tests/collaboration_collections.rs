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

#[tokio::test]
async fn member_resource_deletion_revokes_removed_public_mocks_and_retains_documents() {
    let directory = tempfile::tempdir().unwrap();
    let router = hosted(config(
        moleapi_server::sqlite_database_url(&directory.path().join("delete.db")).unwrap(),
        true,
    ))
    .await
    .unwrap();
    let owner = register(&router, "deleteowner").await;
    let editor = register(&router, "deleteeditor").await;
    let viewer = register(&router, "deleteviewer").await;
    let mut source = example_data();
    let mut child_request = source["collections"][0]["requests"][0].clone();
    child_request["id"] = json!("child-request");
    source["collections"].as_array_mut().unwrap().push(json!({"id":"child","name":"Child","parent_id":"c","description":"","requests":[child_request]}));
    source["collections"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"keep","name":"Keep","description":"","requests":[]}));
    source["documents"] = json!([{"id":"guide","name":"Guide","collection_id":"child","source":"retained document","secret":true}]);
    source["scenarios"] = json!([{"id":"suite","name":"References root","description":"","collection_id":"c","steps":[{"id":"step","request_id":"r","name":"Referenced request","group":"","enabled":true}]}]);
    let (status, _) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Deletion","data":source})),
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
    let mut public_paths = Vec::new();
    for request_id in ["r", "child-request"] {
        let (status, share) = call(
            &router,
            "POST",
            "/api/workspaces/w/mock-shares",
            Some(&owner),
            Some(json!({"request_id":request_id,"expires_in_days":1})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{share}");
        let path = format!("/api/public/mock/{}/e", share["token"].as_str().unwrap());
        assert_eq!(
            call(&router, "GET", &path, None, None).await.0,
            StatusCode::CREATED
        );
        public_paths.push(path);
    }
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces/w/mock-shares",
            Some(&editor),
            Some(json!({"request_id":"r","expires_in_days":1}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (_, saved) = call(&router, "GET", &base, Some(&editor), None).await;
    let mut next = saved["data"].clone();
    next["collections"] = json!([next["collections"][2].clone()]);
    next["documents"][0]["collection_id"] = Value::Null;
    assert_eq!(
        call(
            &router,
            "PUT",
            &base,
            Some(&editor),
            Some(json!({"name":saved["name"],"data":next,"expected_revision":saved["revision"]}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    next["scenarios"] = json!([]);
    let input = json!({"name":saved["name"],"data":next,"expected_revision":saved["revision"]});
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
    for path in public_paths {
        assert_eq!(
            call(&router, "GET", &path, None, None).await.0,
            StatusCode::NOT_FOUND
        );
    }
    let (_, canonical) = call(&router, "GET", "/api/workspaces/w", Some(&owner), None).await;
    assert_eq!(
        canonical["data"]["collections"].as_array().unwrap().len(),
        1
    );
    assert_eq!(canonical["data"]["collections"][0]["id"], "keep");
    assert_eq!(
        canonical["data"]["documents"][0]["source"],
        "retained document"
    );
    assert!(canonical["data"]["documents"][0]["collection_id"].is_null());
    let (_, audit) = call(&router, "GET", &format!("{base}/audit"), Some(&owner), None).await;
    assert!(
        audit
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["action"] == "workspace.update" && event["actor_id"] == editor_id)
    );
    assert!(!audit.to_string().contains("retained document"));
}
