mod common;
use common::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
async fn open(router: &Router, url: &str, framing: &str) -> String {
    let mut data = example_data();
    data["global_variables"] =
        json!([{"id":"empty","key":"empty_token","value":"","secret":true,"enabled":true}]);
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = json!(url);
    request["examples"] = json!([]);
    request["protocol"] = json!({"kind":"tcp","framing":framing});
    let (status, w) = call(
        router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"TCP","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    let (status, s) = call(
        router,
        "POST",
        "/api/sessions",
        None,
        Some(json!({"workspace_id":"w","request":w["data"]["collections"][0]["requests"][0]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{s}");
    let id = s["id"].as_str().unwrap().to_owned();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let (_, s) = call(router, "GET", &format!("/api/sessions/{id}"), None, None).await;
            if s["state"] == "open" {
                break;
            }
            assert_ne!(s["state"], "error", "{s}");
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    id
}
async fn events(router: &Router, id: &str) -> Value {
    call(
        router,
        "GET",
        &format!("/api/sessions/{id}/events"),
        None,
        None,
    )
    .await
    .1
}
#[tokio::test]
async fn raw_binary_send_half_close_continues_reading_and_accounts_wire_bytes() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("tcp://{}", listener.local_addr().unwrap());
    let fixture = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).await.unwrap();
        assert_eq!(bytes, [0, 1, 255, 65]);
        stream.write_all(b"after EOF").await.unwrap();
    });
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("tcp.db")).await.unwrap();
    let id = open(&router, &url, "raw").await;
    let(status,value)=call(&router,"POST",&format!("/api/sessions/{id}/send"),None,Some(json!({"kind":"tcp_send","message":{"encoding":"hex","payload_source":"00 01 ff 41","secret":false}}))).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    let (status, value) = call(
        &router,
        "POST",
        &format!("/api/sessions/{id}/send"),
        None,
        Some(json!({"kind":"tcp_half_close"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    fixture.await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let (_, s) = call(&router, "GET", &format!("/api/sessions/{id}"), None, None).await;
            if s["state"] == "closed" {
                assert_eq!(s["sent_bytes"], 4);
                assert_eq!(s["received_bytes"], 9);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let batch = events(&router, &id).await;
    assert!(
        batch["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["direction"] == "incoming"
                && e["message"]["kind"] == "tcp_data"
                && e["message"]["text"] == "after EOF"),
        "{batch}"
    );
    let (status, _) = call(
        &router,
        "POST",
        &format!("/api/sessions/{id}/send"),
        None,
        Some(json!({"kind":"tcp_send","message":{"encoding":"text","payload_source":"late"}})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn tcp_network_custom_ca_and_client_identity_preserve_tls_name_and_raw_payload() {
    use rcgen::{BasicConstraints, CertificateParams, ExtendedKeyUsagePurpose, IsCa, KeyPair};
    use std::sync::Arc;
    let mut params = CertificateParams::new(vec!["TCP Test CA".into()]).unwrap();
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let ca_key = KeyPair::generate().unwrap();
    let ca = params.self_signed(&ca_key).unwrap();
    let issuer = rcgen::Issuer::from_params(&params, &ca_key);
    let mut params = CertificateParams::new(vec!["tcp-network.test".into()]).unwrap();
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    let server_key = KeyPair::generate().unwrap();
    let server_cert = params.signed_by(&server_key, &issuer).unwrap();
    let mut params = CertificateParams::new(vec!["client.test".into()]).unwrap();
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
    let client_key = KeyPair::generate().unwrap();
    let client_cert = params.signed_by(&client_key, &issuer).unwrap();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut roots = rustls::RootCertStore::empty();
    roots.add(ca.der().clone()).unwrap();
    let verifier = rustls::server::WebPkiClientVerifier::builder_with_provider(
        Arc::new(roots),
        provider.clone(),
    )
    .build()
    .unwrap();
    let config = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_client_cert_verifier(verifier)
        .with_single_cert(
            vec![server_cert.der().clone()],
            rustls::pki_types::PrivatePkcs8KeyDer::from(server_key.serialize_der()).into(),
        )
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let fixture = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut tls = tokio_rustls::TlsAcceptor::from(Arc::new(config))
            .accept(socket)
            .await
            .unwrap();
        assert_eq!(tls.get_ref().1.server_name(), Some("tcp-network.test"));
        assert!(tls.get_ref().1.peer_certificates().is_some());
        let mut bytes = [0; 4];
        tls.read_exact(&mut bytes).await.unwrap();
        assert_eq!(bytes, [0, 1, 255, 65]);
        tls.write_all(b"tcp-network-ok").await.unwrap();
        tls.shutdown().await.unwrap();
    });
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("tcp-network.db")).await.unwrap();
    let mut data = example_data();
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = json!(format!("tcps://tcp-network.test:{port}"));
    request["protocol"] = json!({"kind":"tcp","framing":"raw","no_delay":false});
    request["examples"] = json!([]);
    request["network"] = json!({"built_in_roots":false,"ca_pem":ca.pem(),"dns":[{"hostname":"tcp-network.test","addresses":["127.0.0.1"]}],"identity":{"enabled":true,"certificate_pem":client_cert.pem(),"key_pem":client_key.serialize_pem()}});
    let (status, w) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"TCP network","data":data})),
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
    assert_eq!(status, StatusCode::OK, "{s}");
    let id = s["id"].as_str().unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let (_, s) = call(&router, "GET", &format!("/api/sessions/{id}"), None, None).await;
            if s["state"] == "open" {
                break;
            }
            assert_ne!(s["state"], "error", "{s}");
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let (status, s) = call(
        &router,
        "POST",
        &format!("/api/sessions/{id}/send"),
        None,
        Some(
            json!({"kind":"tcp_send","message":{"encoding":"hex","payload_source":"00 01 ff 41"}}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{s}");
    tokio::time::timeout(std::time::Duration::from_secs(5), fixture)
        .await
        .unwrap()
        .unwrap();
    call(
        &router,
        "DELETE",
        &format!("/api/sessions/{id}"),
        None,
        None,
    )
    .await;
}
