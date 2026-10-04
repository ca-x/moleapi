mod common;
use common::*;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
async fn request(
    router: &Router,
    token: Option<&str>,
    id: &str,
    url: &str,
    tls: bool,
    idle: u64,
) -> String {
    let mut data = example_data();
    let r = &mut data["collections"][0]["requests"][0];
    r["protocol"] = json!({"kind":"tcp","idle_timeout_ms":idle});
    r["url"] = json!(url);
    r["verify_tls"] = json!(tls);
    r["examples"] = json!([]);
    let (status, w) = call(
        router,
        "POST",
        "/api/workspaces",
        token,
        Some(json!({"id":id,"name":"TCP","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    let (status, s) = call(
        router,
        "POST",
        "/api/sessions",
        token,
        Some(json!({"workspace_id":id,"request":w["data"]["collections"][0]["requests"][0]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{s}");
    s["id"].as_str().unwrap().into()
}
async fn terminal(router: &Router, token: Option<&str>, id: &str) -> Value {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let (_, v) = call(router, "GET", &format!("/api/sessions/{id}"), token, None).await;
            if v["state"] == "error" || v["state"] == "closed" {
                return v;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}
#[tokio::test]
async fn actual_tls_rejects_untrusted_certificate_and_explicit_skip_connects() {
    let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let key = rustls::pki_types::PrivatePkcs8KeyDer::from(certificate.signing_key.serialize_der());
    let tls = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(vec![certificate.cert.der().clone()], key.into())
    .unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(tls));
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("tcps://localhost:{}", l.local_addr().unwrap().port());
    let fixture = tokio::spawn(async move {
        let mut accepted = 0;
        for _ in 0..2 {
            let (s, _) = l.accept().await.unwrap();
            if let Ok(mut s) = acceptor.accept(s).await {
                accepted += 1;
                s.write_all(b"secure data").await.unwrap();
                s.shutdown().await.unwrap();
            }
        }
        assert_eq!(accepted, 1);
    });
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("tls.db")).await.unwrap();
    let denied = request(&router, None, "verify", &url, true, 0).await;
    let summary = terminal(&router, None, &denied).await;
    assert_eq!(summary["state"], "error");
    assert!(
        summary["reason"].as_str().unwrap().contains("certificate"),
        "{summary}"
    );
    let allowed = request(&router, None, "skip", &url, false, 0).await;
    let summary = terminal(&router, None, &allowed).await;
    assert_eq!(summary["state"], "closed", "{summary}");
    assert!(summary["received_bytes"].as_u64().unwrap() > 11);
    assert!(summary["sent_bytes"].as_u64().unwrap() > 0);
    let (_, batch) = call(
        &router,
        "GET",
        &format!("/api/sessions/{allowed}/events"),
        None,
        None,
    )
    .await;
    assert!(
        batch["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["message"]["text"] == "secure data"),
        "{batch}"
    );
    fixture.await.unwrap();
}
#[tokio::test]
async fn hosted_policy_blocks_private_socket_and_other_owner_cannot_read_or_send() {
    let temp = tempfile::tempdir().unwrap();
    let router = hosted(config(
        format!(
            "sqlite://{}?mode=rwc",
            temp.path().join("hosted.db").display()
        ),
        true,
    ))
    .await
    .unwrap();
    let owner = register(&router, "owner").await;
    let other = register(&router, "other").await;
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("tcp://{}", l.local_addr().unwrap());
    let id = request(&router, Some(&owner), "w", &url, true, 0).await;
    let summary = terminal(&router, Some(&owner), &id).await;
    assert_eq!(summary["state"], "error");
    assert!(
        summary["reason"].as_str().unwrap().contains("blocked"),
        "{summary}"
    );
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), l.accept())
            .await
            .is_err()
    );
    for (method, path, body) in [
        ("GET", format!("/api/sessions/{id}/events"), None),
        (
            "POST",
            format!("/api/sessions/{id}/send"),
            Some(json!({"kind":"tcp_send","message":{"encoding":"text","payload_source":"no"}})),
        ),
    ] {
        let (status, _) = call(&router, method, &path, Some(&other), body).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}
#[tokio::test]
async fn deleting_workspace_closes_connected_socket() {
    use tokio::io::AsyncReadExt;
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("tcp://{}", l.local_addr().unwrap());
    let (ready, wait) = tokio::sync::oneshot::channel();
    let fixture = tokio::spawn(async move {
        let (mut s, _) = l.accept().await.unwrap();
        let _ = ready.send(());
        let mut b = [0; 1];
        assert_eq!(s.read(&mut b).await.unwrap(), 0);
    });
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("delete.db")).await.unwrap();
    let id = request(&router, None, "w", &url, true, 0).await;
    wait.await.unwrap();
    let (status, _) = call(
        &router,
        "DELETE",
        "/api/workspaces/w",
        None,
        Some(json!({"expected_revision":1})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    tokio::time::timeout(std::time::Duration::from_secs(2), fixture)
        .await
        .unwrap()
        .unwrap();
    let (status, _) = call(&router, "GET", &format!("/api/sessions/{id}"), None, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[test]
fn slow_dns_setup_does_not_consume_new_connections_idle_budget() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        use tokio::io::AsyncReadExt;
        let temp = tempfile::tempdir().unwrap();
        let router = local(&temp.path().join("idle-start.db")).await.unwrap();
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("tcp://localhost:{}", l.local_addr().unwrap().port());
        let (accepted, wait) = tokio::sync::oneshot::channel();
        let fixture = tokio::spawn(async move {
            let (mut s, _) = l.accept().await.unwrap();
            let _ = accepted.send(());
            let mut b = [0; 1];
            s.read_exact(&mut b).await.unwrap();
            assert_eq!(&b, b"x");
            s.write_all(b"ok").await.unwrap();
        });
        let (started, ready) = tokio::sync::oneshot::channel();
        let blocker = tokio::task::spawn_blocking(move || {
            let _ = started.send(());
            std::thread::sleep(std::time::Duration::from_millis(400));
        });
        ready.await.unwrap();
        let id = request(&router, None, "w", &url, true, 200).await;
        wait.await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        let (_, summary) = call(&router, "GET", &format!("/api/sessions/{id}"), None, None).await;
        assert_eq!(summary["state"], "open", "{summary}");
        let (status, value) = call(
            &router,
            "POST",
            &format!("/api/sessions/{id}/send"),
            None,
            Some(json!({"kind":"tcp_send","message":{"payload_source":"x"}})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{value}");
        fixture.await.unwrap();
        blocker.await.unwrap();
        assert_eq!(terminal(&router, None, &id).await["state"], "closed");
    });
}

#[tokio::test]
async fn scoped_payload_variable_taints_echo_without_explicit_secret_flag() {
    use tokio::io::AsyncReadExt;
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("tcp://{}", l.local_addr().unwrap());
    let fixture = tokio::spawn(async move {
        let (mut s, _) = l.accept().await.unwrap();
        let mut bytes = [0; 19];
        s.read_exact(&mut bytes).await.unwrap();
        assert_eq!(&bytes, b"private-by-variable");
        s.write_all(&bytes).await.unwrap();
    });
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("private-variable.db"))
        .await
        .unwrap();
    let mut data = example_data();
    data["global_variables"] = json!([{"id":"v","key":"private","value":"private-by-variable","secret":true,"enabled":true}]);
    let r = &mut data["collections"][0]["requests"][0];
    r["protocol"] = json!({"kind":"tcp"});
    r["url"] = json!(url);
    r["examples"] = json!([]);
    let (status, w) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"Scoped TCP","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    let (status, s) = call(
        &router,
        "POST",
        "/api/sessions",
        None,
        Some(json!({"workspace_id":"w","request":w["data"]["collections"][0]["requests"][0]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let id = s["id"].as_str().unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let (_, s) = call(&router, "GET", &format!("/api/sessions/{id}"), None, None).await;
            if s["state"] == "open" {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let (status, value) = call(
        &router,
        "POST",
        &format!("/api/sessions/{id}/send"),
        None,
        Some(json!({"kind":"tcp_send","message":{"payload_source":"{{private}}","secret":false}})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    fixture.await.unwrap();
    assert_eq!(terminal(&router, None, id).await["state"], "closed");
    let (_, batch) = call(
        &router,
        "GET",
        &format!("/api/sessions/{id}/events"),
        None,
        None,
    )
    .await;
    for event in batch["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["message"]["kind"] == "tcp_data")
    {
        assert_eq!(event["message"]["redacted"], true);
        assert_eq!(event["message"]["base64"], "");
    }
    assert!(!batch.to_string().contains("private-by-variable"));
}
