mod common {
    pub mod ntlm_fixture;
}
use common::ntlm_fixture::*;
use moleapi_core::*;
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
fn request(url: String) -> RequestSpec {
    serde_json::from_value(json!({"id":"r","name":"NTLM","url":url,"method":"POST","description":"","query":[],"headers":[],"body_kind":"binary","body":json!({"base64":"AAH/QQ==","file_name":"data.bin","mime":"application/octet-stream"}).to_string(),"auth":{"kind":"ntlm","token":"","username":"User","password":"Password","ntlm":{"domain":"DOMAIN","workstation":"WORKSTATION"}},"timeout_ms":3000,"follow_redirects":true,"verify_tls":true,"assertions":[],"examples":[]})).unwrap()
}
const LOCAL: NetworkPolicy = NetworkPolicy {
    allow_private_network: true,
};
#[tokio::test]
async fn mature_sspi_acceptor_verifies_ntlmv2_on_one_real_http_socket_with_exact_bytes() {
    let fixture = serve_ntlm(false).await;
    let r = request(format!("{}/resource?repeat=a&repeat=b", fixture.url));
    let response = execute(&r, None, LOCAL).await.unwrap();
    assert_eq!(response.status, 200, "{}", response.body);
    let value: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(value["connection"], 1);
    assert_eq!(value["body_base64"], "AAH/QQ==");
    assert_eq!(value["path"], "/resource?repeat=a&repeat=b");
    assert_eq!(fixture.connections.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.rounds.load(Ordering::SeqCst), 3);
    assert!(
        response
            .private_auth_values
            .iter()
            .any(|v| v.starts_with("NTLM "))
    );
    fixture.task.abort();
}
#[tokio::test]
async fn wrong_password_finishes_with_real_401_and_close_never_sends_authenticate_on_new_socket() {
    let fixture = serve_ntlm(false).await;
    let mut r = request(format!("{}/", fixture.url));
    r.auth.password = "WrongPassword".into();
    let result = execute(&r, None, LOCAL).await.unwrap();
    assert_eq!(result.status, 401);
    assert_eq!(fixture.rounds.load(Ordering::SeqCst), 3);
    fixture.task.abort();
    let fixture = serve_ntlm(true).await;
    let r = request(format!("{}/", fixture.url));
    assert!(
        execute(&r, None, LOCAL)
            .await
            .err()
            .unwrap()
            .to_string()
            .contains("closed")
    );
    assert_eq!(fixture.rounds.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.connections.load(Ordering::SeqCst), 1);
    fixture.task.abort();
}
#[tokio::test]
async fn real_tls_channel_binding_is_verified_by_the_sspi_server_and_trust_opt_out_is_explicit() {
    let fixture = serve_ntlm_tls(false, true).await;
    let mut r = request(format!("{}/", fixture.url));
    assert!(execute(&r, None, LOCAL).await.is_err());
    assert_eq!(fixture.rounds.load(Ordering::SeqCst), 0);
    r.verify_tls = false;
    let response = execute(&r, None, LOCAL).await.unwrap();
    assert_eq!(response.status, 200, "{}", response.body);
    r.auth.ntlm.as_mut().unwrap().channel_binding = false;
    let response = execute(&r, None, LOCAL).await.unwrap();
    assert_eq!(
        response.status, 200,
        "SDK acceptor allows missing binding unless EPA policy requires it"
    );
    let mismatched = serve_ntlm_tls_binding(false, true, true).await;
    r.url = format!("{}/", mismatched.url);
    r.auth.ntlm.as_mut().unwrap().channel_binding = true;
    assert_eq!(
        execute(&r, None, LOCAL).await.unwrap().status,
        401,
        "a wrong binding must be rejected"
    );
    mismatched.task.abort();
    fixture.task.abort();
}
#[tokio::test]
async fn redirects_restart_same_origin_ntlm_but_cross_origin_challenges_do_not_receive_credentials()
{
    let fixture = serve_ntlm(false).await;
    let r = request(format!("{}/redirect", fixture.url));
    let response = execute(&r, None, LOCAL).await.unwrap();
    assert_eq!(response.status, 200);
    let value: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(value["method"], "GET");
    assert_eq!(value["body_base64"], "");
    assert_eq!(fixture.connections.load(Ordering::SeqCst), 2);
    assert_eq!(fixture.rounds.load(Ordering::SeqCst), 6);
    fixture.task.abort();
    let target_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_url = format!("http://{}", target_listener.local_addr().unwrap());
    let received = Arc::new(AtomicUsize::new(0));
    let seen = received.clone();
    let target = tokio::spawn(async move {
        let app = axum::Router::new().route(
            "/",
            axum::routing::post(move |headers: axum::http::HeaderMap| {
                let seen = seen.clone();
                async move {
                    assert!(!headers.contains_key("authorization"));
                    seen.fetch_add(1, Ordering::SeqCst);
                    (
                        axum::http::StatusCode::UNAUTHORIZED,
                        [("www-authenticate", "NTLM")],
                        "not authenticated",
                    )
                }
            }),
        );
        axum::serve(target_listener, app).await.unwrap()
    });
    let redirect_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let redirect_url = format!("http://{}", redirect_listener.local_addr().unwrap());
    let redirect = tokio::spawn(async move {
        let app = axum::Router::new().route(
            "/",
            axum::routing::post(move || {
                let target_url = target_url.clone();
                async move {
                    (
                        axum::http::StatusCode::TEMPORARY_REDIRECT,
                        [("location", target_url)],
                    )
                }
            }),
        );
        axum::serve(redirect_listener, app).await.unwrap()
    });
    assert_eq!(
        execute(&request(redirect_url), None, LOCAL)
            .await
            .unwrap()
            .status,
        401
    );
    assert_eq!(received.load(Ordering::SeqCst), 1);
    target.abort();
    redirect.abort();
}
#[tokio::test]
async fn timeouts_and_cancellation_close_the_owned_socket_and_malformed_type2_never_authenticates()
{
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    for abort in [false, true] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let started = Arc::new(tokio::sync::Notify::new());
        let ready = started.clone();
        let closed = Arc::new(tokio::sync::Notify::new());
        let done = closed.clone();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = [0u8; 4096];
            let mut received = Vec::new();
            while !received.windows(4).any(|v| v == b"\r\n\r\n") {
                let n = socket.read(&mut buffer).await.unwrap();
                received.extend_from_slice(&buffer[..n]);
            }
            ready.notify_one();
            while socket.read(&mut buffer).await.is_ok_and(|n| n > 0) {}
            done.notify_one();
        });
        let mut r = request(url);
        r.timeout_ms = 1000;
        r.body_kind = "none".into();
        r.body.clear();
        let running = tokio::spawn(async move { execute(&r, None, LOCAL).await });
        started.notified().await;
        if abort {
            running.abort();
        } else {
            assert!(
                running
                    .await
                    .unwrap()
                    .err()
                    .unwrap()
                    .to_string()
                    .contains("timed out")
            );
        }
        tokio::time::timeout(std::time::Duration::from_secs(2), closed.notified())
            .await
            .unwrap();
        server.abort();
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 8192];
        let _ = socket.read(&mut buf).await.unwrap();
        socket
            .write_all(
                b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nWWW-Authenticate: NTLM\r\n\r\n",
            )
            .await
            .unwrap();
        let _ = socket.read(&mut buf).await.unwrap();
        socket.write_all(b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nWWW-Authenticate: NTLM eA==\r\n\r\n").await.unwrap();
    });
    let mut r = request(url);
    r.body_kind = "none".into();
    r.body.clear();
    assert!(execute(&r, None, LOCAL).await.is_err());
    server.abort();
}

