mod common;
use common::*;

fn docs_data() -> Value {
    json!({
        "schema_version":1,
        "collections":[{"id":"folder","name":"Guides","description":"","requests":[]}],
        "environments":[],
        "active_environment_id":null,
        "documents":[
            {"id":"guide","name":"Public guide","collection_id":"folder","source":"# Hello\nSafe content","secret":false},
            {"id":"private","name":"Private notes","source":"never-publish-sentinel","secret":true}
        ]
    })
}

fn publish(revision: i64, ids: &[&str], key: &str) -> Value {
    json!({"title":"Reference","expected_revision":revision,"document_ids":ids,"idempotency_key":key})
}

#[tokio::test]
async fn publication_is_explicit_immutable_owned_and_revocable() {
    let dir = tempfile::tempdir().unwrap();
    let router = hosted(config(
        moleapi_server::sqlite_database_url(&dir.path().join("publish.db")).unwrap(),
        true,
    ))
    .await
    .unwrap();
    let owner = register(&router, "publishowner").await;
    let foreign = register(&router, "publishother").await;
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Workspace private name","data":docs_data()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let revision = saved["revision"].as_i64().unwrap();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces/w/doc-publications",
            Some(&owner),
            Some(publish(revision, &["private"], "private-attempt")),
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &router,
            "GET",
            "/api/workspaces/w/doc-publications",
            Some(&owner),
            None,
        )
        .await
        .1,
        json!([])
    );
    let (status, publication) = call(
        &router,
        "POST",
        "/api/workspaces/w/doc-publications",
        Some(&owner),
        Some(publish(revision, &["guide"], "publish-once")),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{publication}");
    let id = publication["id"].as_str().unwrap();
    assert_eq!(id.len(), 32);
    assert_eq!(publication["public_url"], format!("/docs/{id}"));
    let public_path = format!("/api/public/docs/{id}");
    let html_path = format!("/docs/{id}");
    assert_eq!(
        call(&router, "GET", &html_path, None, None).await.0,
        StatusCode::OK
    );
    let (status, snapshot) = call(&router, "GET", &public_path, None, None).await;
    assert_eq!(status, StatusCode::OK, "{snapshot}");
    assert_eq!(snapshot["title"], "Reference");
    assert_eq!(snapshot["documents"][0]["folder"], "Guides");
    assert_eq!(snapshot["documents"][0]["source"], "# Hello\nSafe content");
    assert!(!snapshot.to_string().contains("never-publish-sentinel"));
    assert!(!snapshot.to_string().contains("Workspace private name"));
    assert!(!snapshot.to_string().contains("workspace_id"));
    assert_eq!(
        call(
            &router,
            "GET",
            "/api/workspaces/w/doc-publications",
            Some(&foreign),
            None,
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (_, retried) = call(
        &router,
        "POST",
        "/api/workspaces/w/doc-publications",
        Some(&owner),
        Some(publish(revision, &["guide"], "publish-once")),
    )
    .await;
    assert_eq!(retried["id"], publication["id"]);
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces/w/doc-publications",
            Some(&owner),
            Some(publish(revision, &["guide", "private"], "publish-once")),
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let mut changed = docs_data();
    changed["documents"][0]["source"] = json!("# Changed later");
    let (status, updated) = call(
        &router,
        "PUT",
        "/api/workspaces/w",
        Some(&owner),
        Some(json!({"name":"Workspace private name","expected_revision":revision,"data":changed})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(
        call(&router, "GET", &public_path, None, None).await.1,
        snapshot
    );
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces/w/doc-publications",
            Some(&owner),
            Some(publish(revision, &["guide"], "next-key")),
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let revoke_path = format!("/api/workspaces/w/doc-publications/{id}");
    assert_eq!(
        call(&router, "DELETE", &revoke_path, Some(&foreign), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&router, "DELETE", &revoke_path, Some(&owner), None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&router, "GET", &public_path, None, None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&router, "GET", &html_path, None, None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces/w/doc-publications",
            Some(&owner),
            Some(publish(revision, &["guide"], "publish-once")),
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let (_, history) = call(
        &router,
        "GET",
        "/api/workspaces/w/doc-publications",
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(history.as_array().unwrap().len(), 1);
    assert_eq!(history[0]["revoked"], true);
}

#[tokio::test]
async fn publication_fails_closed_on_workspace_deletion_and_invalid_ids() {
    let dir = tempfile::tempdir().unwrap();
    let router = hosted(config(
        moleapi_server::sqlite_database_url(&dir.path().join("delete.db")).unwrap(),
        true,
    ))
    .await
    .unwrap();
    let owner = register(&router, "deletepublishowner").await;
    let (_, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"W","data":docs_data()})),
    )
    .await;
    let revision = saved["revision"].as_i64().unwrap();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces/w/doc-publications",
            Some(&owner),
            Some(publish(revision, &["guide", "guide"], "duplicate")),
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (_, published) = call(
        &router,
        "POST",
        "/api/workspaces/w/doc-publications",
        Some(&owner),
        Some(publish(revision, &["guide"], "once")),
    )
    .await;
    let public_path = format!("/api/public/docs/{}", published["id"].as_str().unwrap());
    assert_eq!(
        call(&router, "GET", &public_path, None, None).await.0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &router,
            "DELETE",
            "/api/workspaces/w",
            Some(&owner),
            Some(json!({"expected_revision":revision})),
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&router, "GET", &public_path, None, None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&router, "GET", "/api/public/docs/not-an-id", None, None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    for path in ["/docs/not-an-id", "/docs/unknown/extra", "/docs"] {
        assert_eq!(
            call(&router, "GET", path, None, None).await.0,
            StatusCode::NOT_FOUND
        );
    }
}
