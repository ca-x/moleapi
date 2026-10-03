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
