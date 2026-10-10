mod common;
use axum::{
    extract::ws::{Message, WebSocketUpgrade},
    routing::get,
};
use common::*;
use std::{sync::Arc, time::Duration};

async fn await_state(router: &Router, owner: &str, id: &str, expected: &str) {
    tokio::time::timeout(Duration::from_secs(6), async {
        loop {
            let (status, summary) = call(
                router,
                "GET",
                &format!("/api/sessions/{id}"),
                Some(owner),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{summary}");
            if summary["state"] == expected {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
}
async fn socket(router: &Router, owner: &str, url: &str) -> String {
    let workspace_id = uuid::Uuid::new_v4().to_string();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = json!(url);
    data["collections"][0]["requests"][0]["protocol"] = json!({"kind":"websocket"});
    let (status, workspace) = call(
        router,
        "POST",
        "/api/workspaces",
        Some(owner),
        Some(json!({"id":workspace_id,"name":"Socket", "data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{workspace}");
    let (status, session) = call(router, "POST", "/api/sessions", Some(owner), Some(json!({"workspace_id":workspace_id, "request":workspace["data"]["collections"][0]["requests"][0]}))).await;
    assert_eq!(status, StatusCode::OK, "{session}");
    let id = session["id"].as_str().unwrap().to_owned();
    await_state(router, owner, &id, "open").await;
    id
}
#[tokio::test]
async fn revoke_and_logout_propagate_to_other_instances_without_replaying_history() {
    let started = Arc::new(tokio::sync::Notify::new());
    let signal = started.clone();
    let (url, fixture) = serve(
        Router::new()
            .route(
                "/slow",
                get(move || {
                    let signal = signal.clone();
                    async move {
                        signal.notify_one();
                        tokio::time::sleep(Duration::from_secs(20)).await;
                        "done"
                    }
                }),
            )
            .route(
                "/socket",
                get(|upgrade: WebSocketUpgrade| async {
                    upgrade.on_upgrade(|mut socket| async move {
                        while let Some(Ok(message)) = socket.recv().await {
                            if matches!(message, Message::Close(_)) {
                                break;
                            }
                            if socket.send(message).await.is_err() {
                                break;
                            }
                        }
                    })
                }),
            ),
    )
    .await;
    let temporary = tempfile::tempdir().unwrap();
    let database_url = format!(
        "sqlite://{}?mode=rwc",
        temporary.path().join("instances.db").display()
    );
    let config = moleapi_server::Config {
        database_url,
        setup_token: "setup-secret".into(),
        allow_registration: true,
        allow_private_network: true,
    };
    let a = hosted(config.clone()).await.unwrap();
    let b = hosted(config.clone()).await.unwrap();
    let owner = register(&a, "distributedowner").await;
    let stranger = register(&a, "unaffectedowner").await;
    let socket_url = format!("{}/socket", url.replacen("http", "ws", 1));
    let owned_socket = socket(&b, &owner, &socket_url).await;
    let other_socket = socket(&b, &stranger, &socket_url).await;
    let (_, created) = call(
        &a,
        "POST",
        "/api/auth/tokens",
        Some(&owner),
        Some(json!({"name":"Remote run"})),
    )
    .await;
    let token = created["token"].as_str().unwrap().to_owned();
    let id = created["id"].as_str().unwrap();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = json!(format!("{url}/slow"));
    data["collections"][0]["requests"][0]["timeout_ms"] = json!(30000);
    let (status, _) = call(
        &b,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"run","name":"Live run", "data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let executing = b.clone();
    let run_token = token.clone();
    let run = tokio::spawn(async move {
        call(
            &executing,
            "POST",
            "/api/workspaces/run/run",
            Some(&run_token),
            Some(json!({"collection_id":"c","job_id":"distributed-run"})),
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(5), started.notified())
        .await
        .unwrap();
    assert_eq!(
        call(
            &a,
            "DELETE",
            &format!("/api/auth/tokens/{id}"),
            Some(&owner),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, result) = tokio::time::timeout(Duration::from_secs(6), run)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status, StatusCode::OK, "{result}");
    assert!(
        result["cancelled"] == true || result["stopped_reason"] == "owner_changed",
        "{result}"
    );
    await_state(&b, &owner, &owned_socket, "closed").await;
    await_state(&b, &stranger, &other_socket, "open").await;
    assert_eq!(
        call(&b, "GET", "/api/workspaces", Some(&token), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );

    // A new instance baselines the earlier revision instead of replaying its cancellation.
    let c = hosted(config).await.unwrap();
    let fresh_socket = socket(&c, &owner, &socket_url).await;
    tokio::time::sleep(Duration::from_millis(1200)).await;
    await_state(&c, &owner, &fresh_socket, "open").await;
    // Session logout publishes another event; a different valid login can inspect the closed socket.
    let (_, login) = call(
        &a,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({"username":"distributedowner","password":"goodpassword123"})),
    )
    .await;
    let second_session = login["token"].as_str().unwrap();
    assert_eq!(
        call(&a, "POST", "/api/auth/logout", Some(&owner), None)
            .await
            .0,
        StatusCode::OK
    );
    await_state(&c, second_session, &fresh_socket, "closed").await;
    await_state(&b, &stranger, &other_socket, "open").await;
    fixture.abort();
}

#[tokio::test]
async fn concurrent_revokes_publish_each_revision_and_missing_tokens_publish_nothing() {
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
    let temporary = tempfile::tempdir().unwrap();
    let url = format!(
        "sqlite://{}?mode=rwc",
        temporary.path().join("concurrent.db").display()
    );
    let config = config(url.clone(), true);
    let a = hosted(config.clone()).await.unwrap();
    let b = hosted(config).await.unwrap();
    let owner = register(&a, "revisionowner").await;
    let (_, first) = call(
        &a,
        "POST",
        "/api/auth/tokens",
        Some(&owner),
        Some(json!({"name":"First"})),
    )
    .await;
    let (_, second) = call(
        &a,
        "POST",
        "/api/auth/tokens",
        Some(&owner),
        Some(json!({"name":"Second"})),
    )
    .await;
    let first_path = format!("/api/auth/tokens/{}", first["id"].as_str().unwrap());
    let second_path = format!("/api/auth/tokens/{}", second["id"].as_str().unwrap());
    let (first_result, second_result) = tokio::join!(
        call(&a, "DELETE", &first_path, Some(&owner), None),
        call(&b, "DELETE", &second_path, Some(&owner), None),
    );
    assert_eq!(first_result.0, StatusCode::OK, "{first_result:?}");
    assert_eq!(second_result.0, StatusCode::OK, "{second_result:?}");
    assert_eq!(
        call(&a, "DELETE", &first_path, Some(&owner), None).await.0,
        StatusCode::NOT_FOUND
    );
    let db = Database::connect(&url).await.unwrap();
    let rows = db
        .query_all(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT revision FROM settings WHERE id LIKE 'credential-revocation:%'".to_owned(),
        ))
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].try_get::<i64>("", "revision").unwrap(), 2);
    assert_eq!(
        call(
            &b,
            "GET",
            "/api/workspaces",
            Some(first["token"].as_str().unwrap()),
            None
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &a,
            "GET",
            "/api/workspaces",
            Some(second["token"].as_str().unwrap()),
            None
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
}
