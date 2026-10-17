mod common;
use common::*;

#[tokio::test]
async fn mock_calls_record_owned_metadata_without_request_secrets_and_can_be_cleared() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("mock-logs.db")).await.unwrap();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = json!("https://example.test/items");
    let (status, _) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"W","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        call(
            &router,
            "GET",
            "/api/mock/w/r/e?token=private-query",
            None,
            None
        )
        .await
        .0,
        StatusCode::CREATED
    );
    let routed = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/mocks/w/items?token=private-query")
                .header("authorization", "Bearer private-owner")
                .header("cookie", "private-cookie")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(routed.status(), StatusCode::CREATED);
    let (status, logs) = call(&router, "GET", "/api/workspaces/w/mock-calls", None, None).await;
    assert_eq!(status, StatusCode::OK, "{logs}");
    assert_eq!(logs.as_array().unwrap().len(), 2, "{logs}");
    assert_eq!(logs[0]["response_status"], 201);
    assert_eq!(logs[0]["request_id"], "r");
    assert_eq!(logs[0]["example_id"], "e");
    let text = logs.to_string();
    for secret in [
        "private-query",
        "private-owner",
        "private-cookie",
        "example body",
    ] {
        assert!(
            !text.contains(secret),
            "stored Mock call leaked {secret}: {text}"
        );
    }
    assert_eq!(
        call(
            &router,
            "DELETE",
            "/api/workspaces/w/mock-calls",
            None,
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, logs) = call(&router, "GET", "/api/workspaces/w/mock-calls", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(logs.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn hosted_mock_calls_are_owner_scoped_and_deleted_with_the_workspace() {
    use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("owners.db");
    let url = moleapi_server::sqlite_database_url(&path).unwrap();
    let router = hosted(config(url.clone(), true)).await.unwrap();
    let alice = register(&router, "mocklogalice").await;
    let bob = register(&router, "mocklogbobby").await;
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&alice),
        Some(json!({"id":"w","name":"W","data":example_data()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(
        call(&router, "GET", "/api/mock/w/r/e", Some(&alice), None)
            .await
            .0,
        StatusCode::CREATED
    );
    assert_eq!(
        call(
            &router,
            "GET",
            "/api/workspaces/w/mock-calls",
            Some(&bob),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &router,
            "GET",
            "/api/workspaces/w/mock-calls",
            Some(&alice),
            None
        )
        .await
        .1
        .as_array()
        .unwrap()
        .len(),
        1
    );
    assert_eq!(
        call(
            &router,
            "DELETE",
            "/api/workspaces/w",
            Some(&alice),
            Some(json!({"expected_revision":saved["revision"]}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let db = Database::connect(url).await.unwrap();
    let count=db.query_one(Statement::from_string(DatabaseBackend::Sqlite,"SELECT COUNT(*) AS count FROM documents WHERE kind IN ('mock-call','mock-call-sequence') AND ref_id = 'w'")) .await.unwrap().unwrap();
    let count: i64 = count.try_get("", "count").unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn concurrent_mock_calls_from_two_routers_keep_the_latest_two_hundred() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("concurrent.db");
    let first = local(&path).await.unwrap();
    let second = local(&path).await.unwrap();
    assert_eq!(
        call(
            &first,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"W","data":example_data()}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let mut tasks = tokio::task::JoinSet::new();
    for index in 0..8 {
        let router = if index % 2 == 0 {
            first.clone()
        } else {
            second.clone()
        };
        tasks.spawn(async move {
            for _ in 0..30 {
                let (status, body) = call(&router, "GET", "/api/mock/w/r/e", None, None).await;
                assert_eq!(status, StatusCode::CREATED, "{body}");
            }
        });
    }
    while let Some(result) = tasks.join_next().await {
        result.unwrap();
    }
    let (status, logs) = call(&first, "GET", "/api/workspaces/w/mock-calls", None, None).await;
    assert_eq!(status, StatusCode::OK, "{logs}");
    let values = logs.as_array().unwrap();
    assert_eq!(values.len(), 200);
    assert_eq!(values.first().unwrap()["sequence"], 240);
    assert_eq!(values.last().unwrap()["sequence"], 41);
}

#[tokio::test]
async fn failed_and_unmatched_mock_calls_store_only_generic_outcomes() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("failure.db")).await.unwrap();
    let mut data = example_data();
    data["script_mode"] = json!("v2");
    data["collections"][0]["requests"][0]["url"] = json!("https://example.test/items");
    data["collections"][0]["requests"][0]["examples"][0]["mock_script"] =
        json!("throw new Error('private-diagnostic')");
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"W","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&router, "GET", "/api/mock/w/r/e", None, None).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &router,
            "GET",
            "/api/mocks/w/missing?token=private-query",
            None,
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let too_many = format!(
        "/api/mocks/w/items?{}",
        (0..101)
            .map(|n| format!("p{n}=x"))
            .collect::<Vec<_>>()
            .join("&")
    );
    assert_eq!(
        call(&router, "GET", &too_many, None, None).await.0,
        StatusCode::BAD_REQUEST
    );
    let (status, logs) = call(&router, "GET", "/api/workspaces/w/mock-calls", None, None).await;
    assert_eq!(status, StatusCode::OK, "{logs}");
    let values = logs.as_array().unwrap();
    assert_eq!(values.len(), 3, "{logs}");
    assert_eq!(values[0]["outcome"], "rejected");
    assert_eq!(values[1]["outcome"], "unmatched");
    assert_eq!(values[2]["outcome"], "failed");
    let text = logs.to_string();
    assert!(!text.contains("private-diagnostic"));
    assert!(!text.contains("private-query"));
}

#[tokio::test]
async fn mock_response_is_withheld_when_its_required_log_cannot_be_written() {
    use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("write-failure.db");
    let router = local(&path).await.unwrap();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"W","data":example_data()}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let db = Database::connect(moleapi_server::sqlite_database_url(&path).unwrap())
        .await
        .unwrap();
    db.execute(Statement::from_string(DatabaseBackend::Sqlite,"CREATE TRIGGER fail_mock_call BEFORE INSERT ON documents WHEN NEW.kind = 'mock-call' BEGIN SELECT RAISE(ABORT, 'blocked'); END")).await.unwrap();
    let (status, body) = call(&router, "GET", "/api/mock/w/r/e", None, None).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    assert!(!body.to_string().contains("example body"));
}
