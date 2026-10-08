#[path = "support/grpc.rs"]
mod fixture;
#[path = "support/grpc_network.rs"]
mod network_fixture;
use moleapi_core::{
    Environment, NetworkPolicy, RequestSpec, Specification, grpc_method, protobuf_pool,
};
use moleapi_protocols::{
    EventMessage, PreparedFeedback, SendMessage, SessionManager, SessionState,
};
use std::{sync::Arc, time::Duration};
fn request(url: &str, method: &str, text: &str) -> RequestSpec {
    serde_json::from_value(serde_json::json!({"protocol":{"kind":"grpc","service":"moleapi.fixture.EchoService","method":method,"message_source":serde_json::json!({"text":text,"count":"9223372036854775807","blob":"AP8=","mode":"SPECIAL","label":"fixture"}).to_string()},"specification_id":"proto","id":"r","name":"gRPC fixture","method":"POST","url":url,"description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":3000,"follow_redirects":false,"verify_tls":true,"assertions":[],"examples":[]})).unwrap()
}
fn method(request: &RequestSpec) -> prost_reflect::MethodDescriptor {
    use base64::Engine;
    let spec = Specification { id:"proto".into(),name:"fixture".into(),kind:"protobuf".into(),source:serde_json::json!({"kind":"descriptor","descriptor_set_base64":base64::engine::general_purpose::STANDARD.encode(fixture::DESCRIPTORS)}).to_string(),dialect:"descriptor-set".into() };
    grpc_method(&protobuf_pool(&spec).unwrap(), request).unwrap()
}
fn environment() -> Environment {
    Environment {
        id: "e".into(),
        name: "e".into(),
        variables: vec![
            serde_json::from_value(
                serde_json::json!({"id":"v","key":"next","value":"second","enabled":true}),
            )
            .unwrap(),
        ],
    }
}
fn start(
    manager: &SessionManager,
    request: RequestSpec,
    mask: Arc<dyn Fn(&str) -> String + Send + Sync>,
) -> String {
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
        .configure_grpc(
            "owner",
            &summary.id,
            method(&request),
            environment(),
            mask.clone(),
        )
        .unwrap();
    manager
        .start(
            "owner",
            &summary.id,
            request,
            NetworkPolicy {
                allow_private_network: true,
            },
            mask,
        )
        .unwrap();
    summary.id
}
async fn terminal(manager: &SessionManager, id: &str) -> Vec<moleapi_protocols::SessionEvent> {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let summary = manager.summary("owner", id).unwrap();
            if !summary.state.live() {
                assert_eq!(summary.state, SessionState::Closed, "{:?}", summary.reason);
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    manager.events("owner", id, 0).unwrap().events
}
fn messages(events: &[moleapi_protocols::SessionEvent]) -> Vec<serde_json::Value> {
    events
        .iter()
        .filter_map(|event| match &event.message {
            EventMessage::GrpcMessage { message } if event.direction == "incoming" => {
                Some(message.clone())
            }
            _ => None,
        })
        .collect()
}
#[tokio::test]
async fn actual_tonic_all_four_modes_json_headers_trailers_and_half_close() {
    let (url, fixture) = fixture::start(false).await;
    for mode in ["Unary", "ServerStream", "ClientStream", "Bidi"] {
        let manager = SessionManager::new();
        let id = start(
            &manager,
            request(&url, mode, "first"),
            Arc::new(str::to_owned),
        );
        if matches!(mode, "ClientStream" | "Bidi") {
            // Valid even during Connecting: stream commands remain ordered and bounded.
            manager
                .send(
                    "owner",
                    &id,
                    SendMessage::GrpcMessage {
                        message_source: r#"{"text":"{{next}}"}"#.into(),
                    },
                )
                .unwrap();
            manager
                .send("owner", &id, SendMessage::GrpcHalfClose)
                .unwrap();
            assert!(
                manager
                    .send(
                        "owner",
                        &id,
                        SendMessage::GrpcMessage {
                            message_source: "{}".into()
                        }
                    )
                    .unwrap_err()
                    .to_string()
                    .contains("half-closed")
            );
        } else {
            assert!(
                manager
                    .send(
                        "owner",
                        &id,
                        SendMessage::GrpcMessage {
                            message_source: "{}".into()
                        }
                    )
                    .is_err()
            );
        }
        let events = terminal(&manager, &id).await;
        let incoming = messages(&events);
        assert_eq!(
            incoming.len(),
            match mode {
                "ServerStream" => 3,
                "Bidi" => 2,
                _ => 1,
            },
            "{mode}"
        );
        if mode == "ClientStream" {
            assert_eq!(incoming[0]["text"], "firstsecond");
            assert_eq!(incoming[0]["count"], "2");
        } else {
            assert_eq!(incoming[0]["text"], "first");
            assert_eq!(incoming[0]["blob"], "AP8=");
        }
        assert!(events.iter().any(|event| matches!(&event.message, EventMessage::GrpcMetadata {phase,metadata} if phase=="headers" && metadata.iter().any(|p|p.key=="x-fixture" && p.value=="tonic") && metadata.iter().any(|p|p.key=="x-fixture-bin" && p.value=="AP8="))));
        assert!(
            events
                .iter()
                .any(|event| matches!(&event.message, EventMessage::GrpcStatus { code: 0, .. }))
        );
        if mode == "ServerStream" {
            assert!(events.iter().any(|event| matches!(&event.message, EventMessage::GrpcMetadata {phase,metadata} if phase=="trailers" && metadata.iter().any(|p|p.key=="x-trailer" && p.value=="completed"))));
        }
        assert!(manager.summary("owner", &id).unwrap().handshake.is_none());
        assert!(manager.summary("intruder", &id).is_err());
        assert!(
            manager
                .send("intruder", &id, SendMessage::GrpcHalfClose)
                .is_err()
        );
    }
    fixture.abort();
}
#[tokio::test]
async fn actual_tonic_status_details_auth_deadline_cancellation_and_privacy() {
    let (url, fixture) = fixture::start(false).await;
    let manager = SessionManager::new();
    let id = start(
        &manager,
        request(&url, "Unary", "error"),
        Arc::new(str::to_owned),
    );
    let events = terminal(&manager, &id).await;
    assert!(events.iter().any(|event| matches!(&event.message, EventMessage::GrpcStatus {code:7,name,message,details_base64,metadata} if name=="PermissionDenied" && message.contains("rejected") && details_base64=="AP8q" && metadata.iter().any(|p|p.key=="x-trailer"))));
    manager.remove("owner", &id).await.unwrap();
    let mut auth = request(&url, "Unary", "secret-value");
    auth.headers.push(
        serde_json::from_value(
            serde_json::json!({"id":"h","key":"x-require-auth","value":"yes","enabled":true}),
        )
        .unwrap(),
    );
    let id = start(&manager, auth.clone(), Arc::new(str::to_owned));
    assert!(
        terminal(&manager, &id)
            .await
            .iter()
            .any(|event| matches!(&event.message, EventMessage::GrpcStatus { code: 16, .. }))
    );
    manager.remove("owner", &id).await.unwrap();
    auth.auth.kind = "bearer".into();
    auth.auth.token = "fixture-secret".into();
    let id = start(
        &manager,
        auth,
        Arc::new(|text: &str| text.replace("secret-value", "[REDACTED]")),
    );
    let events = terminal(&manager, &id).await;
    assert!(
        !serde_json::to_string(&events)
            .unwrap()
            .contains("secret-value")
    );
    assert_eq!(messages(&events)[0], "[REDACTED]");
    manager.remove("owner", &id).await.unwrap();
    let mut deadline = request(&url, "Unary", "wait");
    deadline.timeout_ms = 60;
    let id = start(&manager, deadline, Arc::new(str::to_owned));
    let deadline_events = terminal(&manager, &id).await;
    assert!(
        deadline_events
            .iter()
            .any(|event| matches!(&event.message, EventMessage::GrpcStatus { code: 4, .. })),
        "{deadline_events:?}"
    );
    manager.remove("owner", &id).await.unwrap();
    let id = start(
        &manager,
        request(&url, "ServerStream", "wait"),
        Arc::new(str::to_owned),
    );
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if !messages(&manager.events("owner", &id, 0).unwrap().events).is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    manager.close("owner", &id).await.unwrap();
    assert!(
        terminal(&manager, &id)
            .await
            .iter()
            .any(|event| matches!(&event.message, EventMessage::GrpcStatus { code: 1, .. }))
    );
    fixture.abort();
}
#[tokio::test]
async fn actual_reflection_v1_and_v1alpha_fallback_and_pinned_network_policy() {
    for alpha_only in [false, true] {
        let (url, fixture) = fixture::start(alpha_only).await;
        let request = request(&url, "Unary", "hello");
        let reflected = moleapi_protocols::reflect(
            &request,
            NetworkPolicy {
                allow_private_network: true,
            },
        )
        .await
        .unwrap();
        let schema = reflected.schema.unwrap();
        assert_eq!(
            schema
                .services
                .iter()
                .find(|s| s.name == "moleapi.fixture.EchoService")
                .unwrap()
                .methods
                .len(),
            4
        );
        assert_eq!(
            protobuf_pool(&reflected.specification.unwrap())
                .unwrap()
                .services()
                .filter(|s| s.full_name() == "moleapi.fixture.EchoService")
                .count(),
            1
        );
        assert!(
            moleapi_protocols::reflect(
                &request,
                NetworkPolicy {
                    allow_private_network: false
                }
            )
            .await
            .unwrap_err()
            .to_string()
            .contains("Private")
        );
        fixture.abort();
    }
}
#[tokio::test]
async fn mature_tls_connector_certificate_verification_optout_is_explicit_and_functional() {
    let (url, fixture) = fixture::start_tls().await;
    let manager = SessionManager::new();
    let request = request(&url, "Unary", "TLS echo");
    let id = start(&manager, request.clone(), Arc::new(str::to_owned));
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if !manager.summary("owner", &id).unwrap().state.live() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        manager.summary("owner", &id).unwrap().state,
        SessionState::Error,
        "untrusted self-signed certificate must fail when verification is enabled"
    );
    manager.remove("owner", &id).await.unwrap();
    let mut unverified = request;
    unverified.verify_tls = false;
    let id = start(&manager, unverified, Arc::new(str::to_owned));
    let events = terminal(&manager, &id).await;
    assert_eq!(messages(&events)[0]["text"], "TLS echo");
    fixture.abort();
}
#[tokio::test]
async fn reflection_uses_shared_live_admission_and_releases_capacity_on_drop() {
    let manager = SessionManager::new();
    let request = request("http://127.0.0.1:1", "Unary", "hello");
    let mut leases = Vec::new();
    for _ in 0..4 {
        leases.push(
            manager
                .reserve_reflection("owner", "w", &request, "safe".into())
                .unwrap(),
        );
    }
    assert!(
        manager
            .reserve_reflection("owner", "w", &request, "safe".into())
            .is_err()
    );
    manager.close_owner("owner").await;
    for lease in &leases {
        tokio::time::timeout(Duration::from_millis(100), lease.cancelled())
            .await
            .unwrap();
    }
    drop(leases);
    assert!(
        manager
            .reserve_reflection("owner", "w", &request, "safe".into())
            .is_ok()
    );
}

