mod common;
use common::*;
#[tokio::test]
async fn identical_storage_auth_cas_tests_on_every_configured_database() {
    let directory = tempfile::tempdir().unwrap();
    let mut urls = vec![format!(
        "sqlite://{}?mode=rwc",
        directory.path().join("server.db").display()
    )];
    for key in [
        "MOLEAPI_TEST_DATABASE_URL",
        "MOLEAPI_TEST_POSTGRES_URL",
        "MOLEAPI_TEST_MYSQL_URL",
    ] {
        if let Ok(url) = std::env::var(key) {
            urls.push(url);
        }
    }
    for url in urls {
        let router = common::hosted(config(url.clone(), true)).await.unwrap();
        if call(&router, "GET", "/api/auth/status", None, None).await.1["setup_required"] == true {
            let closed = common::hosted(config(url.clone(), false)).await.unwrap();
            let prefix = uuid::Uuid::new_v4().simple().to_string();
            let first = json!({"username":format!("admin_a_{prefix}"),"password":"goodpassword123","setup_token":"setup-secret"});
            let second = json!({"username":format!("admin_b_{prefix}"),"password":"goodpassword123","setup_token":"setup-secret"});
            let (first, second) = tokio::join!(
                call(&closed, "POST", "/api/auth/register", None, Some(first)),
                call(&closed, "POST", "/api/auth/register", None, Some(second))
            );
            assert!(
                (first.0 == StatusCode::OK && second.0 == StatusCode::FORBIDDEN)
                    || (second.0 == StatusCode::OK && first.0 == StatusCode::FORBIDDEN),
                "{first:?} {second:?}"
            );
        }
        // Running all migrations again must keep both schema and existing content.
        let router_again = common::hosted(config(url, true)).await.unwrap();
        assert_eq!(
            call(&router, "GET", "/api/health", None, None).await.0,
            StatusCode::OK
        );
        assert_eq!(
            call(&router, "GET", "/api/workspaces", None, None).await.0,
            StatusCode::UNAUTHORIZED
        );
        let prefix = uuid::Uuid::new_v4().simple().to_string();
        let username = format!("user_{prefix}");
        let a = register(&router, &username).await;
        let b = register(&router, &format!("other_{prefix}")).await;
        let (status, _) = call(
            &router,
            "POST",
            "/api/auth/register",
            None,
            Some(json!({"username":username,"password":"goodpassword123"})),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(
            call(
                &router,
                "POST",
                "/api/auth/login",
                None,
                Some(json!({"username":username,"password":"incorrect"}))
            )
            .await
            .0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            call(
                &router,
                "POST",
                "/api/auth/login",
                None,
                Some(json!({"username":username,"password":"goodpassword123"}))
            )
            .await
            .0,
            StatusCode::OK
        );
        let body = json!({"id":"shared-id","name":"Original","data":example_data()});
        let (status, w) = call(&router, "POST", "/api/workspaces", Some(&a), Some(body)).await;
        assert_eq!(status, StatusCode::OK, "{w}");
        assert_eq!(w["revision"], 1);
        // Run real callback persistence/privacy/CAS/cascade on every configured engine.
        let (status, receiver) = call(&router, "POST", "/api/workspaces/shared-id/webhooks", Some(&a),
            Some(json!({"name":"Database receiver","response":{"status":201,"body":"{\"received\":true}","headers":[]}}))).await;
        assert_eq!(status, StatusCode::OK, "{receiver}");
        let receiver_id = receiver["id"].as_str().unwrap();
        let receiver_path = receiver["receiver_path"].as_str().unwrap();
        assert_eq!(
            call(
                &router_again,
                "POST",
                receiver_path,
                None,
                Some(json!({"password":"database-private","copied":"database-private"}))
            )
            .await
            .0,
            StatusCode::CREATED
        );
        let capture_path = format!("/api/webhooks/{receiver_id}/captures");
        let (status, captures) = call(&router, "GET", &capture_path, Some(&a), None).await;
        assert_eq!(status, StatusCode::OK, "{captures}");
        assert_eq!(captures["captures"].as_array().unwrap().len(), 1);
        assert!(!captures.to_string().contains("database-private"));
        assert_eq!(
            call(&router, "GET", &capture_path, Some(&b), None).await.0,
            StatusCode::NOT_FOUND
        );
        let (status, stale) = call(
            &router,
            "PUT",
            &format!("/api/webhooks/{receiver_id}"),
            Some(&a),
            Some(
                json!({"name":"Stale receiver","active":false,"response":{},"expected_revision":1}),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{stale}");

        assert_eq!(
            call(
                &router_again,
                "GET",
                "/api/workspaces/shared-id",
                Some(&a),
                None
            )
            .await
            .0,
            StatusCode::OK
        );
        for path in [
            "/api/workspaces/shared-id",
            "/api/workspaces/shared-id/history",
            "/api/workspaces/shared-id/versions",
            "/api/mock/shared-id/r/e",
        ] {
            assert_eq!(
                call(&router, "GET", path, Some(&b), None).await.0,
                StatusCode::NOT_FOUND,
                "{path}"
            );
        }
        assert_eq!(
            call(
                &router,
                "POST",
                "/api/workspaces/shared-id/run",
                Some(&b),
                Some(json!({"collection_id":"c"}))
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
        let (status, _) = call(
            &router,
            "POST",
            "/api/workspaces",
            Some(&b),
            Some(json!({"id":"shared-id","name":"Own tenant","data":data()})),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        // Oversized-for-MySQL-TEXT payload checks portable large JSON storage.
        let mut large = example_data();
        large["collections"][0]["description"] = json!("z".repeat(90_000));
        let update = json!({"name":"Updated","data":large,"expected_revision":1});
        let (one, two) = tokio::join!(
            call(
                &router,
                "PUT",
                "/api/workspaces/shared-id",
                Some(&a),
                Some(update.clone())
            ),
            call(
                &router,
                "PUT",
                "/api/workspaces/shared-id",
                Some(&a),
                Some(update)
            )
        );
        assert!(
            (one.0 == StatusCode::OK && two.0 == StatusCode::CONFLICT)
                || (two.0 == StatusCode::OK && one.0 == StatusCode::CONFLICT),
            "{one:?} {two:?}"
        );
        let (status, versions) = call(
            &router,
            "GET",
            "/api/workspaces/shared-id/versions",
            Some(&a),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(versions.as_array().unwrap().len(), 1);
        assert_eq!(versions[0]["name"], "Original");
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/mock/shared-id/r/e")
                    .header("authorization", format!("Bearer {a}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(response.headers()["x-example"], "yes");
        assert!(!response.headers().contains_key("connection"));
        assert!(!response.headers().contains_key("x-hop"));
        assert_eq!(
            to_bytes(response.into_body(), 100).await.unwrap().as_ref(),
            b"example body"
        );
        assert_eq!(
            call(
                &router,
                "DELETE",
                "/api/workspaces/shared-id",
                Some(&a),
                Some(json!({"expected_revision":1}))
            )
            .await
            .0,
            StatusCode::CONFLICT
        );
        assert_eq!(
            call(
                &router,
                "DELETE",
                "/api/workspaces/shared-id",
                Some(&a),
                Some(json!({"expected_revision":2}))
            )
            .await
            .0,
            StatusCode::OK
        );
        assert_eq!(
            call(&router, "GET", &capture_path, Some(&a), None).await.0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            call(&router, "POST", receiver_path, None, Some(json!({})))
                .await
                .0,
            StatusCode::NOT_FOUND
        );
        let (recreated_status, recreated) = call(
            &router,
            "POST",
            "/api/workspaces",
            Some(&a),
            Some(json!({"id":"shared-id","name":"Recreated","data":data()})),
        )
        .await;
        assert_eq!(recreated_status, StatusCode::OK, "{recreated}");
        assert!(
            recreated["revision"].as_i64().unwrap() > 2,
            "recreation must not reset revisions on any database"
        );
        assert_eq!(
            call(
                &router,
                "PUT",
                "/api/workspaces/shared-id",
                Some(&a),
                Some(json!({"name":"Stale","data":data(),"expected_revision":2}))
            )
            .await
            .0,
            StatusCode::CONFLICT
        );
        assert_eq!(
            call(&router, "GET", "/api/workspaces/shared-id", Some(&a), None)
                .await
                .1["name"],
            "Recreated"
        );
        assert_eq!(
            call(&router, "GET", "/api/workspaces/shared-id", Some(&b), None)
                .await
                .0,
            StatusCode::OK
        );
        assert_eq!(
            call(&router, "GET", "/api/sync/status", Some(&a), None)
                .await
                .0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            call(&router, "POST", "/api/auth/logout", Some(&a), None)
                .await
                .0,
            StatusCode::OK
        );
        assert_eq!(
            call(&router, "GET", "/api/workspaces", Some(&a), None)
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            call(&router, "GET", "/api/nonexistent", None, None).await.0,
            StatusCode::NOT_FOUND
        );
    }
}

#[tokio::test]
async fn recreation_keeps_monotonic_revisions_so_stale_updates_cannot_overwrite_it() {
    let directory = tempfile::tempdir().unwrap();
    let router = common::local(&directory.path().join("recreate.db"))
        .await
        .unwrap();
    let (_, first) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"reused","name":"Original","data":data()})),
    )
    .await;
    assert_eq!(first["revision"], 1);
    assert_eq!(
        call(
            &router,
            "DELETE",
            "/api/workspaces/reused",
            None,
            Some(json!({"expected_revision":1}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, recreated) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"reused","name":"New incarnation","data":data()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{recreated}");
    assert!(
        recreated["revision"].as_i64().unwrap() > 1,
        "revision must not reset after recreation"
    );
    assert_eq!(
        call(
            &router,
            "PUT",
            "/api/workspaces/reused",
            None,
            Some(json!({"name":"Stale client","data":data(),"expected_revision":1}))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(&router, "GET", "/api/workspaces/reused", None, None)
            .await
            .1["name"],
        "New incarnation"
    );
}