#[tokio::test]
async fn ntlm_connect_proxy_keeps_one_authenticated_socket_and_proxy_credentials_private() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let fixture = serve_ntlm(false).await;
    let target = url::Url::parse(&fixture.url).unwrap();
    let target_address = format!("127.0.0.1:{}", target.port().unwrap());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_url = format!("http://{}", listener.local_addr().unwrap());
    let proxy = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut header = Vec::new();
        while !header.ends_with(b"\r\n\r\n") {
            header.push(socket.read_u8().await.unwrap());
            assert!(header.len() < 8192);
        }
        let header = String::from_utf8(header).unwrap();
        assert!(header.starts_with(&format!("CONNECT {target_address} HTTP/1.1")));
        assert!(
            header.to_ascii_lowercase().contains(
                "proxy-authorization: basic dXNlcjpwYXNz"
                    .to_ascii_lowercase()
                    .as_str()
            )
        );
        let mut target = tokio::net::TcpStream::connect(target_address)
            .await
            .unwrap();
        socket
            .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
            .await
            .unwrap();
        let _ = tokio::io::copy_bidirectional(&mut socket, &mut target).await;
    });
    let mut r = request(format!("{}/proxy", fixture.url));
    r.network = Some(Box::new(RequestNetwork {
        proxy: RequestProxy {
            enabled: true,
            url: proxy_url,
            username: "user".into(),
            password: "pass".into(),
            ..Default::default()
        },
        ..Default::default()
    }));
    let result = execute(&r, None, LOCAL).await.unwrap();
    assert_eq!(result.status, 200, "{}", result.body);
    assert_eq!(fixture.connections.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.rounds.load(Ordering::SeqCst), 3);
    fixture.task.abort();
    proxy.abort();
}
