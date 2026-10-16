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
