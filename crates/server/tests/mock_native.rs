mod common;
use common::*;

#[tokio::test]
async fn native_listener_is_explicit_and_exposes_only_capability_routes() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("native-mock.db")).await.unwrap();
    assert_eq!(
        call(&router, "GET", "/api/mock-public/listener", None, None)
            .await
            .1["active"],
        false
    );
    let (_, share) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"W","data":example_data()})),
    )
    .await;
    assert_eq!(share["id"], "w");
    let (status, grant) = call(
        &router,
        "POST",
        "/api/workspaces/w/mock-shares",
        None,
        Some(json!({"request_id":"r","expires_in_days":1})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{grant}");
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/mock-public/listener/start",
            None,
            Some(json!({"host":"0.0.0.0","port":0}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/mock-public/listener/start",
            None,
            Some(json!({"host":"localhost","port":0}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let occupied = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = occupied.local_addr().unwrap().port();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/mock-public/listener/start",
            None,
            Some(json!({"host":"127.0.0.1","port":port}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(&router, "GET", "/api/mock-public/listener", None, None)
            .await
            .1["active"],
        false
    );
    drop(occupied);
    let (status, lan) = call(
        &router,
        "POST",
        "/api/mock-public/listener/start",
        None,
        Some(json!({"host":"0.0.0.0","port":0,"allow_lan":true})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{lan}");
    assert!(lan["bind"].as_str().unwrap().starts_with("0.0.0.0:"));
    assert!(
        lan["origin"]
            .as_str()
            .unwrap()
            .starts_with("http://127.0.0.1:")
    );
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/mock-public/listener/stop",
            None,
            Some(json!({}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, started) = call(
        &router,
        "POST",
        "/api/mock-public/listener/start",
        None,
        Some(json!({"host":"127.0.0.1","port":0})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{started}");
    assert_eq!(started["active"], true);
    let origin = started["origin"].as_str().unwrap();
    assert!(origin.starts_with("http://127.0.0.1:"));
    assert_eq!(
        call(&router, "GET", "/api/mock-public/listener", None, None)
            .await
            .1["origin"],
        origin
    );
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/mock-public/listener/start",
            None,
            Some(json!({}))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let url = format!(
        "{origin}/api/public/mock/{}/e",
        grant["token"].as_str().unwrap()
    );
    let public = reqwest::get(&url).await.unwrap();
    assert_eq!(public.status(), StatusCode::CREATED);
    assert_eq!(public.text().await.unwrap(), "example body");
    assert_eq!(
        reqwest::get(format!("{origin}/api/workspaces/w"))
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        reqwest::get(format!("{origin}/api/mock/w/r/e"))
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &router,
            "DELETE",
            &format!(
                "/api/workspaces/w/mock-shares/{}",
                grant["id"].as_str().unwrap()
            ),
            None,
            None,
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        reqwest::get(&url).await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
    let (_, replacement) = call(
        &router,
        "POST",
        "/api/workspaces/w/mock-shares",
        None,
        Some(json!({"request_id":"r","expires_in_days":1})),
    )
    .await;
    let replacement_url = format!(
        "{origin}/api/public/mock/{}/e",
        replacement["token"].as_str().unwrap()
    );
    let stopped = call(
        &router,
        "POST",
        "/api/mock-public/listener/stop",
        None,
        Some(json!({})),
    )
    .await;
    assert_eq!(stopped.0, StatusCode::OK);
    assert_eq!(stopped.1["active"], false);
    assert!(reqwest::get(&replacement_url).await.is_err());
    let (_, restarted) = call(
        &router,
        "POST",
        "/api/mock-public/listener/start",
        None,
        Some(json!({"host":"127.0.0.1","port":0})),
    )
    .await;
    let restarted_origin = restarted["origin"].as_str().unwrap().to_string();
    assert_eq!(
        reqwest::get(format!(
            "{restarted_origin}/api/public/mock/{}/e",
            replacement["token"].as_str().unwrap()
        ))
        .await
        .unwrap()
        .status(),
        StatusCode::CREATED
    );
    drop(router);
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if tokio::net::TcpStream::connect(restarted_origin.trim_start_matches("http://"))
                .await
                .is_err()
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("native listener survived its IPC router");
}

#[tokio::test]
async fn stopping_native_listener_cancels_a_delayed_public_call() {
    let dir = tempfile::tempdir().unwrap();
    let slots = std::sync::Arc::new(tokio::sync::Semaphore::new(32));
    let router = moleapi_server::local_with_mock_delay_slots_for_test(
        &dir.path().join("cancel-mock.db"),
        std::path::Path::new(env!("CARGO_BIN_EXE_moleapi-server")),
        slots.clone(),
    )
    .await
    .unwrap();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["examples"][0]["delay_ms"] = json!(5_000);
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
    let (_, share) = call(
        &router,
        "POST",
        "/api/workspaces/w/mock-shares",
        None,
        Some(json!({"request_id":"r","expires_in_days":1})),
    )
    .await;
    let (_, started) = call(
        &router,
        "POST",
        "/api/mock-public/listener/start",
        None,
        Some(json!({})),
    )
    .await;
    let url = format!(
        "{}/api/public/mock/{}/e",
        started["origin"].as_str().unwrap(),
        share["token"].as_str().unwrap()
    );
    let running = tokio::spawn(async move { reqwest::get(url).await });
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while slots.available_permits() != 31 {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("native public call did not enter its delay");
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/mock-public/listener/stop",
            None,
            Some(json!({}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let result = tokio::time::timeout(std::time::Duration::from_secs(3), running)
        .await
        .expect("stopped listener did not cancel its call")
        .unwrap();
    if let Ok(response) = result {
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(response.headers()["referrer-policy"], "no-referrer");
        assert_eq!(response.headers()["access-control-allow-origin"], "*");
    }
    assert_eq!(slots.available_permits(), 32);
}

#[tokio::test]
async fn hosted_mode_cannot_control_a_native_public_listener() {
    let dir = tempfile::tempdir().unwrap();
    let router = hosted(moleapi_server::Config {
        database_url: moleapi_server::sqlite_database_url(&dir.path().join("hosted.db")).unwrap(),
        setup_token: "setup-secret".into(),
        allow_registration: true,
        allow_private_network: true,
    })
    .await
    .unwrap();
    let owner = register(&router, "hostedlistener").await;
    assert_eq!(
        call(
            &router,
            "GET",
            "/api/mock-public/listener",
            Some(&owner),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/mock-public/listener/start",
            Some(&owner),
            Some(json!({"host":"127.0.0.1","port":0}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}
