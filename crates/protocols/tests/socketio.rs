use moleapi_core::{Environment, NetworkPolicy, RequestSpec};
use moleapi_protocols::{
    EventMessage, PreparedFeedback, SendMessage, SessionManager, SessionState,
};
use serde_json::json;
use std::{sync::Arc, time::Duration};
fn request(url: &str) -> RequestSpec {
    serde_json::from_value(json!({"protocol":{"kind":"socketio","namespace":"/fixture","path":"/custom/socket.io/","auth_source":"{\"token\":\"fixture-token\"}","listeners":["echo","question","server-ack-result","private"]},"id":"r","name":"Socket.IO fixture","method":"GET","url":url,"description":"","query":[{"id":"q","key":"required","value":"yes","enabled":true}],"headers":[{"id":"h","key":"X-Fixture","value":"yes","enabled":true}],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1500,"follow_redirects":false,"verify_tls":true,"assertions":[],"examples":[]})).unwrap()
}
fn environment() -> Environment {
    serde_json::from_value(json!({"id":"e","name":"selected","variables":[{"id":"v","key":"selected","value":"original selected scope","enabled":true},{"id":"p","key":"private","value":"private-secret","enabled":true}]})).unwrap()
}
fn start(manager: &SessionManager, request: RequestSpec, allow_private_network: bool) -> String {
    let mask: Arc<dyn Fn(&str) -> String + Send + Sync> = Arc::new(|s| {
        s.replace("private-secret", "[REDACTED]")
            .replace("fixture-token", "[REDACTED]")
    });
    let summary = manager
        .register(
            "owner",
            "w",
            &request,
            "safe".into(),
            PreparedFeedback::default(),
        )
        .unwrap();
    manager
        .configure_socketio("owner", &summary.id, environment(), mask.clone())
        .unwrap();
    manager
        .start(
            "owner",
            &summary.id,
            request,
            NetworkPolicy {
                allow_private_network,
            },
            mask,
        )
        .unwrap();
    summary.id
}
async fn state(manager: &SessionManager, id: &str, expected: SessionState) {
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            let summary = manager.summary("owner", id).unwrap();
            if summary.state == expected {
                return;
            }
            assert!(summary.state.live(), "Expected {expected:?}: {summary:?}");
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}
async fn event(
    manager: &SessionManager,
    id: &str,
    predicate: impl Fn(&EventMessage) -> bool,
) -> EventMessage {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let batch = manager.events("owner", id, 0).unwrap();
            for event in batch.events {
                if event.direction == "incoming" && predicate(&event.message) {
                    return event.message;
                }
            }
            let summary = manager.summary("owner", id).unwrap();
            assert!(summary.state.live(), "{summary:?}");
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap()
}
fn emit(
    event: &str,
    source: &str,
    attachments: &[&str],
    ack: Option<&str>,
    timeout: u64,
) -> SendMessage {
    SendMessage::SocketioEmit {
        event: event.into(),
        arguments_source: source.into(),
        attachments_base64: attachments.iter().map(|s| (*s).into()).collect(),
        ack_id: ack.map(str::to_owned),
        ack_timeout_ms: timeout,
    }
}
fn fixture() -> String {
    std::env::var("MOLEAPI_SOCKETIO_FIXTURE_URL").unwrap_or_else(|_| "ws://127.0.0.1:18886".into())
}
#[tokio::test]
#[ignore = "official Socket.IO 4 fixture: npm ci and node server.cjs; see fixtures/socketio/README.md"]
async fn official_socketio4_namespace_custompath_json_mixedbinary_ack_timeout_and_server_reply() {
    let manager = SessionManager::new();
    let id = start(&manager, request(&fixture()), true);
    state(&manager, &id, SessionState::Open).await;
    assert_eq!(manager.summary("owner", &id).unwrap().protocol, "socketio");
    assert_eq!(
        manager
            .summary("owner", &id)
            .unwrap()
            .handshake
            .unwrap()
            .status,
        101
    );
    let source = r#"["{{selected}}", {"nested":[{"_placeholder":true,"num":0},"tail"]}, {"_placeholder":true,"num":1}, 42]"#;
    manager
        .send(
            "owner",
            &id,
            emit("echo", source, &["AP8H", "AQID"], Some("mixed"), 1000),
        )
        .unwrap();
    let received = event(
        &manager,
        &id,
        |e| matches!(e, EventMessage::SocketioEvent { event, .. } if event == "echo"),
    )
    .await;
    let EventMessage::SocketioEvent {
        arguments,
        attachments_base64,
        ..
    } = received
    else {
        unreachable!()
    };
    assert_eq!(arguments[0], "original selected scope");
    assert_eq!(
        arguments[1],
        json!({"nested":[{"_placeholder":true,"num":0},"tail"]})
    );
    assert_eq!(arguments[2], json!({"_placeholder":true,"num":1}));
    assert_eq!(arguments[3], 42);
    assert_eq!(attachments_base64, ["AP8H", "AQID"]);
    let ack = event(
        &manager,
        &id,
        |e| matches!(e, EventMessage::SocketioAck { ack_id, .. } if ack_id == "mixed"),
    )
    .await;
    let EventMessage::SocketioAck {
        status,
        arguments: ack_arguments,
        attachments_base64: ack_binary,
        ..
    } = ack
    else {
        unreachable!()
    };
    assert_eq!(status, "ok");
    assert_eq!(ack_arguments, arguments);
    assert_eq!(ack_binary, attachments_base64);
    manager
        .send(
            "owner",
            &id,
            emit("ack-timeout", "[]", &[], Some("timeout"), 100),
        )
        .unwrap();
    assert!(
        manager
            .send(
                "owner",
                &id,
                emit("ack-timeout", "[]", &[], Some("timeout"), 100)
            )
            .is_err()
    );
    let timed = event(&manager, &id, |e| matches!(e, EventMessage::SocketioAck { ack_id, status, .. } if ack_id == "timeout" && status == "timeout")).await;
    assert!(matches!(
        timed,
        EventMessage::SocketioAck { error: Some(_), .. }
    ));
    manager
        .send(
            "owner",
            &id,
            emit("ack-error", "[null]", &[], Some("app-error"), 1000),
        )
        .unwrap();
    let ack = event(
        &manager,
        &id,
        |e| matches!(e, EventMessage::SocketioAck { ack_id, .. } if ack_id == "app-error"),
    )
    .await;
    assert!(
        matches!(ack, EventMessage::SocketioAck { status, arguments, .. } if status == "ok" && arguments == vec![json!({"error":"application-error","code":42})])
    );
    manager
        .send("owner", &id, emit("server-ack", "[]", &[], None, 1000))
        .unwrap();
    let question = event(
        &manager,
        &id,
        |e| matches!(e, EventMessage::SocketioEvent { event, .. } if event == "question"),
    )
    .await;
    let EventMessage::SocketioEvent {
        ack_id: Some(token),
        attachments_base64,
        arguments,
        ..
    } = question
    else {
        panic!("Missing server ACK")
    };
    assert_eq!(arguments[0], "answer me");
    assert_eq!(attachments_base64, ["AP8H"]);
    let reply = SendMessage::SocketioAck {
        ack_id: token.clone(),
        arguments_source: r#"["answered",{"_placeholder":true,"num":0}]"#.into(),
        attachments_base64: vec!["/wA=".into()],
    };
    assert!(manager.send("other-owner", &id, reply.clone()).is_err());
    manager.send("owner", &id, reply.clone()).unwrap();
    assert!(manager.send("owner", &id, reply).is_err());
    let result = event(
        &manager,
        &id,
        |e| matches!(e, EventMessage::SocketioEvent { event, .. } if event == "server-ack-result"),
    )
    .await;
    assert!(
        matches!(result, EventMessage::SocketioEvent { arguments, attachments_base64, .. } if arguments[0] == json!({"error":null}) && arguments[1] == "answered" && attachments_base64 == ["/wA="])
    );
    // Empty binary is an attachment, distinct from an omitted argument.
    manager
        .send(
            "owner",
            &id,
            emit(
                "echo",
                r#"[{"_placeholder":true,"num":0},"after empty"]"#,
                &[""],
                Some("empty-binary"),
                1000,
            ),
        )
        .unwrap();
    let empty_ack = event(
        &manager,
        &id,
        |e| matches!(e, EventMessage::SocketioAck { ack_id, .. } if ack_id == "empty-binary"),
    )
    .await;
    assert!(
        matches!(empty_ack, EventMessage::SocketioAck { status, arguments, attachments_base64, .. } if status == "ok" && arguments[0] == json!({"_placeholder":true,"num":0}) && arguments[1] == "after empty" && attachments_base64 == [""])
    );
    // Near-limit decoded binary remains legal even though API base64 expands its representation.
    use base64::Engine;
    // Arbitrary binary includes every byte, especially Engine.IO polling delimiter 0x1e.
    let arbitrary =
        base64::engine::general_purpose::STANDARD.encode((0..=255u8).collect::<Vec<_>>());
    manager
        .send(
            "owner",
            &id,
            emit(
                "echo",
                r#"[{"_placeholder":true,"num":0}]"#,
                &[&arbitrary],
                Some("all-bytes"),
                1000,
            ),
        )
        .unwrap();
    let arbitrary_ack = event(
        &manager,
        &id,
        |e| matches!(e, EventMessage::SocketioAck { ack_id, .. } if ack_id == "all-bytes"),
    )
    .await;
    assert!(
        matches!(arbitrary_ack, EventMessage::SocketioAck { status, attachments_base64, .. } if status == "ok" && attachments_base64 == [arbitrary])
    );
    let large = base64::engine::general_purpose::STANDARD.encode(vec![0xab; 900_000]);
    manager
        .send(
            "owner",
            &id,
            emit(
                "echo",
                r#"[{"_placeholder":true,"num":0}]"#,
                &[&large],
                Some("large-binary"),
                1000,
            ),
        )
        .unwrap();
    let large_ack = event(
        &manager,
        &id,
        |e| matches!(e, EventMessage::SocketioAck { ack_id, .. } if ack_id == "large-binary"),
    )
    .await;
    assert!(
        matches!(large_ack, EventMessage::SocketioAck { status, attachments_base64, .. } if status == "ok" && attachments_base64 == [large])
    );
    // Concurrent acknowledged binary packets cannot interleave header/attachment groups.
    for n in 0..16 {
        manager
            .send(
                "owner",
                &id,
                emit(
                    "echo",
                    &format!(r#"[{n},{{"_placeholder":true,"num":0}}]"#),
                    &["AP8H"],
                    Some(&format!("ordered-{n}")),
                    1000,
                ),
            )
            .unwrap();
    }
    for n in 0..16 {
        let ack = event(&manager, &id, |e| matches!(e, EventMessage::SocketioAck { ack_id, .. } if ack_id == &format!("ordered-{n}"))).await;
        assert!(
            matches!(ack, EventMessage::SocketioAck { status, arguments, attachments_base64, .. } if status == "ok" && arguments[0] == n && attachments_base64 == ["AP8H"])
        );
    }
    // SDK heartbeat is exercised across several server ping cycles.
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(
        manager.summary("owner", &id).unwrap().state,
        SessionState::Open
    );
    manager
        .send("owner", &id, emit("close-now", "[]", &[], None, 1000))
        .unwrap();
    state(&manager, &id, SessionState::Closed).await;
    assert!(
        manager
            .summary("owner", &id)
            .unwrap()
            .reason
            .unwrap()
            .contains("namespace disconnected")
    );
    manager.remove("owner", &id).await.unwrap();
}
#[tokio::test]
#[ignore = "official Socket.IO 4 fixture"]
async fn official_socketio4_listener_updates_privacy_auth_rejection_and_cancellation() {
    let manager = SessionManager::new();
    let id = start(&manager, request(&fixture()), true);
    state(&manager, &id, SessionState::Open).await;
    manager
        .send(
            "owner",
            &id,
            SendMessage::SocketioListen {
                event: "private".into(),
                enabled: false,
            },
        )
        .unwrap();
    manager
        .send(
            "owner",
            &id,
            emit("private", r#"["{{private}}"]"#, &[], None, 1000),
        )
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!manager.events("owner", &id, 0).unwrap().events.iter().any(|e| e.direction == "incoming" && matches!(&e.message, EventMessage::SocketioEvent { event, .. } if event == "private")));
    manager
        .send(
            "owner",
            &id,
            SendMessage::SocketioListen {
                event: "private".into(),
                enabled: true,
            },
        )
        .unwrap();
    manager
        .send(
            "owner",
            &id,
            emit("private", r#"["{{private}}"]"#, &[], None, 1000),
        )
        .unwrap();
    event(
        &manager,
        &id,
        |e| matches!(e, EventMessage::SocketioEvent { event, .. } if event == "private"),
    )
    .await;
    let serialized = serde_json::to_string(&manager.events("owner", &id, 0).unwrap()).unwrap();
    assert!(!serialized.contains("private-secret") && !serialized.contains("fixture-token"));
    manager
        .send(
            "owner",
            &id,
            emit("ack-timeout", "[]", &[], Some("cancelled"), 120_000),
        )
        .unwrap();
    manager.close_owner("owner").await;
    assert_eq!(
        manager.summary("owner", &id).unwrap().state,
        SessionState::Closed
    );
    assert!(
        manager
            .send("owner", &id, emit("echo", "[]", &[], None, 1000))
            .is_err()
    );
    manager.remove("owner", &id).await.unwrap();
    let mut bad = request(&fixture());
    if let moleapi_core::Protocol::Socketio { auth_source, .. } = &mut bad.protocol {
        *auth_source = r#"{"token":"wrong"}"#.into();
    }
    let denied = start(&manager, bad, true);
    state(&manager, &denied, SessionState::Error).await;
    assert!(
        manager
            .summary("owner", &denied)
            .unwrap()
            .reason
            .unwrap()
            .contains("namespace rejected")
    );
}
#[tokio::test]
async fn socketio_policy_pending_cancel_validation_and_source_preserving_defaults() {
    let mut draft = request("ws://127.0.0.1:18886");
    let value = serde_json::to_value(&draft.protocol).unwrap();
    assert_eq!(value["event"], "message");
    assert_eq!(value["arguments_source"], "[]");
    assert_eq!(value["ack_timeout_ms"], 5000);
    if let moleapi_core::Protocol::Socketio {
        auth_source,
        arguments_source,
        ..
    } = &mut draft.protocol
    {
        *auth_source = "incomplete auth".into();
        *arguments_source = "[unfinished{{later}}".into();
    }
    assert!(moleapi_core::validate_request(&draft, true).is_ok());
    assert!(moleapi_core::validate_request(&draft, false).is_err());
    let manager = SessionManager::new();
    let id = start(&manager, request("ws://127.0.0.1:18886"), false);
    state(&manager, &id, SessionState::Error).await;
    assert!(
        manager
            .summary("owner", &id)
            .unwrap()
            .reason
            .unwrap()
            .contains("blocked")
    );
    manager.remove("owner", &id).await.unwrap();
    let summary = manager
        .register(
            "owner",
            "w",
            &request("ws://192.0.2.1:18886"),
            "safe".into(),
            PreparedFeedback::default(),
        )
        .unwrap();
    manager.close("owner", &summary.id).await.unwrap();
    assert!(
        manager
            .start(
                "owner",
                &summary.id,
                request("ws://192.0.2.1:18886"),
                NetworkPolicy {
                    allow_private_network: true
                },
                Arc::new(str::to_owned)
            )
            .is_err()
    );
    assert!(manager.events("other", &summary.id, 0).is_err());
    let mut malformed = request("ws://127.0.0.1:18886?EIO=3");
    assert!(moleapi_core::validate_request(&malformed, false).is_err());
    malformed.url = "ws://127.0.0.1:18886".into();
    if let moleapi_core::Protocol::Socketio { namespace, .. } = &mut malformed.protocol {
        *namespace = "/bad,namespace".into();
    }
    assert!(moleapi_core::validate_request(&malformed, false).is_err());
}
#[tokio::test]
async fn socketio_connect_timeout_and_stop_during_handshake() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let stalled = tokio::spawn(async move {
        let mut streams = vec![];
        loop {
            streams.push(listener.accept().await.unwrap().0);
        }
    });
    let manager = SessionManager::new();
    let mut req = request(&url);
    req.timeout_ms = 100;
    let id = start(&manager, req, true);
    state(&manager, &id, SessionState::Error).await;
    assert!(
        manager
            .summary("owner", &id)
            .unwrap()
            .reason
            .unwrap()
            .contains("timed out")
    );
    manager.remove("owner", &id).await.unwrap();
    let id = start(&manager, request(&url), true);
    tokio::time::timeout(Duration::from_secs(1), manager.close("owner", &id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        manager.summary("owner", &id).unwrap().state,
        SessionState::Closed
    );
    stalled.abort();
}
#[tokio::test]
#[ignore = "official Socket.IO TLS fixture requires node and npm ci"]
async fn official_socketio_tls_original_hostname_sni_and_certificate_policy() {
    use tokio::io::{AsyncBufReadExt, BufReader};
    let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let cert = dir.path().join("cert.pem");
    let key = dir.path().join("key.pem");
    std::fs::write(&cert, certificate.cert.pem()).unwrap();
    std::fs::write(&key, certificate.signing_key.serialize_pem()).unwrap();
    let mut server = tokio::process::Command::new("node")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/socketio/server.cjs"
        ))
        .env("TLS_CERT", cert)
        .env("TLS_KEY", key)
        .env("TLS_HOST", "localhost")
        .env("PORT", "0")
        .stdout(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut stdout = BufReader::new(server.stdout.take().unwrap()).lines();
    let ready = tokio::time::timeout(Duration::from_secs(5), stdout.next_line())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let port = ready.strip_prefix("ready:").unwrap();
    let manager = SessionManager::new();
    let mut req = request(&format!("wss://localhost:{port}"));
    req.verify_tls = false;
    let id = start(&manager, req, true);
    state(&manager, &id, SessionState::Open).await;
    manager
        .send(
            "owner",
            &id,
            emit("echo", "[\"original hostname\"]", &[], Some("tls"), 1000),
        )
        .unwrap();
    event(&manager, &id, |e| matches!(e, EventMessage::SocketioAck { ack_id, status, .. } if ack_id == "tls" && status == "ok")).await;
    manager.remove("owner", &id).await.unwrap();
    let verified = start(&manager, request(&format!("wss://localhost:{port}")), true);
    state(&manager, &verified, SessionState::Error).await;
    // The same self-signed certificate must be rejected when verification is enabled.
    assert!(
        manager
            .summary("owner", &verified)
            .unwrap()
            .reason
            .unwrap()
            .to_ascii_lowercase()
            .contains("tls")
    );
    manager.remove("owner", &verified).await.unwrap();
    let mut req = request(&format!("wss://127.0.0.1:{port}"));
    req.verify_tls = false;
    let wrong_host = start(&manager, req, true);
    state(&manager, &wrong_host, SessionState::Error).await;
    assert!(
        manager
            .summary("owner", &wrong_host)
            .unwrap()
            .reason
            .unwrap()
            .contains("Original TLS hostname required")
    );
    server.kill().await.unwrap();
}
#[tokio::test]
#[ignore = "official Socket.IO 4 fixture"]
async fn official_socketio4_ordinary_error_open_close_event_listeners_and_emits() {
    let manager = SessionManager::new();
    let mut req = request(&fixture());
    if let moleapi_core::Protocol::Socketio { listeners, .. } = &mut req.protocol {
        *listeners = vec!["error".into(), "open".into(), "close".into()];
    }
    let id = start(&manager, req, true);
    state(&manager, &id, SessionState::Open).await;
    for name in ["error", "open", "close"] {
        manager
            .send(
                "owner",
                &id,
                SendMessage::SocketioListen {
                    event: name.into(),
                    enabled: true,
                },
            )
            .unwrap();
        let source = json!([{ "ordinary_event": name }]).to_string();
        let correlation = format!("ordinary-{name}");
        manager
            .send(
                "owner",
                &id,
                emit(name, &source, &[], Some(&correlation), 1000),
            )
            .unwrap();
        let received = event(
            &manager,
            &id,
            |e| matches!(e, EventMessage::SocketioEvent { event, .. } if event == name),
        )
        .await;
        assert!(
            matches!(received, EventMessage::SocketioEvent { arguments, .. } if arguments == vec![json!({"ordinary_event":name})])
        );
        let ack = event(
            &manager,
            &id,
            |e| matches!(e, EventMessage::SocketioAck { ack_id, .. } if ack_id == &correlation),
        )
        .await;
        assert!(
            matches!(ack, EventMessage::SocketioAck { status, arguments, .. } if status == "ok" && arguments == vec![json!({"ordinary_event":name})])
        );
        // Application names must not trigger SDK lifecycle aliases or close the session.
        assert_eq!(
            manager.summary("owner", &id).unwrap().state,
            SessionState::Open
        );
    }
    manager.remove("owner", &id).await.unwrap();
}
#[tokio::test]
#[ignore = "official Socket.IO TLS fixture requires node and npm ci"]
async fn socketio_network_ca_dns_and_pem_pfx_mutual_tls_preserve_original_sni() {
    use rcgen::{BasicConstraints, CertificateParams, ExtendedKeyUsagePurpose, IsCa, KeyPair};
    use tokio::io::{AsyncBufReadExt, BufReader};
    let mut params = CertificateParams::new(vec!["Socket.IO Test CA".into()]).unwrap();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "Socket.IO network CA");
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let ca_key = KeyPair::generate().unwrap();
    let ca = params.self_signed(&ca_key).unwrap();
    let issuer = rcgen::Issuer::from_params(&params, &ca_key);
    let mut params = CertificateParams::new(vec!["socketio-network.test".into()]).unwrap();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "socketio-network.test");
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    let server_key = KeyPair::generate().unwrap();
    let cert = params.signed_by(&server_key, &issuer).unwrap();
    let mut params = CertificateParams::new(vec!["client.test".into()]).unwrap();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "socketio-client");
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
    let client_key = KeyPair::generate().unwrap();
    let client_cert = params.signed_by(&client_key, &issuer).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let cert_path = dir.path().join("cert.pem");
    let key_path = dir.path().join("key.pem");
    let ca_path = dir.path().join("ca.pem");
    std::fs::write(&cert_path, cert.pem()).unwrap();
    std::fs::write(&key_path, server_key.serialize_pem()).unwrap();
    std::fs::write(&ca_path, ca.pem()).unwrap();
    let mut server = tokio::process::Command::new("node")
        .arg(format!(
            "{}/tests/fixtures/socketio/server.cjs",
            env!("CARGO_MANIFEST_DIR")
        ))
        .env("TLS_CERT", cert_path)
        .env("TLS_KEY", key_path)
        .env("TLS_CA", ca_path)
        .env("TLS_REQUIRE_CLIENT", "1")
        .env("TLS_HOST", "socketio-network.test")
        .env("PORT", "0")
        .stdout(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(server.stdout.take().unwrap()).lines();
    let ready = tokio::time::timeout(Duration::from_secs(5), lines.next_line())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let port = ready.strip_prefix("ready:").unwrap();
    let mut r = request(&format!("wss://socketio-network.test:{port}"));
    r.network = Some(Box::new(moleapi_core::RequestNetwork {
        built_in_roots: false,
        ca_pem: ca.pem(),
        dns: vec![moleapi_core::DnsOverride {
            hostname: "socketio-network.test".into(),
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
    let mut store = p12_keystore::KeyStore::new();
    store.add_entry(
        "socketio-client",
        p12_keystore::KeyStoreEntry::PrivateKeyChain(p12_keystore::PrivateKeyChain::new(
            &[1u8, 2, 3][..],
            p12_keystore::PrivateKey::from_der(&client_key.serialize_der()).unwrap(),
            [p12_keystore::Certificate::from_der(client_cert.der()).unwrap()],
        )),
    );
    use base64::Engine;
    let pfx = base64::engine::general_purpose::STANDARD
        .encode(store.writer("socketio-private").write().unwrap());
    for pfx_mode in [false, true] {
        if pfx_mode {
            r.network.as_mut().unwrap().identity = moleapi_core::ClientIdentity {
                enabled: true,
                format: moleapi_core::IdentityFormat::Pkcs12,
                pkcs12_base64: pfx.clone(),
                password: "socketio-private".into(),
                alias: "socketio-client".into(),
                ..Default::default()
            };
        }
        let manager = SessionManager::new();
        let id = start(&manager, r.clone(), true);
        state(&manager, &id, SessionState::Open).await;
        manager
            .send(
                "owner",
                &id,
                emit(
                    "echo",
                    r#"["network-ok",42]"#,
                    &[],
                    Some("network-ack"),
                    1000,
                ),
            )
            .unwrap();
        let received = event(
            &manager,
            &id,
            |e| matches!(e,EventMessage::SocketioEvent {event,..} if event=="echo"),
        )
        .await;
        let EventMessage::SocketioEvent { arguments, .. } = received else {
            unreachable!()
        };
        assert_eq!(
            arguments,
            json!(["network-ok", 42]).as_array().unwrap().clone()
        );
        manager.close("owner", &id).await.unwrap();
    }
    server.kill().await.unwrap();
    server.wait().await.unwrap();
}
#[tokio::test]
#[ignore = "official Socket.IO 4 fixture"]
async fn socketio_network_connect_proxy_preserves_host_and_isolates_proxy_credentials() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let endpoint = url::Url::parse(&fixture()).unwrap();
    let port = endpoint.port().unwrap();
    let proxy = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_url = format!("http://{}", proxy.local_addr().unwrap());
    let tunnel = tokio::spawn(async move {
        let (mut socket, _) = proxy.accept().await.unwrap();
        let mut header = Vec::new();
        while !header.ends_with(b"\r\n\r\n") {
            header.push(socket.read_u8().await.unwrap());
            assert!(header.len() < 8192);
        }
        let header = String::from_utf8(header).unwrap();
        assert!(header.starts_with(&format!("CONNECT 127.0.0.1:{port} HTTP/1.1")));
        assert!(
            header.to_ascii_lowercase().contains(
                "proxy-authorization: basic dXNlcjpwYXNz"
                    .to_ascii_lowercase()
                    .as_str()
            )
        );
        let mut target = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        socket
            .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
            .await
            .unwrap();
        let _ = tokio::io::copy_bidirectional(&mut socket, &mut target).await;
    });
    let mut r = request(&format!("ws://socketio-network.test:{port}"));
    let moleapi_core::Protocol::Socketio { listeners, .. } = &mut r.protocol else {
        unreachable!()
    };
    listeners.push("network-info".into());
    r.network = Some(Box::new(moleapi_core::RequestNetwork {
        proxy: moleapi_core::RequestProxy {
            enabled: true,
            url: proxy_url,
            username: "user".into(),
            password: "pass".into(),
            ..Default::default()
        },
        dns: vec![moleapi_core::DnsOverride {
            hostname: "socketio-network.test".into(),
            addresses: vec!["127.0.0.1".into()],
        }],
        ..Default::default()
    }));
    let manager = SessionManager::new();
    let id = start(&manager, r.clone(), true);
    state(&manager, &id, SessionState::Open).await;
    manager
        .send("owner", &id, emit("network-info", "[]", &[], None, 1000))
        .unwrap();
    let e = event(
        &manager,
        &id,
        |e| matches!(e,EventMessage::SocketioEvent {event,..} if event=="network-info"),
    )
    .await;
    let EventMessage::SocketioEvent { arguments, .. } = e else {
        unreachable!()
    };
    assert_eq!(
        arguments[0]["host"],
        format!("socketio-network.test:{port}")
    );
    assert!(arguments[0]["proxyAuthorization"].is_null());
    manager.close("owner", &id).await.unwrap();
    tunnel.abort();
    let denied = start(&manager, r, false);
    state(&manager, &denied, SessionState::Error).await;
}
