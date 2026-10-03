use base64::{Engine, engine::general_purpose::STANDARD};
use moleapi_core::{
    Environment, MqttConfig, MqttMessage, MqttSubscription, MqttWill, NetworkPolicy, Protocol,
    RequestSpec,
};
use moleapi_protocols::{
    EventMessage, PreparedFeedback, SendMessage, SessionManager, SessionState,
};
use serde_json::json;
use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::Arc,
    time::Duration,
};
struct Broker {
    child: Child,
    directory: tempfile::TempDir,
    port: u16,
    binary: PathBuf,
}
impl Broker {
    fn new(tls: bool, auth: bool) -> Self {
        Self::new_transport(tls, auth, false)
    }
    fn new_transport(tls: bool, auth: bool, websocket: bool) -> Self {
        let binary = PathBuf::from(
            std::env::var("MOLEAPI_MOSQUITTO_BIN")
                .expect("set MOLEAPI_MOSQUITTO_BIN to isolated mature broker"),
        );
        let directory = tempfile::tempdir().unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(port >= 18891);
        drop(listener);
        let mut config = format!(
            "listener {port} 127.0.0.1\nallow_anonymous {}\npersistence false\n",
            !auth
        );
        if websocket {
            config += "protocol websockets\n";
        }
        if auth {
            let password = directory.path().join("password");
            let bin = std::env::var("MOLEAPI_MOSQUITTO_PASSWD_BIN")
                .map(PathBuf::from)
                .unwrap_or_else(|_| binary.with_file_name("mosquitto_passwd"));
            assert!(
                Command::new(bin)
                    .args(["-b", "-c"])
                    .arg(&password)
                    .args(["fixture-user", "fixture-password"])
                    .status()
                    .unwrap()
                    .success()
            );
            config += &format!("password_file {}\n", password.display());
        }
        if tls {
            let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
            let ca = directory.path().join("cert.pem");
            let key = directory.path().join("key.pem");
            std::fs::write(&ca, cert.cert.pem()).unwrap();
            std::fs::write(&key, cert.signing_key.serialize_pem()).unwrap();
            config += &format!("certfile {}\nkeyfile {}\n", ca.display(), key.display());
        }
        std::fs::write(directory.path().join("broker.conf"), config).unwrap();
        let child = Command::new(&binary)
            .arg("-c")
            .arg(directory.path().join("broker.conf"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut broker = Self {
            child,
            directory,
            port,
            binary,
        };
        broker.ready();
        broker
    }
    fn ready(&mut self) {
        for _ in 0..100 {
            assert!(self.child.try_wait().unwrap().is_none(), "broker exited");
            if std::net::TcpStream::connect(("127.0.0.1", self.port)).is_ok() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("broker not ready");
    }
    fn url(&self, tls: bool) -> String {
        format!(
            "{}://localhost:{}",
            if tls { "mqtts" } else { "mqtt" },
            self.port
        )
    }
    fn stop(&mut self) {
        self.child.kill().unwrap();
        self.child.wait().unwrap();
    }
    fn restart(&mut self) {
        self.child = Command::new(&self.binary)
            .arg("-c")
            .arg(self.directory.path().join("broker.conf"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        self.ready();
    }
}
impl Drop for Broker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn request(url: &str, version: &str) -> RequestSpec {
    let mut r:RequestSpec=serde_json::from_value(json!({"protocol":{"kind":"mqtt"},"id":"r","name":"MQTT fixture","method":"GET","url":url,"description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":false,"verify_tls":true,"assertions":[],"examples":[]})).unwrap();
    config(&mut r).version = version.into();
    r
}
fn config(r: &mut RequestSpec) -> &mut MqttConfig {
    let Protocol::Mqtt { config } = &mut r.protocol else {
        unreachable!()
    };
    config
}
fn env() -> Environment {
    serde_json::from_value(json!({"id":"e","name":"scope","variables":[{"id":"selected","key":"selected","value":"original selected scope","enabled":true},{"id":"private","key":"private","value":"private-secret","enabled":true}]})).unwrap()
}
fn start(m: &SessionManager, r: RequestSpec, private: bool) -> String {
    let r = moleapi_core::resolve_request(&r, Some(&env())).unwrap();
    let mask: Arc<dyn Fn(&str) -> String + Send + Sync> = Arc::new(|s| {
        s.replace("private-secret", "[REDACTED]")
            .replace("fixture-password", "[REDACTED]")
    });
    let summary = m
        .register("owner", "w", &r, "safe".into(), PreparedFeedback::default())
        .unwrap();
    let Protocol::Mqtt { config } = &r.protocol else {
        unreachable!()
    };
    m.configure_mqtt("owner", &summary.id, *config.clone(), env(), mask.clone())
        .unwrap();
    m.start(
        "owner",
        &summary.id,
        r,
        NetworkPolicy {
            allow_private_network: private,
        },
        mask,
    )
    .unwrap();
    summary.id
}
async fn state(m: &SessionManager, id: &str, target: SessionState) {
    tokio::time::timeout(Duration::from_secs(6), async {
        loop {
            let summary = m.summary("owner", id).unwrap();
            if summary.state == target {
                return;
            }
            assert!(summary.state.live(), "{summary:?}");
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}
async fn event(
    m: &SessionManager,
    id: &str,
    after: u64,
    p: impl Fn(&EventMessage, &str) -> bool,
) -> EventMessage {
    tokio::time::timeout(Duration::from_secs(6), async {
        loop {
            for e in m.events("owner", id, after).unwrap().events {
                if p(&e.message, &e.direction) {
                    return e.message;
                }
            }
            let summary = m.summary("owner", id).unwrap();
            assert!(summary.state.live(), "{summary:?}");
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap()
}
fn message(topic: &str, text: &str, qos: u8, retain: bool) -> MqttMessage {
    MqttMessage {
        topic: topic.into(),
        payload_source: text.into(),
        qos,
        retain,
        ..Default::default()
    }
}
fn publish(m: &SessionManager, id: &str, message: MqttMessage) {
    m.send("owner", id, SendMessage::MqttPublish { message })
        .unwrap();
}
async fn sub(m: &SessionManager, id: &str, filter: &str) {
    let after = m.events("owner", id, 0).unwrap().next_cursor;
    m.send(
        "owner",
        id,
        SendMessage::MqttSubscribe {
            subscription: MqttSubscription {
                filter: filter.into(),
                qos: 2,
                ..Default::default()
            },
        },
    )
    .unwrap();
    event(m,id,after,|e,_|matches!(e,EventMessage::MqttStatus{operation,status,..} if operation=="subscribe"&&status=="acknowledged")).await;
}
#[tokio::test]
#[ignore = "requires isolated Mosquitto 2.x broker binary"]
async fn mature_broker_both_versions_qos_wildcards_binary_json_retained_clear_unsubscribe() {
    let broker = Broker::new(false, false);
    for version in ["3.1.1", "5"] {
        let m = SessionManager::new();
        let id = start(&m, request(&broker.url(false), version), true);
        state(&m, &id, SessionState::Open).await;
        sub(&m, &id, "moleapi/+").await;
        for qos in 0..=2 {
            let after = m.events("owner", &id, 0).unwrap().next_cursor;
            publish(
                &m,
                &id,
                message(
                    "moleapi/json",
                    r#"{"nested":{"number":18446744073709551616000},"scope":"{{selected}}"}"#,
                    qos,
                    false,
                ),
            );
            let received = event(&m, &id, after, |e, d| {
                d == "incoming"
                    && matches!(e,EventMessage::MqttMessage{topic,..} if topic=="moleapi/json")
            })
            .await;
            let EventMessage::MqttMessage {
                payload_text,
                qos: received_qos,
                ..
            } = received
            else {
                unreachable!()
            };
            assert_eq!(received_qos, qos);
            assert_eq!(
                payload_text.unwrap(),
                r#"{"nested":{"number":18446744073709551616000},"scope":"original selected scope"}"#
            );
            if qos > 0 {
                let ack=event(&m,&id,after,|e,_|matches!(e,EventMessage::MqttStatus{operation,status,packet_id:Some(id),..} if operation=="publish"&&status=="acknowledged"&&*id>0)).await;
                assert!(matches!(ack, EventMessage::MqttStatus { .. }));
            }
        }
        let after = m.events("owner", &id, 0).unwrap().next_cursor;
        let bytes = vec![0, 255, 128, 42, 0];
        let mut binary = message("moleapi/binary", &STANDARD.encode(&bytes), 2, true);
        binary.encoding = "base64".into();
        publish(&m, &id, binary.clone());
        let e = event(&m, &id, after, |e, d| {
            d == "incoming"
                && matches!(e,EventMessage::MqttMessage{topic,..} if topic=="moleapi/binary")
        })
        .await;
        let EventMessage::MqttMessage {
            payload_base64,
            payload_text,
            ..
        } = e
        else {
            unreachable!()
        };
        assert_eq!(STANDARD.decode(payload_base64).unwrap(), bytes);
        assert!(payload_text.is_none());
        let second = start(&m, request(&broker.url(false), version), true);
        state(&m, &second, SessionState::Open).await;
        sub(&m, &second, "moleapi/binary").await;
        event(&m, &second, 0, |e, d| {
            d == "incoming" && matches!(e, EventMessage::MqttMessage { retain: true, .. })
        })
        .await;
        binary.payload_source = String::new();
        publish(&m, &id, binary);
        let clear=event(&m,&second,m.events("owner",&second,0).unwrap().next_cursor,|e,d|d=="incoming"&&matches!(e,EventMessage::MqttMessage{payload_base64,..} if payload_base64.is_empty())).await;
        assert!(matches!(clear, EventMessage::MqttMessage { .. }));
        m.close("owner", &second).await.unwrap();
        let third = start(&m, request(&broker.url(false), version), true);
        state(&m, &third, SessionState::Open).await;
        sub(&m, &third, "moleapi/binary").await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(
            !m.events("owner", &third, 0)
                .unwrap()
                .events
                .iter()
                .any(|e| matches!(e.message, EventMessage::MqttMessage { .. }))
        );
        let after = m.events("owner", &id, 0).unwrap().next_cursor;
        m.send(
            "owner",
            &id,
            SendMessage::MqttUnsubscribe {
                filter: "moleapi/+".into(),
            },
        )
        .unwrap();
        event(&m,&id,after,|e,_|matches!(e,EventMessage::MqttStatus{operation,status,..} if operation=="unsubscribe"&&status=="acknowledged")).await;
        let after = m.events("owner", &id, 0).unwrap().next_cursor;
        publish(&m, &id, message("moleapi/unsubscribed", "gone", 1, false));
        event(&m,&id,after,|e,_|matches!(e,EventMessage::MqttStatus{operation,status,..} if operation=="publish"&&status=="acknowledged")).await;
        assert!(
            !m.events("owner", &id, after)
                .unwrap()
                .events
                .iter()
                .any(|e| e.direction == "incoming"
                    && matches!(e.message, EventMessage::MqttMessage { .. }))
        );
        m.close_owner("owner").await;
        assert_eq!(m.summary("owner", &id).unwrap().state, SessionState::Closed);
    }
}
#[tokio::test]
#[ignore = "requires isolated Mosquitto 2.x broker binary"]
async fn v5_properties_will_ungraceful_versus_graceful_and_private_bytes_flags() {
    let broker = Broker::new(false, false);
    let m = SessionManager::new();
    let observer = start(&m, request(&broker.url(false), "5"), true);
    state(&m, &observer, SessionState::Open).await;
    let after = m.events("owner", &observer, 0).unwrap().next_cursor;
    m.send(
        "owner",
        &observer,
        SendMessage::MqttSubscribe {
            subscription: MqttSubscription {
                filter: "moleapi/#".into(),
                qos: 2,
                subscription_identifier: Some(19),
                retain_as_published: true,
                user_properties: vec![moleapi_core::MqttProperty {
                    key: "sub".into(),
                    value: "value".into(),
                    secret: false,
                }],
                ..Default::default()
            },
        },
    )
    .unwrap();
    event(&m,&observer,after,|e,_|matches!(e,EventMessage::MqttStatus{operation,status,..} if operation=="subscribe"&&status=="acknowledged")).await;
    let mut msg = message("moleapi/properties", "props", 1, false);
    msg.properties.content_type = Some("text/plain".into());
    msg.properties.response_topic = Some("moleapi/reply".into());
    msg.properties.correlation_data_base64 = Some(STANDARD.encode([0, 255]));
    msg.properties.payload_format_indicator = Some(1);
    msg.properties.message_expiry_interval = Some(60);
    msg.properties
        .user_properties
        .push(moleapi_core::MqttProperty {
            key: "trace".into(),
            value: "actual-v5".into(),
            secret: false,
        });
    publish(&m, &observer, msg);
    let e = event(&m, &observer, after, |e, d| {
        d == "incoming"
            && matches!(e,EventMessage::MqttMessage{topic,..} if topic=="moleapi/properties")
    })
    .await;
    let EventMessage::MqttMessage { properties, .. } = e else {
        unreachable!()
    };
    assert_eq!(properties["content_type"], "text/plain");
    assert_eq!(
        properties["correlation_data_base64"],
        STANDARD.encode([0, 255])
    );
    assert_eq!(properties["subscription_identifiers"], json!([19]));
    assert_eq!(properties["user_properties"][0]["value"], "actual-v5");
    let mut r = request(&broker.url(false), "5");
    config(&mut r).session_expiry_interval = 30;
    config(&mut r).will = Some(MqttWill {
        message: message("moleapi/will", "broker will", 1, false),
        delay_interval: 1,
    });
    let id = start(&m, r.clone(), true);
    state(&m, &id, SessionState::Open).await;
    let after = m.events("owner", &observer, 0).unwrap().next_cursor;
    m.send("owner", &id, SendMessage::MqttAbort).unwrap();
    state(&m, &id, SessionState::Closed).await;
    event(&m, &observer, after, |e, d| {
        d == "incoming" && matches!(e,EventMessage::MqttMessage{topic,..} if topic=="moleapi/will")
    })
    .await;
    let id = start(&m, r, true);
    state(&m, &id, SessionState::Open).await;
    let after = m.events("owner", &observer, 0).unwrap().next_cursor;
    let closed = m.close("owner", &id).await.unwrap();
    assert!(closed.reason.unwrap().contains("gracefully"));
    tokio::time::sleep(Duration::from_millis(1300)).await;
    assert!(!m.events("owner",&observer,after).unwrap().events.iter().any(|e|matches!(&e.message,EventMessage::MqttMessage{topic,..} if topic=="moleapi/will")));
    let mut secret = message(
        "moleapi/private-secret",
        "AP9iaW5hcnktc2VjcmV0AA==",
        2,
        false,
    );
    secret.encoding = "base64".into();
    secret.payload_secret = true;
    secret.topic_secret = true;
    secret
        .properties
        .user_properties
        .push(moleapi_core::MqttProperty {
            key: "private".into(),
            value: "explicit-property-secret".into(),
            secret: true,
        });
    let after = m.events("owner", &observer, 0).unwrap().next_cursor;
    publish(&m, &observer, secret);
    let e = event(&m, &observer, after, |e, d| {
        d == "incoming"
            && matches!(
                e,
                EventMessage::MqttMessage {
                    payload_redacted: true,
                    ..
                }
            )
    })
    .await;
    let EventMessage::MqttMessage {
        topic_redacted,
        properties_redacted,
        payload_base64,
        payload_text,
        ..
    } = e
    else {
        unreachable!()
    };
    assert!(topic_redacted && properties_redacted);
    assert!(payload_base64.is_empty() && payload_text.is_none());
    assert!(
        !serde_json::to_string(&m.events("owner", &observer, after).unwrap())
            .unwrap()
            .contains("explicit-property-secret")
    );
    m.close_owner("owner").await;
}
#[tokio::test]
#[ignore = "requires isolated Mosquitto 2.x broker binary and password tool"]
async fn basic_auth_tls_verification_policy_original_hostname_and_cancel() {
    let broker = Broker::new(true, true);
    let m = SessionManager::new();
    for version in ["3.1.1", "5"] {
        let mut r = request(&broker.url(true), version);
        r.auth.kind = "basic".into();
        r.auth.username = "fixture-user".into();
        r.auth.password = "fixture-password".into();
        let id = start(&m, r.clone(), true);
        state(&m, &id, SessionState::Error).await;
        assert!(
            m.summary("owner", &id)
                .unwrap()
                .reason
                .unwrap()
                .to_ascii_lowercase()
                .contains("cert")
        );
        r.verify_tls = false;
        let id = start(&m, r.clone(), false);
        state(&m, &id, SessionState::Error).await;
        assert!(
            m.summary("owner", &id)
                .unwrap()
                .reason
                .unwrap()
                .contains("blocked")
        );
        let id = start(&m, r.clone(), true);
        state(&m, &id, SessionState::Open).await;
        m.close("owner", &id).await.unwrap();
        r.auth.password = "wrong".into();
        let id = start(&m, r, true);
        state(&m, &id, SessionState::Error).await;
        assert!(
            !m.summary("owner", &id)
                .unwrap()
                .reason
                .unwrap()
                .contains("fixture-password")
        );
    }
    let r = request("mqtt://127.0.0.1:18899", "5");
    let id = start(&m, r, true);
    m.close("owner", &id).await.unwrap();
    assert_eq!(m.summary("owner", &id).unwrap().state, SessionState::Closed);
    m.close_owner("owner").await;
}
#[tokio::test]
#[ignore = "requires isolated Mosquitto 2.x broker binary"]
async fn broker_loss_bounded_reconnect_subscription_restore_and_cancel() {
    let mut broker = Broker::new(false, false);
    let m = SessionManager::new();
    let mut r = request(&broker.url(false), "5");
    let c = config(&mut r);
    c.reconnect.enabled = true;
    c.reconnect.max_attempts = 5;
    c.reconnect.delay_ms = 100;
    let id = start(&m, r, true);
    state(&m, &id, SessionState::Open).await;
    sub(&m, &id, "moleapi/reconnect").await;
    broker.stop();
    event(&m,&id,0,|e,_|matches!(e,EventMessage::MqttStatus{operation,status,..} if operation=="reconnect"&&status=="waiting")).await;
    broker.restart();
    state(&m, &id, SessionState::Open).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    let after = m.events("owner", &id, 0).unwrap().next_cursor;
    publish(&m, &id, message("moleapi/reconnect", "restored", 1, false));
    event(&m, &id, after, |e, d| {
        d == "incoming"
            && matches!(e,EventMessage::MqttMessage{payload_text:Some(text),..} if text=="restored")
    })
    .await;
    broker.stop();
    event(&m,&id,after,|e,_|matches!(e,EventMessage::MqttStatus{operation,status,..} if operation=="reconnect"&&status=="waiting")).await;
    let before = std::time::Instant::now();
    m.close("owner", &id).await.unwrap();
    assert!(before.elapsed() < Duration::from_secs(1));
    assert_eq!(m.summary("owner", &id).unwrap().state, SessionState::Closed);
    let mut r = request(&broker.url(false), "5");
    config(&mut r).reconnect.enabled = true;
    config(&mut r).reconnect.max_attempts = 2;
    config(&mut r).reconnect.delay_ms = 100;
    let id = start(&m, r, true);
    state(&m, &id, SessionState::Error).await;
    assert_eq!(m.events("owner",&id,0).unwrap().events.iter().filter(|e|matches!(&e.message,EventMessage::MqttStatus{operation,status,..} if operation=="reconnect"&&status=="waiting")).count(),2);
}

#[tokio::test]
#[ignore = "requires isolated Mosquitto 2.x broker with WebSocket support"]
async fn websocket_and_wss_both_versions_properties_query_custom_headers_and_connect_basic() {
    for tls in [false, true] {
        let broker = Broker::new_transport(tls, true, true);
        let m = SessionManager::new();
        for version in ["3.1.1", "5"] {
            let mut r = request(
                &format!(
                    "{}://localhost:{}/mqtt?initial=query",
                    if tls { "wss" } else { "ws" },
                    broker.port
                ),
                version,
            );
            r.verify_tls = false;
            r.auth.kind = "basic".into();
            r.auth.username = "fixture-user".into();
            r.auth.password = "fixture-password".into();
            r.headers.push(moleapi_core::Pair {
                id: "header".into(),
                key: "X-Fixture".into(),
                value: "custom".into(),
                enabled: true,
                secret: None,
                local_value: None,
            });
            r.query.push(moleapi_core::Pair {
                id: "query".into(),
                key: "selected".into(),
                value: "{{selected}}".into(),
                enabled: true,
                secret: None,
                local_value: None,
            });
            let id = start(&m, r, true);
            state(&m, &id, SessionState::Open).await;
            sub(&m, &id, "moleapi/ws").await;
            publish(
                &m,
                &id,
                message("moleapi/ws", "real websocket MQTT", 2, false),
            );
            event(&m,&id,0,|e,d|d=="incoming"&&matches!(e,EventMessage::MqttMessage{payload_text:Some(text),..} if text=="real websocket MQTT")).await;
            m.close("owner", &id).await.unwrap();
        }
    }
}

#[tokio::test]
#[ignore = "requires isolated Mosquitto 2.x broker binary"]
async fn sdk_pinned_dns_without_host_rewrite_and_tls_original_hostname_validation() {
    let broker = Broker::new(false, false);
    let mut options = rumqttc::MqttOptions::new(
        "pinned-no-dns",
        "this-host-must-never-resolve.invalid",
        broker.port,
    );
    options.set_keep_alive(Duration::from_secs(5));
    let (_, mut eventloop) = rumqttc::AsyncClient::new(options, 4);
    let mut network = rumqttc::NetworkOptions::new();
    network.set_pinned_addresses(vec![std::net::SocketAddr::from((
        [127, 0, 0, 1],
        broker.port,
    ))]);
    eventloop.set_network_options(network);
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(2), eventloop.poll())
            .await
            .unwrap()
            .unwrap(),
        rumqttc::Event::Incoming(rumqttc::Packet::ConnAck(_))
    ));
    let mut network = rumqttc::NetworkOptions::new();
    network.set_pinned_addresses(vec![]);
    eventloop.set_network_options(network);
    drop(eventloop);
    let tls = Broker::new(true, false);
    let pem = std::fs::read_to_string(tls.directory.path().join("cert.pem")).unwrap();
    let cert = rustls_pemfile_for_test(&pem);
    let mut roots = rustls::RootCertStore::empty();
    roots.add(cert).unwrap();
    let configuration = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_root_certificates(roots)
    .with_no_client_auth();
    for host in ["localhost", "wrong-original-host.invalid"] {
        let mut options = rumqttc::v5::MqttOptions::new(format!("tls-{host}"), host, tls.port);
        options.set_transport(rumqttc::Transport::Tls(rumqttc::TlsConfiguration::Rustls(
            Arc::new(configuration.clone()),
        )));
        let mut network = rumqttc::NetworkOptions::new();
        network.set_pinned_addresses(vec![std::net::SocketAddr::from(([127, 0, 0, 1], tls.port))]);
        options.set_network_options(network);
        let (_, mut eventloop) = rumqttc::v5::AsyncClient::new(options, 4);
        let result = tokio::time::timeout(Duration::from_secs(2), eventloop.poll())
            .await
            .unwrap();
        if host == "localhost" {
            assert!(result.is_ok(), "{result:?}");
        } else {
            assert!(
                result.is_err(),
                "wrong hostname must fail despite pinned IP"
            );
        }
    }
}
fn rustls_pemfile_for_test(pem: &str) -> rustls::pki_types::CertificateDer<'static> {
    let source = pem
        .lines()
        .filter(|line| !line.starts_with("---"))
        .collect::<String>();
    rustls::pki_types::CertificateDer::from(STANDARD.decode(source).unwrap())
}
#[tokio::test]
#[ignore = "requires isolated Mosquitto 2.x broker binary"]
async fn actual_broker_unsolicited_receive_quota_cannot_reconnect_or_leak_owner() {
    let broker = Broker::new(false, false);
    let m = SessionManager::new();
    let mut r = request(&broker.url(false), "5");
    config(&mut r).reconnect.enabled = true;
    let id = start(&m, r, true);
    state(&m, &id, SessionState::Open).await;
    sub(&m, &id, "moleapi/flood").await;
    assert!(m.events("other-owner", &id, 0).is_err());
    assert!(m.summary("other-owner", &id).is_err());
    let mut options =
        rumqttc::MqttOptions::new("quota-fixture-publisher", "127.0.0.1", broker.port);
    options.set_max_packet_size(1024 * 1024, 1024 * 1024);
    let (client, mut eventloop) = rumqttc::AsyncClient::new(options, 4);
    let pump = tokio::spawn(async move { while eventloop.poll().await.is_ok() {} });
    for _ in 0..45 {
        client
            .publish(
                "moleapi/flood",
                rumqttc::QoS::AtMostOnce,
                false,
                vec![42; 512 * 1024],
            )
            .await
            .unwrap();
    }
    state(&m, &id, SessionState::Error).await;
    let summary = m.summary("owner", &id).unwrap();
    assert!(summary.reason.unwrap().contains("traffic budget"));
    assert!(summary.received_bytes > 20 * 1024 * 1024 && summary.received_bytes < 21 * 1024 * 1024);
    let events = m.events("owner", &id, 0).unwrap();
    assert!(events.dropped_count > 0);
    assert!(!events.events.iter().any(
        |e| matches!(&e.message,EventMessage::MqttStatus{operation,..} if operation=="reconnect")
    ));
    pump.abort();
    m.close_owner("owner").await;
}

#[tokio::test]
#[ignore = "requires isolated Mosquitto 2.x broker binary and password tool"]
async fn v5_actual_acl_publish_rejection_has_broker_reason_not_successful_delivery() {
    let mut broker = Broker::new(false, true);
    broker.stop();
    let acl = broker.directory.path().join("acl");
    std::fs::write(
        &acl,
        "user fixture-user\ntopic read restricted/#\ntopic write allowed/#\n",
    )
    .unwrap();
    let cfg = broker.directory.path().join("broker.conf");
    let mut source = std::fs::read_to_string(&cfg).unwrap();
    source += &format!("acl_file {}\n", acl.display());
    std::fs::write(&cfg, source).unwrap();
    broker.restart();
    let m = SessionManager::new();
    let mut r = request(&broker.url(false), "5");
    r.auth.kind = "basic".into();
    r.auth.username = "fixture-user".into();
    r.auth.password = "fixture-password".into();
    let id = start(&m, r, true);
    state(&m, &id, SessionState::Open).await;
    for qos in std::iter::once(1).chain(std::iter::repeat_n(2, 17)) {
        let after = m.events("owner", &id, 0).unwrap().next_cursor;
        publish(
            &m,
            &id,
            message("restricted/denied", "broker rejects", qos, false),
        );
        let rejected=event(&m,&id,after,|e,_|matches!(e,EventMessage::MqttStatus{status,reason_codes,..} if status=="rejected"&&reason_codes.iter().any(|r|r=="NotAuthorized"))).await;
        assert!(matches!(rejected,EventMessage::MqttStatus{packet_id:Some(id),..} if id>0));
        assert!(!m.events("owner",&id,after).unwrap().events.iter().any(|e|matches!(&e.message,EventMessage::MqttStatus{operation,status,..} if operation=="publish"&&status=="acknowledged")));
    }
    let after = m.events("owner", &id, 0).unwrap().next_cursor;
    publish(
        &m,
        &id,
        message("allowed/accepted", "slot released", 2, false),
    );
    event(&m,&id,after,|e,_|matches!(e,EventMessage::MqttStatus{operation,status,..} if operation=="publish"&&status=="acknowledged")).await;
    m.close_owner("owner").await;
}