#[tokio::test]
async fn grpc_network_custom_ca_dns_and_mutual_tls_use_pem_and_encrypted_pfx() {
    use moleapi_core::{ClientIdentity, IdentityFormat};
    let (url, network, pfx, server) = network_fixture::start_mutual_tls_network().await;
    let mut r = request(&url, "Unary", "network-pem");
    r.network = Some(Box::new(network));
    moleapi_core::validate_request(&r, false).unwrap();
    let manager = SessionManager::new();
    let id = start(&manager, r.clone(), Arc::new(str::to_owned));
    let events = terminal(&manager, &id).await;
    assert_eq!(messages(&events)[0]["text"], "network-pem");
    r.network.as_mut().unwrap().identity = ClientIdentity {
        enabled: true,
        format: IdentityFormat::Pkcs12,
        pkcs12_base64: pfx,
        password: "test-private-password".into(),
        alias: "grpc-client".into(),
        ..Default::default()
    };
    let id = start(&manager, r, Arc::new(str::to_owned));
    let events = terminal(&manager, &id).await;
    assert_eq!(messages(&events)[0]["text"], "network-pem");
    server.abort();
}
#[tokio::test]
async fn grpc_network_connect_proxy_retains_dns_target_and_http2_calls() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let (url, server) = fixture::start(false).await;
    let target = url::Url::parse(&url).unwrap();
    let port = target.port().unwrap();
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
    let mut r = request(
        &format!("http://grpc-network.test:{port}"),
        "Unary",
        "through-proxy",
    );
    r.network = Some(Box::new(moleapi_core::RequestNetwork {
        http_mode: moleapi_core::HttpMode::Auto,
        proxy: moleapi_core::RequestProxy {
            enabled: true,
            url: proxy_url,
            username: "user".into(),
            password: "pass".into(),
            ..Default::default()
        },
        dns: vec![moleapi_core::DnsOverride {
            hostname: "grpc-network.test".into(),
            addresses: vec!["127.0.0.1".into()],
        }],
        ..Default::default()
    }));
    let manager = SessionManager::new();
    let id = start(&manager, r, Arc::new(str::to_owned));
    let events = terminal(&manager, &id).await;
    assert_eq!(messages(&events)[0]["text"], "through-proxy");
    tunnel.abort();
    server.abort();
}
#[tokio::test]
async fn grpc_network_reflection_uses_dns_override_and_rejects_private_hosted_addresses() {
    let (url, server) = fixture::start(false).await;
    let port = url::Url::parse(&url).unwrap().port().unwrap();
    let mut r = request(
        &format!("http://grpc-network.test:{port}"),
        "Unary",
        "reflection",
    );
    r.network = Some(Box::new(moleapi_core::RequestNetwork {
        http_mode: moleapi_core::HttpMode::Auto,
        dns: vec![moleapi_core::DnsOverride {
            hostname: "grpc-network.test".into(),
            addresses: vec!["127.0.0.1".into()],
        }],
        ..Default::default()
    }));
    let reflected = moleapi_protocols::reflect(
        &r,
        NetworkPolicy {
            allow_private_network: true,
        },
    )
    .await
    .unwrap();
    assert!(
        reflected
            .schema
            .unwrap()
            .services
            .iter()
            .any(|s| s.name == "moleapi.fixture.EchoService")
    );
    assert!(
        moleapi_protocols::reflect(
            &r,
            NetworkPolicy {
                allow_private_network: false
            }
        )
        .await
        .is_err()
    );
    r.network.as_mut().unwrap().http_mode = moleapi_core::HttpMode::Http1;
    assert!(
        moleapi_core::validate_request(&r, false)
            .unwrap_err()
            .to_string()
            .contains("HTTP/2")
    );
    server.abort();
}
