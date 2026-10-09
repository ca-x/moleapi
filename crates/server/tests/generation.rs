mod common;
use common::*;
#[tokio::test]
async fn generation_is_owned_private_by_default_and_preserves_explicit_originals() {
    let temp = tempfile::tempdir().unwrap();
    let router = hosted(config(
        format!(
            "sqlite://{}?mode=rwc",
            temp.path().join("codegen.db").display()
        ),
        true,
    ))
    .await
    .unwrap();
    let owner = register(&router, "generator").await;
    let stranger = register(&router, "stranger").await;
    let mut data = example_data();
    data["global_variables"] = json!([{"id":"secret","key":"seed","value":"private+/ original","local_value":"private local override","enabled":true,"secret":true}]);
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] =
        json!("https://example.com/echo?token=query-secret&copy=private%2B%2F+original");
    request["method"] = json!("POST");
    request["body_kind"] = json!("json");
    request["body"] = json!(
        "{\"password\":\"body-secret\",\"copy\":\"body-secret / private+/ original\",\"template\":\"{{environment}}\"}"
    );
    request["auth"] = json!({"kind":"bearer","token":"auth-secret","username":"","password":""});
    request["headers"] = json!([{"id":"h","key":"X-Copy","value":"private local override","enabled":true,"secret":true}]);
    let (status, _) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Generation","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let payload = json!({"workspace_id":"w","request_id":"r","target":"node","client":"fetch"});
    let (status, _) = call(
        &router,
        "POST",
        "/api/generation/snippets",
        None,
        Some(payload.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = call(
        &router,
        "POST",
        "/api/generation/snippets",
        Some(&stranger),
        Some(payload.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, result) = call(
        &router,
        "POST",
        "/api/generation/snippets",
        Some(&owner),
        Some(payload.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let code = result["code"].as_str().unwrap();
    for secret in [
        "private+/ original",
        "private%2B%2F+original",
        "private local override",
        "uri-password",
        "query-secret",
        "body-secret",
        "auth-secret",
    ] {
        assert!(!code.contains(secret), "leaked {secret}: {code}");
    }
    assert!(code.contains("{{environment}}"));
    assert_eq!(result["include_secrets"], false);
    let mut payload = payload;
    payload["include_secrets"] = json!(true);
    let (status, result) = call(
        &router,
        "POST",
        "/api/generation/snippets",
        Some(&owner),
        Some(payload),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert!(result["code"].as_str().unwrap().contains("auth-secret"));
    assert!(result["code"].as_str().unwrap().contains("body-secret"));
    let (status, catalog) = call(
        &router,
        "GET",
        "/api/generation/snippets/catalog",
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(catalog["targets"].as_array().unwrap().len(), 23);
}
#[tokio::test]
async fn local_native_router_generation_rejects_missing_requests_and_invalid_targets() {
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("generation.db")).await.unwrap();
    let (status, _) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"Generation","data":example_data()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    for (request, target) in [("missing", "shell"), ("r", "missing")] {
        let (status, value) = call(
            &router,
            "POST",
            "/api/generation/snippets",
            None,
            Some(json!({"workspace_id":"w","request_id":request,"target":target,"client":"curl"})),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{value}");
    }
}
