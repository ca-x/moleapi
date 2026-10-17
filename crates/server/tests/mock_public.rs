mod common;
use common::*;

#[tokio::test]
async fn public_schema_mock_requires_rotation_after_schema_change() {
    let dir = tempfile::tempdir().unwrap();
    let router = hosted(config(
        moleapi_server::sqlite_database_url(&dir.path().join("schema-share.db")).unwrap(),
        true,
    ))
    .await
    .unwrap();
    let owner = register(&router, "schemasalice").await;
    let mut data = example_data();
    data["collections"][0]["requests"][0]["examples"][0]["mock_schema"] =
        json!(r#"{"type":"object","properties":{"id":{"const":1}},"required":["id"]}"#);
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            Some(&owner),
            Some(json!({"id":"w","name":"W","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, share) = call(
        &router,
        "POST",
        "/api/workspaces/w/mock-shares",
        Some(&owner),
        Some(json!({"request_id":"r","expires_in_days":1})),
    )
    .await;
    let old = format!("/api/public/mock/{}/e", share["token"].as_str().unwrap());
    assert_eq!(
        call(&router, "GET", &old, None, None).await,
        (StatusCode::CREATED, json!({"id":1}))
    );
    let (_, mut saved) = call(&router, "GET", "/api/workspaces/w", Some(&owner), None).await;
    saved["data"]["collections"][0]["requests"][0]["examples"][0]["mock_schema"] =
        json!(r#"{"type":"object","properties":{"id":{"const":2}},"required":["id"]}"#);
    let (status, body) = call(
        &router,
        "PUT",
        "/api/workspaces/w",
        Some(&owner),
        Some(json!({"name":"W","expected_revision":saved["revision"],"data":saved["data"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        call(&router, "GET", &old, None, None).await.0,
        StatusCode::NOT_FOUND
    );
    let (_, rotated) = call(
        &router,
        "POST",
        &format!(
            "/api/workspaces/w/mock-shares/{}/rotate",
            share["id"].as_str().unwrap()
        ),
        Some(&owner),
        None,
    )
    .await;
    let new = format!("/api/public/mock/{}/e", rotated["token"].as_str().unwrap());
    assert_eq!(
        call(&router, "GET", &new, None, None).await,
        (StatusCode::CREATED, json!({"id":2}))
    );
}

#[tokio::test]
async fn owner_creates_lists_rotates_and_revokes_private_mock_share_capabilities() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("mock-shares.db");
    let router = hosted(moleapi_server::Config {
        database_url: moleapi_server::sqlite_database_url(&database).unwrap(),
        setup_token: "setup-secret".into(),
        allow_registration: true,
        allow_private_network: true,
    })
    .await
    .unwrap();
    let alice = register(&router, "sharealice").await;
    let bob = register(&router, "sharebobby").await;
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&alice),
        Some(json!({"id":"w","name":"W","data":example_data()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let path = "/api/workspaces/w/mock-shares";
    let input = json!({"request_id":"r","expires_in_days":30});
    assert_eq!(
        call(&router, "POST", path, None, Some(input.clone()))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&router, "POST", path, Some(&bob), Some(input.clone()))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &router,
            "POST",
            path,
            Some(&alice),
            Some(json!({"request_id":"r","expires_in_days":0}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (status, granted) = call(&router, "POST", path, Some(&alice), Some(input)).await;
    assert_eq!(status, StatusCode::OK, "{granted}");
    let token = granted["token"].as_str().unwrap();
    assert!(token.starts_with("msp_"));
    assert_eq!(token.len(), 68);
    assert_eq!(granted["request_id"], "r");
    assert_eq!(granted["allow_dynamic"], false);
    for include_secrets in [false, true] {
        let (status, backup) = call(
            &router,
            "POST",
            "/api/workspaces/w/export",
            Some(&alice),
            Some(json!({"format":"moleapi","include_secrets":include_secrets})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{backup}");
        assert!(!backup.to_string().contains(token));
        assert!(!backup.to_string().contains("mock-share"));
    }
    let id = granted["id"].as_str().unwrap();
    let (status, listed) = call(&router, "GET", path, Some(&alice), None).await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert!(!listed.to_string().contains(token));
    assert_eq!(
        call(&router, "GET", path, Some(&bob), None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&router, "GET", "/api/mock/w/r/e", None, None).await.0,
        StatusCode::UNAUTHORIZED
    );

    let db = sea_orm::Database::connect(moleapi_server::sqlite_database_url(&database).unwrap())
        .await
        .unwrap();
    use sea_orm::{ConnectionTrait, Statement};
    let row = db
        .query_one(Statement::from_string(
            sea_orm::DbBackend::Sqlite,
            "SELECT id, payload FROM documents WHERE kind = 'mock-share'".to_string(),
        ))
        .await
        .unwrap()
        .unwrap();
    let key: String = row.try_get("", "id").unwrap();
    let payload: String = row.try_get("", "payload").unwrap();
    assert!(!key.contains(token) && !payload.contains(token));

    let rotate = format!("{path}/{id}/rotate");
    assert_eq!(
        call(&router, "POST", &rotate, Some(&bob), None).await.0,
        StatusCode::NOT_FOUND
    );
    let (status, rotated) = call(&router, "POST", &rotate, Some(&alice), None).await;
    assert_eq!(status, StatusCode::OK, "{rotated}");
    let new_token = rotated["token"].as_str().unwrap();
    assert_ne!(new_token, token);
    assert_eq!(rotated["id"], id);
    assert!(
        !call(&router, "GET", path, Some(&alice), None)
            .await
            .1
            .to_string()
            .contains(new_token)
    );
    let revoke = format!("{path}/{id}");
    assert_eq!(
        call(&router, "DELETE", &revoke, Some(&alice), None).await.0,
        StatusCode::OK
    );
    assert!(
        call(&router, "GET", path, Some(&alice), None)
            .await
            .1
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn public_mock_routes_are_scoped_to_one_request_and_revoke_without_exposing_the_token() {
    let dir = tempfile::tempdir().unwrap();
    let router = hosted(moleapi_server::Config {
        database_url: moleapi_server::sqlite_database_url(&dir.path().join("routes.db")).unwrap(),
        setup_token: "setup-secret".into(),
        allow_registration: true,
        allow_private_network: true,
    })
    .await
    .unwrap();
    let owner = register(&router, "publicowner").await;
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = json!("https://example.test/items");
    let mut other = data["collections"][0]["requests"][0].clone();
    other["id"] = json!("other");
    other["url"] = json!("https://example.test/secret");
    other["examples"][0]["id"] = json!("other-example");
    other["examples"][0]["body"] = json!("other private body");
    data["collections"][0]["requests"]
        .as_array_mut()
        .unwrap()
        .push(other);
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            Some(&owner),
            Some(json!({"id":"w","name":"W","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, share) = call(
        &router,
        "POST",
        "/api/workspaces/w/mock-shares",
        Some(&owner),
        Some(json!({"request_id":"r","expires_in_days":7})),
    )
    .await;
    let token = share["token"].as_str().unwrap();
    let path = format!("/api/public/mock/{token}/e");
    let response = router
        .clone()
        .oneshot(Request::builder().uri(&path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["access-control-allow-origin"], "*");
    assert_eq!(
        to_bytes(response.into_body(), 100).await.unwrap().as_ref(),
        b"example body"
    );
    let routed = format!("/api/public/mocks/{token}/items");
    let response = router
        .clone()
        .oneshot(Request::builder().uri(&routed).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(
        to_bytes(response.into_body(), 100).await.unwrap().as_ref(),
        b"example body"
    );
    let foreign = format!("/api/public/mocks/{token}/secret");
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(&foreign)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert!(
        !String::from_utf8_lossy(&to_bytes(response.into_body(), 1000).await.unwrap())
            .contains(token)
    );
    assert_eq!(
        call(
            &router,
            "GET",
            &format!("/api/public/mock/{token}/other-example"),
            None,
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&router, "GET", "/api/public/mock/msp_invalid/e", None, None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri(&foreign)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(response.headers()["access-control-allow-origin"], "*");
    let id = share["id"].as_str().unwrap();
    assert_eq!(
        call(
            &router,
            "DELETE",
            &format!("/api/workspaces/w/mock-shares/{id}"),
            Some(&owner),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&router, "GET", &path, None, None).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn public_dynamic_mock_requires_explicit_permission_and_never_sees_the_share_token() {
    let dir = tempfile::tempdir().unwrap();
    let router = hosted(moleapi_server::Config {
        database_url: moleapi_server::sqlite_database_url(&dir.path().join("dynamic.db")).unwrap(),
        setup_token: "setup-secret".into(),
        allow_registration: true,
        allow_private_network: true,
    })
    .await
    .unwrap();
    let owner = register(&router, "dynamicowner").await;
    let mut data = example_data();
    data["script_mode"] = json!("v2");
    data["collections"][0]["requests"][0]["url"] = json!("https://example.test/items");
    data["collections"][0]["requests"][0]["examples"][0]["mock_script"] =
        json!("pm.mock.setBody(pm.request.url)");
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            Some(&owner),
            Some(json!({"id":"w","name":"W","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let endpoint = "/api/workspaces/w/mock-shares";
    let (status, denied) = call(
        &router,
        "POST",
        endpoint,
        Some(&owner),
        Some(json!({"request_id":"r","expires_in_days":7})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{denied}");
    assert!(denied.get("token").is_none());
    let (_, allowed) = call(
        &router,
        "POST",
        endpoint,
        Some(&owner),
        Some(json!({"request_id":"r","expires_in_days":7,"allow_dynamic":true})),
    )
    .await;
    let token = allowed["token"].as_str().unwrap();
    let path = format!("/api/public/mocks/{token}/items?mode=public");
    let response = router
        .clone()
        .oneshot(Request::builder().uri(&path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body =
        String::from_utf8(to_bytes(response.into_body(), 1000).await.unwrap().to_vec()).unwrap();
    assert!(body.contains("/mock/items?mode=public"), "{body}");
    assert!(!body.contains(token));
    let (_, mut saved) = call(&router, "GET", "/api/workspaces/w", Some(&owner), None).await;
    saved["data"]["collections"][0]["requests"][0]["examples"][0]["mock_script"] =
        json!("throw new Error('private-script-diagnostic')");
    assert_eq!(
        call(
            &router,
            "PUT",
            "/api/workspaces/w",
            Some(&owner),
            Some(json!({"name":"W","expected_revision":1,"data":saved["data"]}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, rotated) = call(
        &router,
        "POST",
        &format!("{endpoint}/{}/rotate", allowed["id"].as_str().unwrap()),
        Some(&owner),
        None,
    )
    .await;
    let failing_path = format!(
        "/api/public/mocks/{}/items",
        rotated["token"].as_str().unwrap()
    );
    let (status, error) = call(&router, "GET", &failing_path, None, None).await;
    assert!(status.is_client_error() || status.is_server_error());
    assert!(!error.to_string().contains("private-script-diagnostic"));
}

#[tokio::test]
async fn public_share_revocation_on_another_instance_fences_a_delayed_response() {
    let dir = tempfile::tempdir().unwrap();
    let config = moleapi_server::Config {
        database_url: moleapi_server::sqlite_database_url(&dir.path().join("revocation.db"))
            .unwrap(),
        setup_token: "setup-secret".into(),
        allow_registration: true,
        allow_private_network: true,
    };
    let owner_router = hosted(config.clone()).await.unwrap();
    let slots = std::sync::Arc::new(tokio::sync::Semaphore::new(32));
    let public_router = moleapi_server::hosted_with_mock_delay_slots_for_test(
        config,
        std::path::Path::new(env!("CARGO_BIN_EXE_moleapi-server")),
        slots.clone(),
    )
    .await
    .unwrap();
    let owner = register(&owner_router, "revokeowner").await;
    let mut data = example_data();
    data["collections"][0]["requests"][0]["examples"][0]["delay_ms"] = json!(5_000);
    assert_eq!(
        call(
            &owner_router,
            "POST",
            "/api/workspaces",
            Some(&owner),
            Some(json!({"id":"w","name":"W","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, share) = call(
        &owner_router,
        "POST",
        "/api/workspaces/w/mock-shares",
        Some(&owner),
        Some(json!({"request_id":"r","expires_in_days":7})),
    )
    .await;
    let path = format!("/api/public/mock/{}/e", share["token"].as_str().unwrap());
    let running_router = public_router.clone();
    let running_path = path.clone();
    let running =
        tokio::spawn(async move { call(&running_router, "GET", &running_path, None, None).await });
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while slots.available_permits() != 31 {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("public Mock did not enter its configured delay");
    let revoke = format!(
        "/api/workspaces/w/mock-shares/{}",
        share["id"].as_str().unwrap()
    );
    assert_eq!(
        call(&owner_router, "DELETE", &revoke, Some(&owner), None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(running.await.unwrap().0, StatusCode::NOT_FOUND);
    assert_eq!(
        call(&public_router, "GET", &path, None, None).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn replacing_a_request_with_the_same_id_requires_explicit_share_rotation() {
    let dir = tempfile::tempdir().unwrap();
    let router = hosted(moleapi_server::Config {
        database_url: moleapi_server::sqlite_database_url(&dir.path().join("request-binding.db"))
            .unwrap(),
        setup_token: "setup-secret".into(),
        allow_registration: true,
        allow_private_network: true,
    })
    .await
    .unwrap();
    let owner = register(&router, "bindingowner").await;
    let mut data = example_data();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            Some(&owner),
            Some(json!({"id":"w","name":"W","data":data.clone()}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, share) = call(
        &router,
        "POST",
        "/api/workspaces/w/mock-shares",
        Some(&owner),
        Some(json!({"request_id":"r","expires_in_days":7})),
    )
    .await;
    let old = format!("/api/public/mock/{}/e", share["token"].as_str().unwrap());
    assert_eq!(
        call(&router, "GET", &old, None, None).await.0,
        StatusCode::CREATED
    );
    let mut replacement = data["collections"][0]["requests"][0].clone();
    replacement["examples"][0]["body"] = json!("new content");
    data["collections"][0]["requests"] = json!([replacement.clone()]);
    assert_eq!(
        call(
            &router,
            "PUT",
            "/api/workspaces/w",
            Some(&owner),
            Some(json!({"name":"W","expected_revision":1,"data":data.clone()}))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&router, "GET", &old, None, None).await.0,
        StatusCode::NOT_FOUND
    );
    let shares = "/api/workspaces/w/mock-shares";
    let (_, listed) = call(&router, "GET", shares, Some(&owner), None).await;
    assert_eq!(listed[0]["stale"], true);
    let rotate = format!("{shares}/{}/rotate", share["id"].as_str().unwrap());
    let (status, rotated) = call(&router, "POST", &rotate, Some(&owner), None).await;
    assert_eq!(status, StatusCode::OK, "{rotated}");
    let fresh = format!("/api/public/mock/{}/e", rotated["token"].as_str().unwrap());
    let (status, body) = call(&router, "GET", &fresh, None, None).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["raw"], "new content");
    assert_eq!(
        call(&router, "GET", &old, None, None).await.0,
        StatusCode::NOT_FOUND
    );
    data["collections"][0]["requests"] = json!([]);
    assert_eq!(
        call(
            &router,
            "PUT",
            "/api/workspaces/w",
            Some(&owner),
            Some(json!({"name":"W","expected_revision":2,"data":data.clone()}))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&router, "GET", &fresh, None, None).await.0,
        StatusCode::NOT_FOUND
    );
    let (_, listed) = call(&router, "GET", shares, Some(&owner), None).await;
    assert!(listed.as_array().unwrap().is_empty());
    data["collections"][0]["requests"] = json!([replacement]);
    assert_eq!(
        call(
            &router,
            "PUT",
            "/api/workspaces/w",
            Some(&owner),
            Some(json!({"name":"W","expected_revision":3,"data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&router, "GET", &fresh, None, None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&router, "POST", &rotate, Some(&owner), None).await.0,
        StatusCode::NOT_FOUND
    );
    let (_, new_share) = call(
        &router,
        "POST",
        shares,
        Some(&owner),
        Some(json!({"request_id":"r","expires_in_days":7})),
    )
    .await;
    let newly_authorized = format!(
        "/api/public/mock/{}/e",
        new_share["token"].as_str().unwrap()
    );
    assert_eq!(
        call(&router, "GET", &newly_authorized, None, None).await.0,
        StatusCode::CREATED
    );
}

#[tokio::test]
async fn expired_public_mock_capability_cannot_call_or_rotate() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("expiry.db");
    let router = hosted(moleapi_server::Config {
        database_url: moleapi_server::sqlite_database_url(&database).unwrap(),
        setup_token: "setup-secret".into(),
        allow_registration: true,
        allow_private_network: true,
    })
    .await
    .unwrap();
    let owner = register(&router, "expiryowner").await;
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            Some(&owner),
            Some(json!({"id":"w","name":"W","data":example_data()}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, share) = call(
        &router,
        "POST",
        "/api/workspaces/w/mock-shares",
        Some(&owner),
        Some(json!({"request_id":"r","expires_in_days":1})),
    )
    .await;
    let path = format!("/api/public/mock/{}/e", share["token"].as_str().unwrap());
    assert_eq!(
        call(&router, "GET", &path, None, None).await.0,
        StatusCode::CREATED
    );
    let db = sea_orm::Database::connect(moleapi_server::sqlite_database_url(&database).unwrap())
        .await
        .unwrap();
    use sea_orm::{ConnectionTrait, Statement};
    let row = db
        .query_one(Statement::from_string(
            sea_orm::DbBackend::Sqlite,
            "SELECT id, payload FROM documents WHERE kind = 'mock-share'".to_string(),
        ))
        .await
        .unwrap()
        .unwrap();
    let key: String = row.try_get("", "id").unwrap();
    let mut payload: serde_json::Value =
        serde_json::from_str(&row.try_get::<String>("", "payload").unwrap()).unwrap();
    payload["expires_at"] = json!(chrono::Utc::now().timestamp() - 1);
    db.execute(Statement::from_sql_and_values(
        sea_orm::DbBackend::Sqlite,
        "UPDATE documents SET payload = ? WHERE id = ?",
        vec![payload.to_string().into(), key.into()],
    ))
    .await
    .unwrap();
    assert_eq!(
        call(&router, "GET", &path, None, None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &router,
            "POST",
            &format!(
                "/api/workspaces/w/mock-shares/{}/rotate",
                share["id"].as_str().unwrap()
            ),
            Some(&owner),
            None
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (status, fresh) = call(
        &router,
        "POST",
        "/api/workspaces/w/mock-shares",
        Some(&owner),
        Some(json!({"request_id":"r","expires_in_days":1})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{fresh}");
    let (_, listed) = call(
        &router,
        "GET",
        "/api/workspaces/w/mock-shares",
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert_eq!(
        call(&router, "GET", &path, None, None).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn rotation_cannot_reauthorize_a_non_http_request() {
    let dir = tempfile::tempdir().unwrap();
    let router = hosted(moleapi_server::Config {
        database_url: moleapi_server::sqlite_database_url(&dir.path().join("protocol.db")).unwrap(),
        setup_token: "setup-secret".into(),
        allow_registration: true,
        allow_private_network: true,
    })
    .await
    .unwrap();
    let owner = register(&router, "protocolowner").await;
    let mut data = example_data();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            Some(&owner),
            Some(json!({"id":"w","name":"W","data":data.clone()}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, share) = call(
        &router,
        "POST",
        "/api/workspaces/w/mock-shares",
        Some(&owner),
        Some(json!({"request_id":"r","expires_in_days":7})),
    )
    .await;
    data["collections"][0]["requests"][0]["protocol"] = json!({"kind":"sse"});
    let (status, updated) = call(
        &router,
        "PUT",
        "/api/workspaces/w",
        Some(&owner),
        Some(json!({"name":"W","expected_revision":1,"data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    let (status, rotated) = call(
        &router,
        "POST",
        &format!(
            "/api/workspaces/w/mock-shares/{}/rotate",
            share["id"].as_str().unwrap()
        ),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{rotated}");
    assert_eq!(
        call(
            &router,
            "GET",
            &format!("/api/public/mock/{}/e", share["token"].as_str().unwrap()),
            None,
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn public_mock_limits_concurrent_calls_before_matching_more_work() {
    let dir = tempfile::tempdir().unwrap();
    let slots = std::sync::Arc::new(tokio::sync::Semaphore::new(32));
    let router = moleapi_server::hosted_with_mock_delay_slots_for_test(
        moleapi_server::Config {
            database_url: moleapi_server::sqlite_database_url(&dir.path().join("capacity.db"))
                .unwrap(),
            setup_token: "setup-secret".into(),
            allow_registration: true,
            allow_private_network: true,
        },
        std::path::Path::new(env!("CARGO_BIN_EXE_moleapi-server")),
        slots.clone(),
    )
    .await
    .unwrap();
    let owner = register(&router, "capacityowner").await;
    let mut data = example_data();
    data["collections"][0]["requests"][0]["examples"][0]["delay_ms"] = json!(30_000);
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            Some(&owner),
            Some(json!({"id":"w","name":"W","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, share) = call(
        &router,
        "POST",
        "/api/workspaces/w/mock-shares",
        Some(&owner),
        Some(json!({"request_id":"r","expires_in_days":1})),
    )
    .await;
    let path = format!("/api/public/mock/{}/e", share["token"].as_str().unwrap());
    let mut calls = tokio::task::JoinSet::new();
    for _ in 0..32 {
        let router = router.clone();
        let path = path.clone();
        calls.spawn(async move { call(&router, "GET", &path, None, None).await.0 });
    }
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while slots.available_permits() != 0 {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("32 public requests did not reach the delay after admission");
    let (status, body) = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        call(&router, "GET", &path, None, None),
    )
    .await
    .expect("the 33rd public request did not receive a prompt capacity response");
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    calls.abort_all();
    while calls.join_next().await.is_some() {}
}
