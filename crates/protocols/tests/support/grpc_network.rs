use super::fixture::{Fixture, pb::echo_service_server::EchoServiceServer};
pub async fn start_mutual_tls_network() -> (
    String,
    moleapi_core::RequestNetwork,
    String,
    tokio::task::JoinHandle<()>,
) {
    use rcgen::{BasicConstraints, CertificateParams, ExtendedKeyUsagePurpose, IsCa, KeyPair};
    let mut ca_params = CertificateParams::new(vec!["MoleAPI gRPC Test CA".into()]).unwrap();
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let ca_key = KeyPair::generate().unwrap();
    let ca = ca_params.self_signed(&ca_key).unwrap();
    let issuer = rcgen::Issuer::from_params(&ca_params, &ca_key);
    let mut server_params = CertificateParams::new(vec!["grpc-network.test".into()]).unwrap();
    server_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    let server_key = KeyPair::generate().unwrap();
    let cert = server_params.signed_by(&server_key, &issuer).unwrap();
    let mut client_params = CertificateParams::new(vec!["client.test".into()]).unwrap();
    client_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
    let client_key = KeyPair::generate().unwrap();
    let client_cert = client_params.signed_by(&client_key, &issuer).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let network = moleapi_core::RequestNetwork {
        http_mode: moleapi_core::HttpMode::Auto,
        built_in_roots: false,
        ca_pem: ca.pem(),
        dns: vec![moleapi_core::DnsOverride {
            hostname: "grpc-network.test".into(),
            addresses: vec!["127.0.0.1".into()],
        }],
        identity: moleapi_core::ClientIdentity {
            enabled: true,
            certificate_pem: client_cert.pem(),
            key_pem: client_key.serialize_pem(),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut store = p12_keystore::KeyStore::new();
    store.add_entry(
        "grpc-client",
        p12_keystore::KeyStoreEntry::PrivateKeyChain(p12_keystore::PrivateKeyChain::new(
            &[1u8, 2, 3][..],
            p12_keystore::PrivateKey::from_der(&client_key.serialize_der()).unwrap(),
            [p12_keystore::Certificate::from_der(client_cert.der()).unwrap()],
        )),
    );
    use base64::Engine;
    let pfx = base64::engine::general_purpose::STANDARD
        .encode(store.writer("test-private-password").write().unwrap());
    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .tls_config(
                tonic::transport::ServerTlsConfig::new()
                    .identity(tonic::transport::Identity::from_pem(
                        cert.pem(),
                        server_key.serialize_pem(),
                    ))
                    .client_ca_root(tonic::transport::Certificate::from_pem(ca.pem())),
            )
            .unwrap()
            .add_service(EchoServiceServer::new(Fixture))
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });
    (
        format!("https://grpc-network.test:{port}"),
        network,
        pfx,
        task,
    )
}
