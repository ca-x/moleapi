mod common;
use common::*;

#[tokio::test]
async fn markdown_documents_save_reload_screen_backups_and_enforce_owner_and_revision() {
    let dir = tempfile::tempdir().unwrap();
    let router = hosted(config(
        moleapi_server::sqlite_database_url(&dir.path().join("docs.db")).unwrap(),
        true,
    ))
    .await
    .unwrap();
    let owner = register(&router, "docsowner").await;
    let foreign = register(&router, "docsforeign").await;
    let mut data = json!({"schema_version":1,"collections":[{"id":"c","name":"Folder","description":"","requests":[]}],"environments":[],"active_environment_id":null,
        "global_variables":[{"id":"v","key":"api_key","value":"docs-private-value","enabled":true,"secret":true}],
        "documents":[{"id":"d","name":"Guide","collection_id":"c","source":"# Guide\ndocs-private-value"},{"id":"shareable","name":"Public guide","source":"# Public\ndocs-private-value ordinary","secret":false}]});
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Docs","data":data.clone()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(
        call(&router, "GET", "/api/workspaces/w", Some(&foreign), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (_, reloaded) = call(&router, "GET", "/api/workspaces/w", Some(&owner), None).await;
    assert_eq!(
        reloaded["data"]["documents"][0]["source"],
        "# Guide\ndocs-private-value"
    );
    let (_, safe) = call(
        &router,
        "POST",
        "/api/workspaces/w/export",
        Some(&owner),
        Some(json!({"format":"moleapi","include_secrets":false})),
    )
    .await;
    assert!(
        !safe["content"]
            .as_str()
            .unwrap()
            .contains("docs-private-value")
    );
    let (_, full) = call(
        &router,
        "POST",
        "/api/workspaces/w/export",
        Some(&owner),
        Some(json!({"format":"moleapi","include_secrets":true})),
    )
    .await;
    assert!(
        full["content"]
            .as_str()
            .unwrap()
            .contains("docs-private-value")
    );
    data["documents"][0]["collection_id"] = json!("missing");
    assert_eq!(
        call(
            &router,
            "PUT",
            "/api/workspaces/w",
            Some(&owner),
            Some(json!({"name":"Docs","expected_revision":saved["revision"],"data":data.clone()}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (_, unchanged) = call(&router, "GET", "/api/workspaces/w", Some(&owner), None).await;
    assert_eq!(unchanged["revision"], saved["revision"]);
    data["documents"][0]["collection_id"] = json!("c");
    data["documents"][0]["source"] = json!("# Updated");
    let (status, updated) = call(
        &router,
        "PUT",
        "/api/workspaces/w",
        Some(&owner),
        Some(json!({"name":"Docs","expected_revision":saved["revision"],"data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["data"]["documents"][0]["source"], "# Updated");
}
