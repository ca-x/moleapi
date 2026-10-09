use bytes::Bytes;
use http_body_util::Full;
use hyper_util::rt::TokioIo;
use moleapi_core::*;
use serde_json::json;
use std::{convert::Infallible, sync::Arc};
const LOCAL: NetworkPolicy = NetworkPolicy {
    allow_private_network: true,
};
fn request(url: String, network: RequestNetwork) -> RequestSpec {
    serde_json::from_value(json!({"id":"r","name":"network","method":"GET","url":url,"description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":5000,"follow_redirects":true,"verify_tls":true,"assertions":[],"examples":[],"network":network})).unwrap()
}
#[tokio::test]
async fn dns_override_preserves_host_and_blocks_private_ips_in_hosted_policy() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        hyper::server::conn::http1::Builder::new()
            .serve_connection(
                TokioIo::new(stream),
                hyper::service::service_fn(|r: hyper::Request<hyper::body::Incoming>| async move {
                    assert_eq!(
                        r.headers()["host"].to_str().unwrap().split(':').next(),
                        Some("network.test")
                    );
                    Ok::<_, Infallible>(hyper::Response::new(Full::new(Bytes::from_static(b"ok"))))
                }),
            )
            .await
            .unwrap();
    });
    let c = RequestNetwork {
        dns: vec![DnsOverride {
            hostname: "network.test".into(),
            addresses: vec!["127.0.0.1".into()],
        }],
        ..Default::default()
    };
    let r = request(format!("http://network.test:{port}/"), c);
    assert!(
        execute(
            &r,
            None,
            NetworkPolicy {
                allow_private_network: false
            }
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("blocked")
    );
    assert_eq!(execute(&r, None, LOCAL).await.unwrap().body, "ok");
    server.abort();
}
#[tokio::test]
async fn custom_ca_and_mutual_tls_accept_pem_and_encrypted_pfx() {
    use rcgen::{BasicConstraints, CertificateParams, ExtendedKeyUsagePurpose, IsCa, KeyPair};
    let mut ca_params = CertificateParams::new(vec!["MoleAPI Test CA".into()]).unwrap();
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let ca_key = KeyPair::generate().unwrap();
    let ca = ca_params.self_signed(&ca_key).unwrap();
    let issuer = rcgen::Issuer::from_params(&ca_params, &ca_key);
    let mut server_params = CertificateParams::new(vec!["network.test".into()]).unwrap();
    server_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    let server_key = KeyPair::generate().unwrap();
    let server_cert = server_params.signed_by(&server_key, &issuer).unwrap();
    let mut client_params = CertificateParams::new(vec!["client.test".into()]).unwrap();
    client_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
    let client_key = KeyPair::generate().unwrap();
    let client_cert = client_params.signed_by(&client_key, &issuer).unwrap();
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
    let server = tokio::spawn(async move {
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let acceptor = acceptor.clone();
            tokio::spawn(async move {
                let Ok(tls) = acceptor.accept(stream).await else {
                    return;
                };
                assert!(tls.get_ref().1.peer_certificates().is_some());
                let _ = hyper::server::conn::http1::Builder::new()
                    .serve_connection(
                        TokioIo::new(tls),
                        hyper::service::service_fn(
                            |_: hyper::Request<hyper::body::Incoming>| async {
                                Ok::<_, Infallible>(hyper::Response::new(Full::new(
                                    Bytes::from_static(b"mutual"),
                                )))
                            },
                        ),
                    )
                    .await;
            });
        }
    });
    let c = RequestNetwork {
        built_in_roots: false,
        ca_pem: ca.pem(),
        dns: vec![DnsOverride {
            hostname: "network.test".into(),
            addresses: vec!["127.0.0.1".into()],
        }],
        identity: ClientIdentity {
            enabled: true,
            certificate_pem: client_cert.pem(),
            key_pem: client_key.serialize_pem(),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut r = request(format!("https://network.test:{port}/"), c);
    assert_eq!(execute(&r, None, LOCAL).await.unwrap().body, "mutual");
    let mut store = p12_keystore::KeyStore::new();
    let chain = p12_keystore::PrivateKeyChain::new(
        &[1u8, 2, 3][..],
        p12_keystore::PrivateKey::from_der(&client_key.serialize_der()).unwrap(),
        [p12_keystore::Certificate::from_der(client_cert.der()).unwrap()],
    );
    store.add_entry(
        "client",
        p12_keystore::KeyStoreEntry::PrivateKeyChain(chain),
    );
    use base64::Engine;
    let bytes = store.writer("private-password").write().unwrap();
    let c = r.network.as_mut().unwrap();
    c.identity = ClientIdentity {
        enabled: true,
        format: IdentityFormat::Pkcs12,
        pkcs12_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
        password: "private-password".into(),
        alias: "client".into(),
        ..Default::default()
    };
    assert_eq!(execute(&r, None, LOCAL).await.unwrap().body, "mutual");
    r.network.as_mut().unwrap().identity.password = "wrong".into();
    assert!(execute(&r, None, LOCAL).await.is_err());
    r.network.as_mut().unwrap().identity.password = "private-password".into();
    r.network.as_mut().unwrap().identity.alias = "missing".into();
    assert!(execute(&r, None, LOCAL).await.is_err());
    r.network.as_mut().unwrap().identity.enabled = false;
    assert!(execute(&r, None, LOCAL).await.is_err());
    server.abort();
}
#[test]
fn inactive_network_drafts_are_not_interpolated_or_overwritten() {
    let c = RequestNetwork {
        proxy: RequestProxy {
            enabled: false,
            url: "{{missing}}".into(),
            password: "{{password}}".into(),
            ..Default::default()
        },
        identity: ClientIdentity {
            enabled: false,
            key_pem: "{{missing}}".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    let r = request("https://example.com".into(), c);
    let saved = serde_json::to_value(&r).unwrap();
    assert!(resolve_request(&r, None).is_ok());
    assert_eq!(serde_json::to_value(&r).unwrap(), saved);
}

#[tokio::test]
async fn http2_prior_knowledge_uses_real_h2_frames() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        hyper::server::conn::http2::Builder::new(hyper_util::rt::TokioExecutor::new())
            .serve_connection(
                TokioIo::new(stream),
                hyper::service::service_fn(|r: hyper::Request<hyper::body::Incoming>| async move {
                    assert_eq!(r.version(), http::Version::HTTP_2);
                    Ok::<_, Infallible>(hyper::Response::new(Full::new(Bytes::from_static(b"h2"))))
                }),
            )
            .await
            .unwrap();
    });
    let r = request(
        format!("http://127.0.0.1:{port}/"),
        RequestNetwork {
            http_mode: HttpMode::Http2PriorKnowledge,
            ..Default::default()
        },
    );
    assert_eq!(execute(&r, None, LOCAL).await.unwrap().body, "h2");
    server.abort();
}
#[tokio::test]
async fn http_proxy_credentials_are_used_only_at_proxy_and_bypass_reaches_target() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        hyper::server::conn::http1::Builder::new()
            .serve_connection(
                TokioIo::new(stream),
                hyper::service::service_fn(|r: hyper::Request<hyper::body::Incoming>| async move {
                    assert_eq!(r.uri().scheme_str(), Some("http"));
                    assert_eq!(r.headers()["proxy-authorization"], "Basic dXNlcjpwYXNz");
                    assert!(!r.headers().contains_key("authorization"));
                    Ok::<_, Infallible>(hyper::Response::new(Full::new(Bytes::from_static(
                        b"proxy",
                    ))))
                }),
            )
            .await
            .unwrap();
    });
    let mut r = request(
        "http://example.com/test".into(),
        RequestNetwork {
            proxy: RequestProxy {
                enabled: true,
                url: proxy_url,
                username: "user".into(),
                password: "pass".into(),
                ..Default::default()
            },
            ..Default::default()
        },
    );
    assert!(
        execute(
            &r,
            None,
            NetworkPolicy {
                allow_private_network: false
            }
        )
        .await
        .is_err()
    );
    assert_eq!(execute(&r, None, LOCAL).await.unwrap().body, "proxy");
    server.abort();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    r.url = format!("http://{}/", listener.local_addr().unwrap());
    r.network.as_mut().unwrap().proxy.bypass = "127.0.0.0/8".into();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let _ = hyper::server::conn::http1::Builder::new()
            .serve_connection(
                TokioIo::new(stream),
                hyper::service::service_fn(|r: hyper::Request<hyper::body::Incoming>| async move {
                    assert!(!r.headers().contains_key("proxy-authorization"));
                    Ok::<_, Infallible>(hyper::Response::new(Full::new(Bytes::from_static(
                        b"direct",
                    ))))
                }),
            )
            .await;
    });
    assert_eq!(execute(&r, None, LOCAL).await.unwrap().body, "direct");
    server.abort();
}
#[tokio::test]
async fn https_auto_negotiates_h2_with_custom_ca_and_original_sni() {
    let certified = rcgen::generate_simple_self_signed(vec!["network.test".into()]).unwrap();
    let mut config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(
        vec![certified.cert.der().clone()],
        rustls::pki_types::PrivatePkcs8KeyDer::from(certified.signing_key.serialize_der()).into(),
    )
    .unwrap();
    config.alpn_protocols = vec![b"h2".to_vec()];
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let tls = tokio_rustls::TlsAcceptor::from(Arc::new(config))
            .accept(stream)
            .await
            .unwrap();
        assert_eq!(tls.get_ref().1.server_name(), Some("network.test"));
        assert_eq!(tls.get_ref().1.alpn_protocol(), Some(b"h2".as_slice()));
        let _ = hyper::server::conn::http2::Builder::new(hyper_util::rt::TokioExecutor::new())
            .serve_connection(
                TokioIo::new(tls),
                hyper::service::service_fn(|r: hyper::Request<hyper::body::Incoming>| async move {
                    assert_eq!(r.version(), http::Version::HTTP_2);
                    Ok::<_, Infallible>(hyper::Response::new(Full::new(Bytes::from_static(
                        b"tls-h2",
                    ))))
                }),
            )
            .await;
    });
    let r = request(
        format!("https://network.test:{port}/"),
        RequestNetwork {
            http_mode: HttpMode::Auto,
            built_in_roots: false,
            ca_pem: certified.cert.pem(),
            dns: vec![DnsOverride {
                hostname: "network.test".into(),
                addresses: vec!["127.0.0.1".into()],
            }],
            ..Default::default()
        },
    );
    assert_eq!(execute(&r, None, LOCAL).await.unwrap().body, "tls-h2");
    server.abort();
}
#[tokio::test]
async fn socks5_authentication_tunnels_without_forwarding_credentials_to_http_target() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let target = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_address = target.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (stream, _) = target.accept().await.unwrap();
        let _ = hyper::server::conn::http1::Builder::new()
            .serve_connection(
                TokioIo::new(stream),
                hyper::service::service_fn(|r: hyper::Request<hyper::body::Incoming>| async move {
                    assert!(!r.headers().contains_key("proxy-authorization"));
                    assert!(!r.headers().contains_key("authorization"));
                    Ok::<_, Infallible>(hyper::Response::new(Full::new(Bytes::from_static(
                        b"socks",
                    ))))
                }),
            )
            .await;
    });
    let proxy = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_url = format!("socks5://{}", proxy.local_addr().unwrap());
    // Test-only SOCKS acceptor validates the client SDK's wire exchange.
    let tunnel = tokio::spawn(async move {
        let (mut socket, _) = proxy.accept().await.unwrap();
        assert_eq!(socket.read_u8().await.unwrap(), 5);
        let count = socket.read_u8().await.unwrap();
        let mut methods = vec![0; count as usize];
        socket.read_exact(&mut methods).await.unwrap();
        assert!(methods.contains(&2));
        socket.write_all(&[5, 2]).await.unwrap();
        assert_eq!(socket.read_u8().await.unwrap(), 1);
        let count = socket.read_u8().await.unwrap();
        let mut user = vec![0; count as usize];
        socket.read_exact(&mut user).await.unwrap();
        assert_eq!(user, b"user");
        let count = socket.read_u8().await.unwrap();
        let mut password = vec![0; count as usize];
        socket.read_exact(&mut password).await.unwrap();
        assert_eq!(password, b"pass");
        socket.write_all(&[1, 0]).await.unwrap();
        let mut prefix = [0; 4];
        socket.read_exact(&mut prefix).await.unwrap();
        assert_eq!(prefix, [5, 1, 0, 1]);
        let mut ip = [0; 4];
        socket.read_exact(&mut ip).await.unwrap();
        assert_eq!(ip, [127, 0, 0, 1]);
        assert_eq!(socket.read_u16().await.unwrap(), target_address.port());
        let mut destination = tokio::net::TcpStream::connect(target_address)
            .await
            .unwrap();
        socket
            .write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 0])
            .await
            .unwrap();
        let _ = tokio::io::copy_bidirectional(&mut socket, &mut destination).await;
    });
    let r = request(
        format!("http://{target_address}/"),
        RequestNetwork {
            proxy: RequestProxy {
                enabled: true,
                url: proxy_url,
                username: "user".into(),
                password: "pass".into(),
                ..Default::default()
            },
            ..Default::default()
        },
    );
    assert_eq!(execute(&r, None, LOCAL).await.unwrap().body, "socks");
    tunnel.abort();
    server.abort();
}
#[tokio::test]
async fn client_identity_stays_on_same_origin_and_is_not_sent_after_cross_origin_redirect() {
    use rcgen::{BasicConstraints, CertificateParams, ExtendedKeyUsagePurpose, IsCa, KeyPair};
    let mut params = CertificateParams::new(vec!["Redirect Test CA".into()]).unwrap();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "redirect-ca");
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let ca_key = KeyPair::generate().unwrap();
    let ca = params.self_signed(&ca_key).unwrap();
    let issuer = rcgen::Issuer::from_params(&params, &ca_key);
    let mut params = CertificateParams::new(vec!["redirect.test".into()]).unwrap();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "redirect.test");
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    let server_key = KeyPair::generate().unwrap();
    let server_cert = params.signed_by(&server_key, &issuer).unwrap();
    let mut params = CertificateParams::new(vec!["client.test".into()]).unwrap();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "redirect-client");
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
                vec![server_cert.der().clone()],
                rustls::pki_types::PrivatePkcs8KeyDer::from(server_key.serialize_der()).into(),
            )
            .unwrap()
    };
    let source_config = Arc::new(config(required));
    let target_config = Arc::new(config(optional));
    let target = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_url = format!(
        "https://redirect.test:{}/done",
        target.local_addr().unwrap().port()
    );
    let target_task = tokio::spawn(async move {
        let (socket, _) = target.accept().await.unwrap();
        let tls = tokio_rustls::TlsAcceptor::from(target_config)
            .accept(socket)
            .await
            .unwrap();
        let has_identity = tls
            .get_ref()
            .1
            .peer_certificates()
            .is_some_and(|c| !c.is_empty());
        let _ = hyper::server::conn::http1::Builder::new()
            .serve_connection(
                TokioIo::new(tls),
                hyper::service::service_fn(
                    move |_: hyper::Request<hyper::body::Incoming>| async move {
                        Ok::<_, Infallible>(
                            hyper::Response::builder()
                                .header("connection", "close")
                                .body(Full::new(Bytes::from_static(if has_identity {
                                    b"client-present"
                                } else {
                                    b"anonymous"
                                })))
                                .unwrap(),
                        )
                    },
                ),
            )
            .await;
    });
    let source = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let source_url = format!(
        "https://redirect.test:{}/start",
        source.local_addr().unwrap().port()
    );
    let source_task = tokio::spawn(async move {
        for _ in 0..2 {
            let (socket, _) = source.accept().await.unwrap();
            let tls = tokio_rustls::TlsAcceptor::from(source_config.clone())
                .accept(socket)
                .await
                .unwrap();
            assert!(
                tls.get_ref()
                    .1
                    .peer_certificates()
                    .is_some_and(|c| !c.is_empty())
            );
            let target_url = target_url.clone();
            let _ = hyper::server::conn::http1::Builder::new()
                .serve_connection(
                    TokioIo::new(tls),
                    hyper::service::service_fn(move |r: hyper::Request<hyper::body::Incoming>| {
                        let location = if r.uri().path() == "/start" {
                            "/jump".to_owned()
                        } else {
                            target_url.clone()
                        };
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
        }
    });
    let r = request(
        source_url,
        RequestNetwork {
            built_in_roots: false,
            ca_pem: ca.pem(),
            dns: vec![DnsOverride {
                hostname: "redirect.test".into(),
                addresses: vec!["127.0.0.1".into()],
            }],
            identity: ClientIdentity {
                enabled: true,
                certificate_pem: client_cert.pem(),
                key_pem: client_key.serialize_pem(),
                ..Default::default()
            },
            ..Default::default()
        },
    );
    assert_eq!(execute(&r, None, LOCAL).await.unwrap().body, "anonymous");
    assert!(r.network.as_ref().unwrap().identity.enabled);
    source_task.await.unwrap();
    target_task.await.unwrap();
}
