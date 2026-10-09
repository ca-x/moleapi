use axum::{
    Router,
    body::Body,
    extract::ws::{Message as AxumMessage, WebSocketUpgrade},
    routing::get,
};
use moleapi_core::{NetworkPolicy, RequestSpec};
use moleapi_protocols::*;
use serde_json::json;
use std::{sync::Arc, time::Duration};
fn request(url: &str, kind: &str) -> RequestSpec {
    serde_json::from_value(json!({"protocol":{"kind":kind},"id":"r","name":"Fixture","method":"GET","url":url,"description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":true,"verify_tls":true,"assertions":[],"examples":[]})).unwrap()
}
async fn serve(router: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    (
        url,
        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        }),
    )
}
fn start(manager: &SessionManager, request: RequestSpec, allow: bool) -> String {
    let summary = manager
        .register(
            "owner",
            "workspace",
            &request,
            request.url.clone(),
            PreparedFeedback::default(),
        )
        .unwrap();
    manager
        .start(
            "owner",
            &summary.id,
            request,
            NetworkPolicy {
                allow_private_network: allow,
            },
            Arc::new(str::to_owned),
        )
        .unwrap();
    summary.id
}
async fn wait(manager: &SessionManager, id: &str, state: SessionState) -> SessionSummary {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let s = manager.summary("owner", id).unwrap();
            if s.state == state {
                return s;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|e| {
        panic!(
            "{e}; wanted {state:?}; got {:?}",
            manager.summary("owner", id)
        )
    })
}
#[tokio::test]
async fn real_sse_split_multiline_id_retry_and_network_policy() {
    let router = Router::new().route(
        "/events",
        get(|| async {
            let chunks = [
                "id: fi",
                "rst\nevent: update\nretry: 2500\ndata: line one\nda",
                "ta: line two\n\n",
            ];
            let stream = futures_util::stream::iter(chunks.map(Ok::<_, std::io::Error>));
            (
                [("content-type", "text/event-stream")],
                Body::from_stream(stream),
            )
        }),
    );
    let (url, server) = serve(router).await;
    let manager = SessionManager::new();
    let id = start(&manager, request(&format!("{url}/events"), "sse"), true);
    wait(&manager, &id, SessionState::Closed).await;
    let batch = manager.events("owner", &id, 0).unwrap();
    let event = batch
        .events
        .iter()
        .find_map(|e| {
            if let EventMessage::Sse {
                event,
                data,
                id,
                retry,
            } = &e.message
            {
                Some((event, data, id, retry))
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(event.0, "update");
    assert_eq!(event.1, "line one\nline two");
    assert_eq!(event.2, "first");
    assert_eq!(*event.3, Some(2500));
    assert!(manager.summary("other-owner", &id).is_err());
    assert!(manager.events("other-owner", &id, 0).is_err());
    let denied = start(&manager, request(&format!("{url}/events"), "sse"), false);
    assert!(
        wait(&manager, &denied, SessionState::Error)
            .await
            .reason
            .unwrap()
            .contains("blocked")
    );
    server.abort();
}
#[tokio::test]
async fn unterminated_sse_buffer_is_bounded_and_cancellation_covers_connect_and_read() {
    let router = Router::new()
        .route(
            "/large",
            get(|| async {
                let chunks = futures_util::stream::iter(
                    (0..130).map(|_| Ok::<_, std::io::Error>("x".repeat(65536))),
                );
                (
                    [("content-type", "text/event-stream")],
                    Body::from_stream(chunks),
                )
            }),
        )
        .route(
            "/connecting",
            get(|| async {
                tokio::time::sleep(Duration::from_secs(30)).await;
                "late"
            }),
        )
        .route(
            "/reading",
            get(|| async {
                (
                    [("content-type", "text/event-stream")],
                    Body::from_stream(futures_util::stream::pending::<
                        Result<&'static str, std::io::Error>,
                    >()),
                )
            }),
        );
    let (url, server) = serve(router).await;
    let manager = SessionManager::new();
    let large = start(&manager, request(&format!("{url}/large"), "sse"), true);
    let s = wait(&manager, &large, SessionState::Error).await;
    assert!(s.reason.unwrap().contains("pending-event wire limit"));
    assert!(s.received_bytes <= MAX_WIRE as u64);
    let connecting = start(&manager, request(&format!("{url}/connecting"), "sse"), true);
    assert_eq!(
        manager.close("owner", &connecting).await.unwrap().state,
        SessionState::Closed
    );
    let reading = start(&manager, request(&format!("{url}/reading"), "sse"), true);
    wait(&manager, &reading, SessionState::Open).await;
    assert_eq!(
        manager.close("owner", &reading).await.unwrap().state,
        SessionState::Closed
    );
    server.abort();
}
#[tokio::test]
async fn real_websocket_text_binary_ping_pong_and_peer_close() {
    let router = Router::new().route(
        "/socket",
        get(|upgrade: WebSocketUpgrade| async {
            upgrade.on_upgrade(|mut socket| async move {
                socket
                    .send(AxumMessage::Ping(vec![9].into()))
                    .await
                    .unwrap();
                while let Some(Ok(message)) = socket.recv().await {
                    match message {
                        AxumMessage::Text(text) if text == "finish" => {
                            socket
                                .send(AxumMessage::Close(Some(axum::extract::ws::CloseFrame {
                                    code: 1000,
                                    reason: "fixture finished".into(),
                                })))
                                .await
                                .unwrap();
                            break;
                        }
                        AxumMessage::Text(_) | AxumMessage::Binary(_) => {
                            socket.send(message).await.unwrap();
                        }
                        AxumMessage::Ping(bytes) => {
                            socket.send(AxumMessage::Pong(bytes)).await.unwrap();
                        }
                        _ => {}
                    }
                }
            })
        }),
    );
    let (url, server) = serve(router).await;
    let manager = SessionManager::new();
    let url = url.replacen("http", "ws", 1);
    let id = start(
        &manager,
        request(&format!("{url}/socket"), "websocket"),
        true,
    );
    let s = wait(&manager, &id, SessionState::Open).await;
    assert_eq!(s.handshake.unwrap().status, 101);
    manager
        .send(
            "owner",
            &id,
            SendMessage::Text {
                text: "hello".into(),
            },
        )
        .unwrap();
    manager
        .send(
            "owner",
            &id,
            SendMessage::Binary {
                base64: "AAEC/w==".into(),
            },
        )
        .unwrap();
    manager
        .send(
            "owner",
            &id,
            SendMessage::Ping {
                base64: "Ag==".into(),
            },
        )
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let e = manager.events("owner", &id, 0).unwrap().events;
            if e.iter().any(|e| {
                e.direction == "incoming"
                    && matches!(&e.message,EventMessage::Text{text} if text=="hello")
            }) && e.iter().any(|e| {
                e.direction == "incoming"
                    && matches!(&e.message,EventMessage::Binary{base64} if base64=="AAEC/w==")
            }) && e.iter().any(|e| {
                e.direction == "incoming"
                    && matches!(&e.message,EventMessage::Pong{base64} if base64=="Ag==")
            }) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(
        manager
            .send(
                "owner",
                &id,
                SendMessage::Binary {
                    base64: "A".repeat(MAX_MESSAGE * 2)
                }
            )
            .is_err()
    );
    assert!(
        manager
            .send(
                "owner",
                &id,
                SendMessage::Ping {
                    base64: "A".repeat(172)
                }
            )
            .is_err()
    );
    manager
        .send(
            "owner",
            &id,
            SendMessage::Text {
                text: "finish".into(),
            },
        )
        .unwrap();
    assert!(
        wait(&manager, &id, SessionState::Closed)
            .await
            .reason
            .unwrap()
            .contains("1000")
    );
    let e = manager.events("owner", &id, 0).unwrap().events;
    assert!(e.iter().any(|e|matches!(&e.message,EventMessage::Close{code:Some(1000),reason} if reason=="fixture finished")));
    assert!(e.iter().any(|e| e.direction == "outgoing"
        && matches!(&e.message,EventMessage::Pong{base64} if base64=="CQ==")));
    server.abort();
}

#[tokio::test]
async fn sse_total_wire_ceiling_applies_to_many_valid_events() {
    let router = Router::new().route(
        "/events",
        get(|| async {
            let chunks = futures_util::stream::iter(
                (0..140)
                    .map(|_| Ok::<_, std::io::Error>(format!("data: {}\n\n", "x".repeat(65520)))),
            );
            (
                [("content-type", "text/event-stream")],
                Body::from_stream(chunks),
            )
        }),
    );
    let (url, server) = serve(router).await;
    let manager = SessionManager::new();
    let id = start(&manager, request(&format!("{url}/events"), "sse"), true);
    let summary = wait(&manager, &id, SessionState::Error).await;
    assert!(summary.reason.unwrap().contains("8 MiB"));
    assert!(summary.received_bytes <= MAX_WIRE as u64);
    assert!(summary.event_count > 100);
    server.abort();
}
#[tokio::test]
async fn websocket_cancel_connecting_reading_and_manager_shutdown() {
    let router = Router::new()
        .route(
            "/connecting",
            get(|| async {
                tokio::time::sleep(Duration::from_secs(30)).await;
                "late"
            }),
        )
        .route(
            "/reading",
            get(|upgrade: WebSocketUpgrade| async {
                upgrade
                    .on_upgrade(|mut socket| async move { while socket.recv().await.is_some() {} })
            }),
        );
    let (url, server) = serve(router).await;
    let url = url.replacen("http", "ws", 1);
    let manager = SessionManager::new();
    let connecting = start(
        &manager,
        request(&format!("{url}/connecting"), "websocket"),
        true,
    );
    assert_eq!(
        manager.close("owner", &connecting).await.unwrap().state,
        SessionState::Closed
    );
    let reading = start(
        &manager,
        request(&format!("{url}/reading"), "websocket"),
        true,
    );
    wait(&manager, &reading, SessionState::Open).await;
    assert_eq!(
        manager.close("owner", &reading).await.unwrap().state,
        SessionState::Closed
    );
    assert!(
        manager
            .events("owner", &reading, 0)
            .unwrap()
            .events
            .iter()
            .any(|e| e.direction == "outgoing" && matches!(e.message, EventMessage::Close { .. }))
    );
    let denied = start(
        &manager,
        request(&format!("{url}/reading"), "websocket"),
        false,
    );
    assert!(
        wait(&manager, &denied, SessionState::Error)
            .await
            .reason
            .unwrap()
            .contains("blocked")
    );
    server.abort();
}
#[tokio::test]
async fn live_redirects_strip_cross_origin_credentials_and_reject_unsafe_scheme() {
    let target = Router::new()
        .route(
            "/events",
            get(|headers: axum::http::HeaderMap| async move {
                assert!(!headers.contains_key("authorization"));
                assert!(!headers.contains_key("cookie"));
                (
                    [("content-type", "text/event-stream")],
                    "data: redirected\n\n",
                )
            }),
        )
        .route(
            "/socket",
            get(
                |headers: axum::http::HeaderMap, upgrade: WebSocketUpgrade| async move {
                    assert!(!headers.contains_key("authorization"));
                    assert!(!headers.contains_key("cookie"));
                    upgrade.on_upgrade(|mut socket| async move {
                        socket.send(AxumMessage::Close(None)).await.unwrap();
                    })
                },
            ),
        );
    let (target, target_server) = serve(target).await;
    let redirect = Router::new()
        .route(
            "/events",
            get({
                let target = target.clone();
                move || async move {
                    (
                        axum::http::StatusCode::FOUND,
                        [("location", format!("{target}/events"))],
                    )
                }
            }),
        )
        .route(
            "/socket",
            get(move || async move {
                (
                    axum::http::StatusCode::FOUND,
                    [("location", format!("{target}/socket"))],
                )
            }),
        )
        .route(
            "/unsafe",
            get(|| async {
                (
                    axum::http::StatusCode::FOUND,
                    [("location", "file:///tmp/forbidden")],
                )
            }),
        );
    let (url, redirect_server) = serve(redirect).await;
    let manager = SessionManager::new();
    for (kind, path) in [("sse", "events"), ("websocket", "socket")] {
        let endpoint = if kind == "websocket" {
            url.replacen("http", "ws", 1)
        } else {
            url.clone()
        };
        let mut request = request(&format!("{endpoint}/{path}"), kind);
        request.auth.kind = "bearer".into();
        request.auth.token = "private".into();
        request.headers = serde_json::from_value(
            json!([{"id":"cookie","key":"cookie","value":"private","enabled":true}]),
        )
        .unwrap();
        let id = start(&manager, request, true);
        assert_eq!(
            wait(&manager, &id, SessionState::Closed).await.state,
            SessionState::Closed
        );
    }
    let id = start(&manager, request(&format!("{url}/unsafe"), "sse"), true);
    assert!(
        wait(&manager, &id, SessionState::Error)
            .await
            .reason
            .unwrap()
            .contains("scheme")
    );
    redirect_server.abort();
    target_server.abort();
}

#[tokio::test]
async fn engine_rejects_non_get_or_active_body_modes_before_connecting() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let connections = Arc::new(AtomicUsize::new(0));
    let count = connections.clone();
    let router = Router::new().route(
        "/events",
        get(move |request: axum::extract::Request| {
            let count = count.clone();
            async move {
                assert!(
                    axum::body::to_bytes(request.into_body(), 1024)
                        .await
                        .unwrap()
                        .is_empty()
                );
                count.fetch_add(1, Ordering::SeqCst);
                ([("content-type", "text/event-stream")], "data: no body\n\n")
            }
        }),
    );
    let (url, server) = serve(router).await;
    let manager = SessionManager::new();
    for (method, body_kind) in [("POST", "none"), ("GET", "json")] {
        let mut r = request(&format!("{url}/events"), "sse");
        r.method = method.into();
        r.body_kind = body_kind.into();
        r.body = "{\"draft\":true}".into();
        let id = start(&manager, r, true);
        assert!(
            wait(&manager, &id, SessionState::Error)
                .await
                .reason
                .unwrap()
                .contains("body mode None")
        );
    }
    assert_eq!(connections.load(Ordering::SeqCst), 0);
    let mut r = request(&format!("{url}/events"), "sse");
    r.body = "saved draft {{unresolved}}".into();
    let id = start(&manager, r, true);
    wait(&manager, &id, SessionState::Closed).await;
    assert_eq!(connections.load(Ordering::SeqCst), 1);
    server.abort();
}

#[tokio::test]
async fn sse_and_websocket_network_dns_settings_preserve_handshake_host_and_private_policy() {
    let router = Router::new()
        .route(
            "/events",
            get(|headers: axum::http::HeaderMap| async move {
                assert!(
                    headers["host"]
                        .to_str()
                        .unwrap()
                        .starts_with("live-network.test:")
                );
                (
                    [("content-type", "text/event-stream")],
                    "data: network-settings\n\n",
                )
            }),
        )
        .route(
            "/socket",
            get(
                |headers: axum::http::HeaderMap, ws: WebSocketUpgrade| async move {
                    assert!(
                        headers["host"]
                            .to_str()
                            .unwrap()
                            .starts_with("live-network.test:")
                    );
                    ws.on_upgrade(|mut socket| async move {
                        socket
                            .send(AxumMessage::Text("network-settings".into()))
                            .await
                            .unwrap();
                        socket.send(AxumMessage::Close(None)).await.unwrap();
                    })
                },
            ),
        );
    let (url, server) = serve(router).await;
    let port = url::Url::parse(&url).unwrap().port().unwrap();
    let manager = SessionManager::new();
    for (kind, scheme, path) in [("sse", "http", "events"), ("websocket", "ws", "socket")] {
        let mut r = request(&format!("{scheme}://live-network.test:{port}/{path}"), kind);
        r.network = Some(Box::new(moleapi_core::RequestNetwork {
            dns: vec![moleapi_core::DnsOverride {
                hostname: "live-network.test".into(),
                addresses: vec!["127.0.0.1".into()],
            }],
            ..Default::default()
        }));
        moleapi_core::validate_request(&r, false).unwrap();
        let denied = start(&manager, r.clone(), false);
        assert!(
            wait(&manager, &denied, SessionState::Error)
                .await
                .reason
                .unwrap()
                .contains("blocked")
        );
        let id = start(&manager, r, true);
        wait(&manager, &id, SessionState::Closed).await;
        assert!(
            manager
                .events("owner", &id, 0)
                .unwrap()
                .events
                .iter()
                .any(|event| match &event.message {
                    EventMessage::Sse { data, .. } => data == "network-settings",
                    EventMessage::Text { text } => text == "network-settings",
                    _ => false,
                })
        );
    }
    server.abort();
}
#[tokio::test]
async fn live_redirects_do_not_forward_client_identity_to_a_different_origin() {
    use bytes::Bytes;
    use futures_util::SinkExt;
    use http_body_util::Full;
    use hyper_util::rt::TokioIo;
    use rcgen::{BasicConstraints, CertificateParams, ExtendedKeyUsagePurpose, IsCa, KeyPair};
    use std::convert::Infallible;
    let mut params = CertificateParams::new(vec!["Live Redirect CA".into()]).unwrap();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "live-redirect-ca");
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let ca_key = KeyPair::generate().unwrap();
    let ca = params.self_signed(&ca_key).unwrap();
    let issuer = rcgen::Issuer::from_params(&params, &ca_key);
    let mut params = CertificateParams::new(vec!["live-redirect.test".into()]).unwrap();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "live-redirect.test");
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    let server_key = KeyPair::generate().unwrap();
    let cert = params.signed_by(&server_key, &issuer).unwrap();
    let mut params = CertificateParams::new(vec!["client.test".into()]).unwrap();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "live-client");
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
    let client_key = KeyPair::generate().unwrap();
    let client_cert = params.signed_by(&client_key, &issuer).unwrap();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut roots = rustls::RootCertStore::empty();
    roots.add(ca.der().clone()).unwrap();
    let roots = Arc::new(roots);
    let required = rustls::server::WebPkiClientVerifier::builder_with_provider(
        roots.clone(),
        provider.clone(),
    )
    .build()
    .unwrap();
    let optional =
        rustls::server::WebPkiClientVerifier::builder_with_provider(roots, provider.clone())
            .allow_unauthenticated()
            .build()
            .unwrap();
    let config = |verifier| {
        rustls::ServerConfig::builder_with_provider(provider.clone())
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_client_cert_verifier(verifier)
            .with_single_cert(
                vec![cert.der().clone()],
                rustls::pki_types::PrivatePkcs8KeyDer::from(server_key.serialize_der()).into(),
            )
            .unwrap()
    };
    let source_config = Arc::new(config(required));
    let target_config = Arc::new(config(optional));
    for kind in ["sse", "websocket"] {
        let target = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let target_url = format!(
            "https://live-redirect.test:{}/target",
            target.local_addr().unwrap().port()
        );
        let tls_config = target_config.clone();
        let target_task = tokio::spawn(async move {
            let (socket, _) = target.accept().await.unwrap();
            let tls = tokio_rustls::TlsAcceptor::from(tls_config)
                .accept(socket)
                .await
                .unwrap();
            let identity = tls
                .get_ref()
                .1
                .peer_certificates()
                .is_some_and(|c| !c.is_empty());
            let body = if identity {
                "client-present"
            } else {
                "anonymous"
            };
            if kind == "websocket" {
                let mut ws = tokio_tungstenite::accept_async(tls).await.unwrap();
                ws.send(tokio_tungstenite::tungstenite::Message::Text(body.into()))
                    .await
                    .unwrap();
                ws.close(None).await.unwrap();
            } else {
                let _ = hyper::server::conn::http1::Builder::new()
                    .serve_connection(
                        TokioIo::new(tls),
                        hyper::service::service_fn(
                            move |_: hyper::Request<hyper::body::Incoming>| async move {
                                Ok::<_, Infallible>(
                                    hyper::Response::builder()
                                        .header("connection", "close")
                                        .header("content-type", "text/event-stream")
                                        .body(Full::new(Bytes::from(format!("data:{body}\n\n"))))
                                        .unwrap(),
                                )
                            },
                        ),
                    )
                    .await;
            }
        });
        let source = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let source_url = format!(
            "{}://live-redirect.test:{}/start",
            if kind == "websocket" { "wss" } else { "https" },
            source.local_addr().unwrap().port()
        );
        let tls_config = source_config.clone();
        let source_task = tokio::spawn(async move {
            let (socket, _) = source.accept().await.unwrap();
            let tls = tokio_rustls::TlsAcceptor::from(tls_config)
                .accept(socket)
                .await
                .unwrap();
            assert!(
                tls.get_ref()
                    .1
                    .peer_certificates()
                    .is_some_and(|c| !c.is_empty())
            );
            let _ = hyper::server::conn::http1::Builder::new()
                .serve_connection(
                    TokioIo::new(tls),
                    hyper::service::service_fn(move |_: hyper::Request<hyper::body::Incoming>| {
                        let location = target_url.clone();
                        async move {
                            Ok::<_, Infallible>(
                                hyper::Response::builder()
                                    .status(302)
                                    .header("location", location)
                                    .header("connection", "close")
                                    .body(Full::new(Bytes::new()))
                                    .unwrap(),
                            )
                        }
                    }),
                )
                .await;
        });
        let mut r = request(&source_url, kind);
        r.network = Some(Box::new(moleapi_core::RequestNetwork {
            built_in_roots: false,
            ca_pem: ca.pem(),
            dns: vec![moleapi_core::DnsOverride {
                hostname: "live-redirect.test".into(),
                addresses: vec!["127.0.0.1".into()],
            }],
            identity: moleapi_core::ClientIdentity {
                enabled: true,
                certificate_pem: client_cert.pem(),
                key_pem: client_key.serialize_pem(),
                ..Default::default()
            },
            ..Default::default()
        }));
        let manager = SessionManager::new();
        let id = start(&manager, r, true);
        wait(&manager, &id, SessionState::Closed).await;
        assert!(
            manager
                .events("owner", &id, 0)
                .unwrap()
                .events
                .iter()
                .any(|e| match &e.message {
                    EventMessage::Sse { data, .. } => data == "anonymous",
                    EventMessage::Text { text } => text == "anonymous",
                    _ => false,
                })
        );
        source_task.await.unwrap();
        target_task.await.unwrap();
    }
}
