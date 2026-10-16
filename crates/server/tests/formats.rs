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
