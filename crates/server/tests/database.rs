mod common;
#[path = "data_database.rs"]
mod data_database;
use common::*;
use std::time::Duration;
#[tokio::test]
async fn identical_storage_auth_cas_tests_on_every_configured_database() {
    data_database::verify_configured_data_clients().await;
    verify_data_completion_history().await;
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
        let backend = url.split("://").next().unwrap_or("unknown").to_owned();
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
        let published_id = format!("published-{prefix}");
        let docs = json!({"schema_version":1,"collections":[],"environments":[],"active_environment_id":null,
            "documents":[{"id":"guide","name":"Guide","source":"# Published across instances","secret":false}]});
        let (status, published_workspace) = call(
            &router,
            "POST",
            "/api/workspaces",
            Some(&a),
            Some(json!({"id":published_id,"name":"Private workspace","data":docs})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{published_workspace}");
        let invite_path = format!("/api/workspaces/{published_id}/invitations");
        let (status, invitation) = call(
            &router,
            "POST",
            &invite_path,
            Some(&a),
            Some(json!({"role":"viewer","expires_in_days":1})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{invitation}");
        let (status, accepted) = call(
            &router_again,
            "POST",
            "/api/workspace-invitations/accept",
            Some(&b),
            Some(json!({"token":invitation["token"]})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{accepted}");
        let owner_id = invitation["owner_id"].as_str().unwrap();
        let shared_path = format!("/api/owners/{owner_id}/workspaces/{published_id}");
        let (status, projected) = call(&router_again, "GET", &shared_path, Some(&b), None).await;
        assert_eq!(status, StatusCode::OK, "{projected}");
        assert_eq!(projected["id"], published_id);
        assert_eq!(
            call(
                &router,
                "DELETE",
                &format!(
                    "{shared_path}/members/{}",
                    accepted["member_id"].as_str().unwrap()
                ),
                Some(&a),
                None
            )
            .await
            .0,
            StatusCode::OK
        );
        assert_eq!(
            call(&router_again, "GET", &shared_path, Some(&b), None)
                .await
                .0,
            StatusCode::NOT_FOUND
        );
        let publications_path = format!("/api/workspaces/{published_id}/doc-publications");
        let publication_input = json!({"title":"Public guides","expected_revision":published_workspace["revision"],"document_ids":["guide"],"idempotency_key":"same-key","appearance":{"theme":"dark","layout":"top"}});
        let (status, publication) = call(
            &router,
            "POST",
            &publications_path,
            Some(&a),
            Some(publication_input.clone()),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{publication}");
        let (status, retry) = call(
            &router_again,
            "POST",
            &publications_path,
            Some(&a),
            Some(publication_input),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{retry}");
        assert_eq!(retry["id"], publication["id"]);
        assert_eq!(
            retry["appearance"],
            json!({"theme":"dark","layout":"top","has_logo":false})
        );
        let publication_id = publication["id"].as_str().unwrap();
        let public_path = format!("/api/public/docs/{publication_id}");
        let (status, public) = call(&router_again, "GET", &public_path, None, None).await;
        assert_eq!(status, StatusCode::OK, "{public}");
        assert_eq!(public["appearance"]["theme"], "dark");
        assert_eq!(public["appearance"]["layout"], "top");
        assert_eq!(
            public["documents"][0]["source"],
            "# Published across instances"
        );
        assert_eq!(
            call(&router_again, "GET", &publications_path, Some(&b), None)
                .await
                .0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            call(
                &router_again,
                "DELETE",
                &format!("{publications_path}/{publication_id}"),
                Some(&a),
                None,
            )
            .await
            .0,
            StatusCode::OK
        );
        assert_eq!(
            call(&router, "GET", &public_path, None, None).await.0,
            StatusCode::NOT_FOUND
        );
        let mut active_publications = Vec::new();
        for index in 0..19 {
            let (status, item) = call(
                &router,
                "POST",
                &publications_path,
                Some(&a),
                Some(json!({"title":"Public guides","expected_revision":published_workspace["revision"],"document_ids":["guide"],"idempotency_key":format!("limit-{index}")})),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{item}");
            active_publications.push(item["id"].as_str().unwrap().to_owned());
        }
        let (last, over_limit) = tokio::join!(
            call(
                &router,
                "POST",
                &publications_path,
                Some(&a),
                Some(
                    json!({"title":"Public guides","expected_revision":published_workspace["revision"],"document_ids":["guide"],"idempotency_key":"limit-19"})
                ),
            ),
            call(
                &router_again,
                "POST",
                &publications_path,
                Some(&a),
                Some(
                    json!({"title":"Public guides","expected_revision":published_workspace["revision"],"document_ids":["guide"],"idempotency_key":"limit-20"})
                ),
            )
        );
        let raced = [last, over_limit];
        assert_eq!(
            raced
                .iter()
                .filter(|reply| reply.0 == StatusCode::OK)
                .count(),
            1,
            "{raced:?}"
        );
        assert_eq!(
            raced
                .iter()
                .filter(|reply| reply.0 == StatusCode::BAD_REQUEST)
                .count(),
            1,
            "{raced:?}"
        );
        active_publications.push(
            raced
                .iter()
                .find(|reply| reply.0 == StatusCode::OK)
                .unwrap()
                .1["id"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
        for id in active_publications {
            let (status, body) = call(
                &router,
                "DELETE",
                &format!("{publications_path}/{id}"),
                Some(&a),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{body}");
        }
        for index in 21..256 {
            let (status, item) = call(
                &router,
                "POST",
                &publications_path,
                Some(&a),
                Some(json!({"title":"Public guides","expected_revision":published_workspace["revision"],"document_ids":["guide"],"idempotency_key":format!("limit-{index}")})),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{item}");
            let (status, body) = call(
                &router,
                "DELETE",
                &format!("{publications_path}/{}", item["id"].as_str().unwrap()),
                Some(&a),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{body}");
        }
        let (_, publication_history) =
            call(&router_again, "GET", &publications_path, Some(&a), None).await;
        assert_eq!(publication_history.as_array().unwrap().len(), 256);
        assert!(
            publication_history
                .as_array()
                .unwrap()
                .iter()
                .all(|item| item["revoked"] == true)
        );
        assert_eq!(
            call(
                &router,
                "POST",
                &publications_path,
                Some(&a),
                Some(json!({"title":"Public guides","expected_revision":published_workspace["revision"],"document_ids":["guide"],"idempotency_key":"limit-256"})),
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
        let password_workspace = format!("protected-{prefix}");
        let (status, protected_workspace) = call(
            &router,
            "POST",
            "/api/workspaces",
            Some(&a),
            Some(json!({"id":password_workspace,"name":"Protected","data":{"schema_version":1,"collections":[],"environments":[],"active_environment_id":null,"documents":[{"id":"guide","name":"Guide","source":"# Protected","secret":false}]}})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{protected_workspace}");
        let protected_path = format!("/api/workspaces/{password_workspace}/doc-publications");
        let (status, protected) = call(
            &router,
            "POST",
            &protected_path,
            Some(&a),
            Some(json!({"title":"Protected guide","expected_revision":protected_workspace["revision"],"document_ids":["guide"],"idempotency_key":"password-rate","visibility":"password","password":"correct-password-123"})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{protected}");
        let protected_id = protected["id"].as_str().unwrap();
        let unlock_path = format!("/api/public/docs/{protected_id}/unlock");
        for index in 0..10 {
            let instance = if index % 2 == 0 {
                &router
            } else {
                &router_again
            };
            let (status, body) = call(
                instance,
                "POST",
                &unlock_path,
                None,
                Some(json!({"password":"wrong-password"})),
            )
            .await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
            assert!(!body.to_string().contains("Protected guide"));
        }
        let limited = router_again
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&unlock_path)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"password":"correct-password-123"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(limited.headers().get("retry-after").is_some());
        assert_eq!(
            call(
                &router,
                "DELETE",
                &format!("{protected_path}/{protected_id}"),
                Some(&a),
                None,
            )
            .await
            .0,
            StatusCode::OK
        );
        assert_eq!(
            call(
                &router_again,
                "POST",
                &unlock_path,
                None,
                Some(json!({"password":"correct-password-123"}))
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
        verify_workspace_restore(&router, &router_again, &a, &b, &prefix, &backend).await;
        let rate_id = format!("rate-{prefix}");
        let mut rate_data = example_data();
        rate_data["collections"][0]["requests"][0]["examples"][0]["rate_limit_per_minute"] =
            json!(1);
        let (status, created) = call(
            &router,
            "POST",
            "/api/workspaces",
            Some(&a),
            Some(json!({"id":rate_id,"name":"Rate","data":rate_data.clone()})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{created}");
        let path = format!("/api/mock/{rate_id}/r/e");
        let (one, two) = tokio::join!(
            call(&router, "GET", &path, Some(&a), None),
            call(&router_again, "GET", &path, Some(&a), None)
        );
        assert_eq!(
            [one.0, two.0]
                .iter()
                .filter(|status| **status == StatusCode::CREATED)
                .count(),
            1
        );
        assert_eq!(
            [one.0, two.0]
                .iter()
                .filter(|status| **status == StatusCode::TOO_MANY_REQUESTS)
                .count(),
            1
        );
        let (status, updated) = call(
            &router,
            "PUT",
            &format!("/api/workspaces/{rate_id}"),
            Some(&a),
            Some(json!({"name":"Rate","expected_revision":created["revision"],"data":rate_data})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{updated}");
        assert_eq!(
            call(&router_again, "GET", &path, Some(&a), None).await.0,
            StatusCode::CREATED
        );
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
        // Private OAuth1 documents use the same transactions on every configured engine.
        let (status,oauth1)=call(&router,"POST","/api/oauth1/tokens/import",Some(&a),Some(json!({"workspace_id":"shared-id","config":{"consumer_key":"database-consumer"},"label":"Database token","token":{"token":"database-private-oauth1","secret":"database-private-secret"}}))).await;
        assert_eq!(status, StatusCode::OK, "{oauth1}");
        assert!(!oauth1.to_string().contains("database-private-oauth1"));
        let oauth1_id = oauth1["id"].as_str().unwrap();
        let oauth1_path = format!("/api/workspaces/shared-id/oauth1/tokens/{oauth1_id}");
        let (status, secret) = call(
            &router_again,
            "GET",
            &format!("{oauth1_path}/secret"),
            Some(&a),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{secret}");
        assert_eq!(secret["token"], "database-private-oauth1");
        assert_eq!(
            call(
                &router,
                "GET",
                &format!("{oauth1_path}/secret"),
                Some(&b),
                None
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
        let (status, renamed) = call(
            &router,
            "PATCH",
            &oauth1_path,
            Some(&a),
            Some(json!({"label":"Renamed database token"})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{renamed}");
        assert_eq!(renamed["label"], "Renamed database token");
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
        let (one, two) = tokio::join!(
            call(&router, "GET", "/api/mock/shared-id/r/e", Some(&a), None),
            call(
                &router_again,
                "GET",
                "/api/mock/shared-id/r/e",
                Some(&a),
                None
            ),
        );
        assert_eq!(one.0, StatusCode::CREATED, "{one:?}");
        assert_eq!(two.0, StatusCode::CREATED, "{two:?}");
        let (status, mock_calls) = call(
            &router_again,
            "GET",
            "/api/workspaces/shared-id/mock-calls",
            Some(&a),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{mock_calls}");
        assert_eq!(mock_calls.as_array().unwrap().len(), 3);
        assert_eq!(mock_calls[0]["sequence"], 3);
        assert_eq!(mock_calls[0]["response_status"], 201);
        assert!(!mock_calls.to_string().contains("example body"));
        let (status, other_calls) = call(
            &router_again,
            "GET",
            "/api/workspaces/shared-id/mock-calls",
            Some(&b),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(other_calls.as_array().unwrap().is_empty());
        let shares = "/api/workspaces/shared-id/mock-shares";
        let (status, public_share) = call(
            &router,
            "POST",
            shares,
            Some(&a),
            Some(json!({"request_id":"r","expires_in_days":7})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{public_share}");
        let original_public = format!(
            "/api/public/mock/{}/e",
            public_share["token"].as_str().unwrap()
        );
        assert_eq!(
            call(&router_again, "GET", &original_public, None, None)
                .await
                .0,
            StatusCode::CREATED
        );
        let (status, foreign_shares) = call(&router_again, "GET", shares, Some(&b), None).await;
        assert_eq!(status, StatusCode::OK);
        assert!(foreign_shares.as_array().unwrap().is_empty());
        let share_id = public_share["id"].as_str().unwrap();
        let (status, rotated) = call(
            &router_again,
            "POST",
            &format!("{shares}/{share_id}/rotate"),
            Some(&a),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{rotated}");
        let rotated_public = format!("/api/public/mock/{}/e", rotated["token"].as_str().unwrap());
        assert_eq!(
            call(&router, "GET", &original_public, None, None).await.0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            call(&router, "GET", &rotated_public, None, None).await.0,
            StatusCode::CREATED
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
            call(&router_again, "GET", &rotated_public, None, None)
                .await
                .0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            call(
                &router_again,
                "GET",
                &format!("{oauth1_path}/secret"),
                Some(&a),
                None
            )
            .await
            .0,
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

async fn verify_workspace_restore(
    router: &Router,
    other_instance: &Router,
    owner: &str,
    foreign: &str,
    suffix: &str,
    backend: &str,
) {
    let id = format!("restore-{suffix}");
    let path = format!("/api/workspaces/{id}");
    let restore = format!("{path}/versions/1/restore");
    let (status, created) = call(
        router,
        "POST",
        "/api/workspaces",
        Some(owner),
        Some(json!({"id":id,"name":"Before","data":data()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let (status, updated) = call(
        router,
        "PUT",
        &path,
        Some(owner),
        Some(json!({"name":"After","data":example_data(),"expected_revision":1})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    let (status, share) = call(
        router,
        "POST",
        &format!("{path}/mock-shares"),
        Some(owner),
        Some(json!({"request_id":"r","expires_in_days":7})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{share}");
    let public = format!("/api/public/mock/{}/e", share["token"].as_str().unwrap());
    assert_eq!(
        call(other_instance, "GET", &public, None, None).await.0,
        StatusCode::CREATED
    );
    let missing = format!("{path}/versions/999/restore");
    for (token, target) in [(foreign, restore.as_str()), (owner, missing.as_str())] {
        assert_eq!(
            call(
                router,
                "POST",
                target,
                Some(token),
                Some(json!({"expected_revision":2}))
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
    }
    let (status, restored) = call(
        router,
        "POST",
        &restore,
        Some(owner),
        Some(json!({"expected_revision":2})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{restored}");
    assert_eq!(restored["name"], "Before");
    assert_eq!(restored["revision"], 3);
    assert_eq!(
        call(other_instance, "GET", &public, None, None).await.0,
        StatusCode::NOT_FOUND
    );
    let (_, versions) = call(
        other_instance,
        "GET",
        &format!("{path}/versions"),
        Some(owner),
        None,
    )
    .await;
    assert_eq!(versions.as_array().unwrap().len(), 2);
    assert_eq!(versions[0]["name"], "After");
    assert_eq!(versions[0]["revision"], 2);
    assert_eq!(
        call(
            router,
            "POST",
            &restore,
            Some(owner),
            Some(json!({"expected_revision":2}))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let restore_again = json!({"expected_revision":3});
    let (first, second) = tokio::join!(
        call(
            router,
            "POST",
            &restore,
            Some(owner),
            Some(restore_again.clone())
        ),
        call(
            other_instance,
            "POST",
            &restore,
            Some(owner),
            Some(restore_again)
        )
    );
    assert!(
        (first.0 == StatusCode::OK && second.0 == StatusCode::CONFLICT)
            || (second.0 == StatusCode::OK && first.0 == StatusCode::CONFLICT),
        "{backend}: {first:?} {second:?}"
    );
    let (status, latest) = call(other_instance, "GET", &path, Some(owner), None).await;
    assert_eq!(status, StatusCode::OK, "{latest}");
    assert_eq!(latest["revision"], 4);
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

pub async fn verify_data_completion_history() {
    for (key, source) in [
        ("MOLEAPI_TEST_POSTGRES_URL", "postgresql"),
        ("MOLEAPI_TEST_MYSQL_URL", "mysql"),
    ] {
        let Ok(url) = std::env::var(key) else {
            continue;
        };
        let temp = tempfile::tempdir().unwrap();
        let router = moleapi_server::local_with_worker(
            &temp.path().join("data-history.db"),
            &std::path::PathBuf::from(env!("CARGO_BIN_EXE_moleapi-server")),
        )
        .await
        .unwrap();
        let mut data = example_data();
        let integer_type = if source == "mysql" {
            "SIGNED"
        } else {
            "BIGINT"
        };
        let sql = format!(
            "SELECT CAST(9007199254740993 AS {integer_type}) AS exact, CAST(1.23 AS DECIMAL(10,2)) AS amount"
        );
        let pre_script = format!(
            "pm.request.body.update({});",
            serde_json::to_string(&sql).unwrap()
        );
        let request = &mut data["collections"][0]["requests"][0];
        request["method"] = json!("GET");
        request["url"] = json!(url);
        request["body_kind"] = json!("none");
        request["body"] = json!("");
        request["assertions"] = json!([]);
        request["protocol"] = json!({"kind":"data","source":source,"tls":false,"sql":"SELECT 1","query_pre_script":pre_script});
        let (status, workspace) = call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"hosted-data","name":"Hosted Data","data":data})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{workspace}");
        let (status, session) = call(&router, "POST", "/api/sessions", None,
            Some(json!({"workspace_id":"hosted-data","request":workspace["data"]["collections"][0]["requests"][0]}))).await;
        assert_eq!(status, StatusCode::OK, "{session}");
        let id = session["id"].as_str().unwrap();
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let (_, summary) =
                    call(&router, "GET", &format!("/api/sessions/{id}"), None, None).await;
                if summary["state"] == "open" {
                    break;
                }
                assert_ne!(summary["state"], "error", "{summary}");
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        let (status, sent) = call(
            &router,
            "POST",
            &format!("/api/sessions/{id}/send"),
            None,
            Some(json!({"kind":"data_query","query_id":"typed-db","sql":"SELECT 1 AS placeholder","read_only":true})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{sent}");
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let (_, events) = call(
                    &router,
                    "GET",
                    &format!("/api/sessions/{id}/events"),
                    None,
                    None,
                )
                .await;
                if events["events"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|event| event["message"]["kind"] == "data_processed")
                {
                    assert!(
                        events["events"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|event| event["message"]["kind"] == "data_finished"),
                        "{source}: {events}"
                    );
                    assert!(
                        events["events"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|event| event["message"]["kind"] == "data_prepared"
                                && event["message"]["sql_changed"] == true),
                        "{source}: {events}"
                    );
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        let (_, entries) = call(
            &router,
            "GET",
            "/api/workspaces/hosted-data/history",
            None,
            None,
        )
        .await;
        let body: serde_json::Value =
            serde_json::from_str(entries[0]["response"]["body"].as_str().unwrap()).unwrap();
        assert_eq!(body["query_id"], "typed-db");
        assert_eq!(
            body["rows"][0][0]["value"], "9007199254740993",
            "{source}: {body}"
        );
        assert_eq!(body["rows"][0][1]["value"], "1.23", "{source}: {body}");
    }
}
