mod common;
use common::*;
#[tokio::test]
async fn import_export_routes_validate_formats_and_default_to_secret_redaction() {
    let dir = tempfile::tempdir().unwrap();
    let router = common::local(&dir.path().join("local.db")).await.unwrap();
    let(status,imported)=call(&router,"POST","/api/import",None,Some(json!({"format":"curl","content":"curl -X POST 'https://example.com/resource?api_key=private-key' -H 'Authorization: Bearer private-token' --data-raw 'payload'"}))).await;
    assert_eq!(status, StatusCode::OK, "{imported}");
    assert_eq!(
        imported["data"]["collections"][0]["requests"][0]["method"],
        "POST"
    );
    let (status, _) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"import","name":"Imported","data":imported["data"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, exported) = call(
        &router,
        "POST",
        "/api/workspaces/import/export",
        None,
        Some(json!({"format":"moleapi","include_secrets":false})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{exported}");
    assert!(
        !exported["content"]
            .as_str()
            .unwrap()
            .contains("private-token")
    );
    assert!(
        !exported["content"]
            .as_str()
            .unwrap()
            .contains("private-key")
    );
    assert_eq!(exported["mime"], "application/json");
    let (_, included) = call(
        &router,
        "POST",
        "/api/workspaces/import/export",
        None,
        Some(json!({"format":"moleapi","include_secrets":true})),
    )
    .await;
    assert!(
        included["content"]
            .as_str()
            .unwrap()
            .contains("private-token")
    );
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/import",
            None,
            Some(json!({"format":"unknown","content":"payload"}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces/missing/export",
            None,
            Some(json!({"format":"moleapi","include_secrets":false}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn owned_postman_variable_preview_and_export_preserve_cas_and_privacy() {
    let temporary = tempfile::tempdir().unwrap();
    let router = hosted(config(
        moleapi_server::sqlite_database_url(&temporary.path().join("variables.db")).unwrap(),
        true,
    ))
    .await
    .unwrap();
    let owner = register(&router, "variable-owner").await;
    let other = register(&router, "variable-other").await;
    let (status, workspace) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"variable-workspace","name":"Variable workspace","data":data()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{workspace}");
    let environment = include_str!("../../formats/tests/fixtures/postman_environment.json");
    let preview_path = "/api/workspaces/variable-workspace/variables/import-preview";
    let (status, preview) = call(
        &router,
        "POST",
        preview_path,
        Some(&owner),
        Some(json!({"content":environment,"expected_revision":workspace["revision"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["scope"], "environment");
    assert_eq!(preview["variables"].as_array().unwrap().len(), 3);
    assert_eq!(
        call(
            &router,
            "POST",
            preview_path,
            Some(&other),
            Some(json!({"content":environment,"expected_revision":workspace["revision"]}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let mut source = workspace["data"].clone();
    source["environments"] =
        json!([{"id":"dev","name":preview["name"],"variables":preview["variables"]}]);
    source["active_environment_id"] = json!("dev");
    let (status, saved) = call(&router,"PUT","/api/workspaces/variable-workspace",Some(&owner),
        Some(json!({"name":"Variable workspace","data":source,"expected_revision":workspace["revision"]}))).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(
        call(
            &router,
            "POST",
            preview_path,
            Some(&owner),
            Some(json!({"content":environment,"expected_revision":workspace["revision"]}))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let export_path = "/api/workspaces/variable-workspace/variables/export";
    let request = |include_secrets| json!({"scope":"environment","environment_id":"dev","include_secrets":include_secrets,"expected_revision":saved["revision"]});
    let (status, safe) = call(
        &router,
        "POST",
        export_path,
        Some(&owner),
        Some(request(false)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{safe}");
    assert!(
        !safe["content"]
            .as_str()
            .unwrap()
            .contains("environment-secret")
    );
    let (status, full) = call(
        &router,
        "POST",
        export_path,
        Some(&owner),
        Some(request(true)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{full}");
    assert!(
        full["content"]
            .as_str()
            .unwrap()
            .contains("environment-secret")
    );
    assert_eq!(
        call(
            &router,
            "POST",
            export_path,
            Some(&other),
            Some(request(true))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(call(&router,"POST",export_path,Some(&owner),
        Some(json!({"scope":"environment","environment_id":"dev","include_secrets":false,"expected_revision":workspace["revision"]}))).await.0,StatusCode::CONFLICT);
    let globals = include_str!("../../formats/tests/fixtures/postman_globals.json");
    let (status, globals_preview) = call(
        &router,
        "POST",
        preview_path,
        Some(&owner),
        Some(json!({"content":globals,"expected_revision":saved["revision"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{globals_preview}");
    assert_eq!(globals_preview["scope"], "globals");
    let mut next = saved["data"].clone();
    next["global_variables"] = globals_preview["variables"].clone();
    let (status, saved) = call(
        &router,
        "PUT",
        "/api/workspaces/variable-workspace",
        Some(&owner),
        Some(
            json!({"name":"Variable workspace","data":next,"expected_revision":saved["revision"]}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (status, exported)=call(&router,"POST",export_path,Some(&owner),
        Some(json!({"scope":"globals","environment_id":null,"include_secrets":false,"expected_revision":saved["revision"]}))).await;
    assert_eq!(status, StatusCode::OK, "{exported}");
    assert_eq!(
        serde_json::from_str::<Value>(exported["content"].as_str().unwrap()).unwrap()["_postman_variable_scope"],
        "globals"
    );
}

#[tokio::test]
async fn hosted_har_import_creates_owned_workspace_and_screens_recorded_secrets() {
    let dir = tempfile::tempdir().unwrap();
    let router = hosted(config(
        moleapi_server::sqlite_database_url(&dir.path().join("har.db")).unwrap(),
        true,
    ))
    .await
    .unwrap();
    let owner = register(&router, "har-owner").await;
    let other = register(&router, "har-other").await;
    let content = include_str!("../../formats/tests/fixtures/capture.har");
    let (status, imported) = call(
        &router,
        "POST",
        "/api/import",
        Some(&owner),
        Some(json!({"format":"har","content":content})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{imported}");
    assert_eq!(
        imported["data"]["collections"][0]["requests"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        imported["data"]["collections"][0]["requests"][0]["examples"][0]["status"],
        200
    );
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"har-owned","name":imported["name"],"data":imported["data"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (status, safe) = call(
        &router,
        "POST",
        "/api/workspaces/har-owned/export",
        Some(&owner),
        Some(json!({"format":"moleapi","include_secrets":false})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{safe}");
    let source = safe["content"].as_str().unwrap();
    assert!(!source.contains("har-secret"));
    assert!(!source.contains("cookie-secret"));
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces/har-owned/export",
            Some(&other),
            Some(json!({"format":"moleapi","include_secrets":true}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let mut unsupported: Value = serde_json::from_str(content).unwrap();
    unsupported["log"]["entries"][2]["request"]["postData"]["mimeType"] =
        json!("multipart/form-data");
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/import",
            Some(&owner),
            Some(json!({"format":"har","content":unsupported.to_string()}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn imported_har_requests_replay_the_recorded_query_headers_and_form_body() {
    let (url, fixture) = serve(
        Router::new()
            .route(
                "/items",
                axum::routing::get(
                    |query: axum::extract::RawQuery, headers: axum::http::HeaderMap| async move {
                        assert_eq!(query.0.as_deref(), Some("tag=one&tag=two"));
                        assert_eq!(headers["authorization"], "Bearer har-secret");
                        assert_eq!(headers["cookie"], "sid=cookie-secret");
                        assert_ne!(headers["host"], "api.example.test");
                        axum::Json(json!({"received":"query"}))
                    },
                ),
            )
            .route(
                "/form",
                axum::routing::post(
                    |headers: axum::http::HeaderMap, body: axum::body::Bytes| async move {
                        assert_eq!(headers["content-type"], "application/x-www-form-urlencoded");
                        assert_eq!(body.as_ref(), b"k=one&k=two");
                        axum::Json(json!({"received":"form"}))
                    },
                ),
            ),
    )
    .await;
    let dir = tempfile::tempdir().unwrap();
    let mut settings = config(
        moleapi_server::sqlite_database_url(&dir.path().join("har-wire.db")).unwrap(),
        true,
    );
    settings.allow_private_network = true;
    let router = hosted(settings).await.unwrap();
    let owner = register(&router, "har-wire-owner").await;
    let mut source: Value =
        serde_json::from_str(include_str!("../../formats/tests/fixtures/capture.har")).unwrap();
    source["log"]["entries"][0]["request"]["url"] = json!(format!("{url}/items?tag=one&tag=two"));
    source["log"]["entries"][1]["request"]["url"] = json!(format!("{url}/form"));
    source["log"]["entries"][2]["request"]["url"] = json!(format!("{url}/json"));
    let (status, imported) = call(
        &router,
        "POST",
        "/api/import",
        Some(&owner),
        Some(json!({"format":"har","content":source.to_string()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{imported}");
    let requests = &imported["data"]["collections"][0]["requests"];
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"har-wire","name":"HAR wire","data":imported["data"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    for (index, expected) in [(0, "query"), (1, "form")] {
        let (status, result) = call(
            &router,
            "POST",
            "/api/execute",
            Some(&owner),
            Some(json!({"workspace_id":"har-wire","request":requests[index]})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{result}");
        assert!(result["body"].as_str().unwrap().contains(expected));
    }
    fixture.abort();
}

#[tokio::test]
async fn hosted_postman20_import_preserves_object_auth_and_owner_private_source() {
    let (url, fixture) = serve(
        Router::new()
            .route(
                "/accounts/1",
                axum::routing::get(|headers: axum::http::HeaderMap| async move {
                    assert_eq!(headers["authorization"], "Bearer bearer-secret");
                    axum::Json(json!({"ok":"bearer"}))
                }),
            )
            .route(
                "/accounts",
                axum::routing::post(
                    |headers: axum::http::HeaderMap, body: axum::body::Bytes| async move {
                        assert_eq!(headers["x-api-key"], "api-key-secret");
                        assert_eq!(body.as_ref(), br#"{"enabled":false}"#);
                        axum::Json(json!({"ok":"apikey"}))
                    },
                ),
            ),
    )
    .await;
    let dir = tempfile::tempdir().unwrap();
    let mut settings = config(
        moleapi_server::sqlite_database_url(&dir.path().join("postman20.db")).unwrap(),
        true,
    );
    settings.allow_private_network = true;
    let router = hosted(settings).await.unwrap();
    let owner = register(&router, "postman20-owner").await;
    let other = register(&router, "postman20-other").await;
    let mut source: Value =
        serde_json::from_str(include_str!("../../formats/tests/fixtures/postman20.json")).unwrap();
    source["item"][0]["item"][0]["request"]["url"] = json!(format!("{url}/accounts/1"));
    source["item"][0]["item"][1]["request"]["url"] = json!(format!("{url}/accounts"));
    let source = source.to_string();
    let (status, imported) = call(
        &router,
        "POST",
        "/api/import",
        Some(&owner),
        Some(json!({"format":"postman","content":source})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{imported}");
    assert_eq!(imported["data"]["specifications"][0]["dialect"], "2.0.0");
    assert_eq!(imported["data"]["specifications"][0]["source"], source);
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"postman20","name":"Postman 2.0","data":imported["data"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let requests = &saved["data"]["collections"];
    let nested = requests
        .as_array()
        .unwrap()
        .iter()
        .find(|collection| collection["name"] == "Accounts")
        .unwrap();
    for (index, expected) in [(0, "bearer"), (1, "apikey")] {
        let (status, result) = call(
            &router,
            "POST",
            "/api/execute",
            Some(&owner),
            Some(json!({"workspace_id":"postman20","request":nested["requests"][index]})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{result}");
        assert!(result["body"].as_str().unwrap().contains(expected));
    }
    let (status, safe) = call(
        &router,
        "POST",
        "/api/workspaces/postman20/export",
        Some(&owner),
        Some(json!({"format":"moleapi","include_secrets":false})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{safe}");
    assert!(
        !safe["content"]
            .as_str()
            .unwrap()
            .contains("collection-secret")
    );
    assert!(!safe["content"].as_str().unwrap().contains("bearer-secret"));
    assert!(!safe["content"].as_str().unwrap().contains("api-key-secret"));
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces/postman20/export",
            Some(&other),
            Some(json!({"format":"moleapi","include_secrets":true}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    fixture.abort();
}
