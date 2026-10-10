mod common;
use common::*;
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
#[tokio::test]
async fn personal_tokens_are_owned_one_time_private_expiring_and_revocable() {
    let temporary = tempfile::tempdir().unwrap();
    let url = format!(
        "sqlite://{}?mode=rwc",
        temporary.path().join("tokens.db").display()
    );
    let router = hosted(config(url.clone(), true)).await.unwrap();
    let owner = register(&router, "owner").await;
    let stranger = register(&router, "stranger").await;
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            Some(&owner),
            Some(json!({"id":"w","name":"Private workspace","data":example_data()}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, created) = call(
        &router,
        "POST",
        "/api/auth/tokens",
        Some(&owner),
        Some(json!({"name":"CI","expires_in_days":7})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let raw = created["token"].as_str().unwrap();
    let id = created["id"].as_str().unwrap();
    assert!(raw.starts_with("moleapi_pat_"));
    assert_eq!(raw.len(), 76);
    let (_, listed) = call(&router, "GET", "/api/auth/tokens", Some(&owner), None).await;
    assert_eq!(listed[0]["id"], id);
    assert!(listed[0].get("token").is_none());
    assert!(listed[0].get("digest").is_none());
    assert!(!listed.to_string().contains(raw));
    let (_, other) = call(&router, "GET", "/api/auth/tokens", Some(&stranger), None).await;
    assert_eq!(other, json!([]));
    assert_eq!(
        call(
            &router,
            "DELETE",
            &format!("/api/auth/tokens/{id}"),
            Some(&stranger),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&router, "GET", "/api/workspaces/w", Some(raw), None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&router, "GET", "/api/auth/tokens", Some(raw), None)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/auth/tokens",
            Some(raw),
            Some(json!({"name":"Escalation"}))
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &router,
            "DELETE",
            &format!("/api/auth/tokens/{id}"),
            Some(raw),
            None
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let db = Database::connect(&url).await.unwrap();
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT digest FROM access_tokens WHERE id = ?",
            [id.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    let digest: String = row.try_get("", "digest").unwrap();
    assert_ne!(digest, raw);
    assert_eq!(digest.len(), 64);
    let (_, export) = call(
        &router,
        "POST",
        "/api/workspaces/w/export",
        Some(raw),
        Some(json!({"format":"moleapi","include_secrets":true})),
    )
    .await;
    assert!(!export.to_string().contains(raw));
    assert!(!export.to_string().contains(&digest));
    db.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "UPDATE access_tokens SET expires_at = 0 WHERE id = ?",
        [id.into()],
    ))
    .await
    .unwrap();
    assert_eq!(
        call(&router, "GET", "/api/workspaces", Some(raw), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let (_, listed) = call(&router, "GET", "/api/auth/tokens", Some(&owner), None).await;
    assert_eq!(listed[0]["expired"], true);
    assert_eq!(
        call(
            &router,
            "DELETE",
            &format!("/api/auth/tokens/{id}"),
            Some(&owner),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, next) = call(
        &router,
        "POST",
        "/api/auth/tokens",
        Some(&owner),
        Some(json!({"name":"Next"})),
    )
    .await;
    let raw = next["token"].as_str().unwrap();
    assert_eq!(
        call(&router, "POST", "/api/auth/logout", Some(raw), None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&router, "GET", "/api/workspaces", Some(raw), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&router, "GET", "/api/workspaces", Some(&owner), None)
            .await
            .0,
        StatusCode::OK
    );
    let local = local(&temporary.path().join("offline.db")).await.unwrap();
    assert_eq!(
        call(
            &local,
            "POST",
            "/api/auth/tokens",
            None,
            Some(json!({"name":"No account"}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}
#[tokio::test]
async fn quotas_are_transactional_and_invalid_inputs_mint_nothing() {
    let temporary = tempfile::tempdir().unwrap();
    let url = format!(
        "sqlite://{}?mode=rwc",
        temporary.path().join("quota.db").display()
    );
    let router = hosted(config(url.clone(), true)).await.unwrap();
    let owner = register(&router, "quotaowner").await;
    for payload in [
        json!({"name":" "}),
        json!({"name":"line\nbreak"}),
        json!({"name":"Too long".repeat(12)}),
        json!({"name":"Zero","expires_in_days":0}),
        json!({"name":"Too long expiry","expires_in_days":366}),
    ] {
        assert_eq!(
            call(
                &router,
                "POST",
                "/api/auth/tokens",
                Some(&owner),
                Some(payload)
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
    let db = Database::connect(&url).await.unwrap();
    let row = db
        .query_one(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT id FROM accounts".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    let id: String = row.try_get("", "id").unwrap();
    for index in 0..99 {
        db.execute(Statement::from_sql_and_values(DbBackend::Sqlite,"INSERT INTO access_tokens (id, owner, name, digest, created_at, expires_at) VALUES (?, ?, ?, ?, ?, ?)",[format!("fixture-{index}").into(),id.clone().into(),"Seed".into(),format!("digest-{index}").into(),0_i64.into(),i64::MAX.into()])).await.unwrap();
    }
    let peer = hosted(config(url.clone(), true)).await.unwrap();
    let first = call(
        &router,
        "POST",
        "/api/auth/tokens",
        Some(&owner),
        Some(json!({"name":"One"})),
    );
    let second = call(
        &peer,
        "POST",
        "/api/auth/tokens",
        Some(&owner),
        Some(json!({"name":"Two"})),
    );
    let (first, second) = tokio::join!(first, second);
    assert!(matches!(
        (first.0, second.0),
        (StatusCode::OK, StatusCode::BAD_REQUEST) | (StatusCode::BAD_REQUEST, StatusCode::OK)
    ));
    let (_, list) = call(&router, "GET", "/api/auth/tokens", Some(&owner), None).await;
    assert_eq!(list.as_array().unwrap().len(), 100);
}

#[tokio::test]
async fn revocation_stops_an_already_running_owned_job() {
    let started = std::sync::Arc::new(tokio::sync::Notify::new());
    let signal = started.clone();
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::get(move || {
            let signal = signal.clone();
            async move {
                signal.notify_one();
                tokio::time::sleep(std::time::Duration::from_secs(20)).await;
                "done"
            }
        }),
    ))
    .await;
    let temporary = tempfile::tempdir().unwrap();
    let router = hosted(moleapi_server::Config {
        database_url: format!(
            "sqlite://{}?mode=rwc",
            temporary.path().join("running.db").display()
        ),
        setup_token: "setup-secret".into(),
        allow_registration: true,
        allow_private_network: true,
    })
    .await
    .unwrap();
    let owner = register(&router, "runnerowner").await;
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = format!("{url}/").into();
    data["collections"][0]["requests"][0]["timeout_ms"] = 30000.into();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            Some(&owner),
            Some(json!({"id":"w","name":"Running","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, created) = call(
        &router,
        "POST",
        "/api/auth/tokens",
        Some(&owner),
        Some(json!({"name":"Runner"})),
    )
    .await;
    let token = created["token"].as_str().unwrap().to_owned();
    let id = created["id"].as_str().unwrap();
    let executing = router.clone();
    let run = tokio::spawn(async move {
        call(
            &executing,
            "POST",
            "/api/workspaces/w/run",
            Some(&token),
            Some(json!({"collection_id":"c","job_id":"pat-run"})),
        )
        .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), started.notified())
        .await
        .unwrap();
    assert_eq!(
        call(
            &router,
            "DELETE",
            &format!("/api/auth/tokens/{id}"),
            Some(&owner),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, result) = tokio::time::timeout(std::time::Duration::from_secs(5), run)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status, StatusCode::OK, "{result}");
    assert!(result["cancelled"] == true || result["stopped_reason"] == "owner_changed");
    fixture.abort();
}
