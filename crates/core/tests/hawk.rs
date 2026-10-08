use base64::{Engine, engine::general_purpose::STANDARD};
use moleapi_core::*;
use std::time::{Duration, SystemTime};
fn config(algorithm: &str) -> HawkAuth {
    HawkAuth {
        id: "client-id".into(),
        key: "published-fixture-key".into(),
        algorithm: algorithm.into(),
        nonce: "fixture-nonce".into(),
        timestamp: "1353832234".into(),
        ext: "extra-data".into(),
        app: "application-id".into(),
        delegation: "delegated-id".into(),
        include_payload_hash: true,
        ..Default::default()
    }
}
#[test]
fn payload_suffix_and_delegation_match_independent_postman_vectors() {
    for (algorithm, mac, hash) in [
        (
            "sha1",
            "fLJOe34SrKxlM+mKIagbj+Og9Yw=",
            "75cUqWHC0LoL3RlSJ4L80N89pSI=",
        ),
        (
            "sha256",
            "EUP58pmxDGI9D78BuWstRQXPFLMHY5Tl6EvaOXxr1IM=",
            "gkAohV8ai/HNrYECPT27qaRmxgsUuVq5zhu1MgLEH0I=",
        ),
    ] {
        let mut request = reqwest::Request::new(
            reqwest::Method::POST,
            "http://example.com:8000/resource?b=1&a=2".parse().unwrap(),
        );
        *request.body_mut() = Some("Hello payload".into());
        request.headers_mut().insert(
            "content-type",
            reqwest::header::HeaderValue::from_static("Application/Problem+JSON; charset=utf-8"),
        );
        sign_hawk_request(&config(algorithm), &mut request, SystemTime::now()).unwrap();
        let header = request.headers()["authorization"]
            .to_str()
            .unwrap()
            .strip_prefix("Hawk ")
            .unwrap()
            .parse::<hawk::Header>()
            .unwrap();
        assert_eq!(STANDARD.encode(header.mac.as_ref().unwrap()), mac);
        assert_eq!(STANDARD.encode(header.hash.as_ref().unwrap()), hash);
        assert_eq!(header.app.as_deref(), Some("application-id"));
        assert_eq!(header.dlg.as_deref(), Some("delegated-id"));
        let digest = if algorithm == "sha1" {
            hawk::DigestAlgorithm::Sha1
        } else {
            hawk::DigestAlgorithm::Sha256
        };
        let key = hawk::Key::new("published-fixture-key", digest).unwrap();
        let payload =
            hawk::PayloadHasher::hash("application/problem+json", digest, "Hello payload").unwrap();
        let server = hawk::RequestBuilder::new("POST", "example.com", 8000, "/resource?b=1&a=2")
            .hash(&payload[..])
            .request();
        assert!(server.validate_header(&header, &key, Duration::from_secs(u32::MAX as u64)));
        let mut forged = header.clone();
        forged.app = Some("forged-application".into());
        assert!(!server.validate_header(&forged, &key, Duration::from_secs(u32::MAX as u64)));
        let mut forged = header;
        forged.dlg = Some("forged-delegation".into());
        assert!(!server.validate_header(&forged, &key, Duration::from_secs(u32::MAX as u64)));
    }
}
fn request() -> RequestSpec {
    serde_json::from_value(serde_json::json!({"id":"hawk","name":"Hawk","method":"POST","url":"https://example.test/","description":"","headers":[],"query":[],"body_kind":"text","body":"raw payload","auth":{"kind":"hawk","token":"","username":"","password":"","hawk":{"id":"id","key":"private-key","algorithm":"sha256","include_payload_hash":true}},"timeout_ms":30000,"follow_redirects":true,"verify_tls":true,"assertions":[],"examples":[]})).unwrap()
}
#[test]
fn inactive_drafts_are_preserved_and_invalid_active_fields_reject() {
    let mut r = request();
    r.auth.kind = "none".into();
    r.auth.hawk.as_mut().unwrap().algorithm = "incomplete draft".into();
    r.auth.hawk.as_mut().unwrap().key = "{{unknown-key}}".into();
    assert!(validate_request(&r, false).is_ok());
    assert!(resolve_request(&r, None).is_ok());
    r.auth.kind = "hawk".into();
    assert!(validate_request(&r, false).is_err());
    let mut r = request();
    r.protocol = Protocol::Sse;
    assert!(validate_request(&r, false).is_err());
    let mut invalid = config("sha256");
    invalid.id = "injected\r\nheader".into();
    assert!(validate_hawk(&invalid, false).is_err());
    invalid = config("sha256");
    invalid.timestamp = u64::MAX.to_string();
    assert!(validate_hawk(&invalid, false).is_err());
}
#[tokio::test]
async fn materialized_http_hook_hashes_actual_bytes_and_does_not_forward_mac_across_origins() {
    use axum::{
        Json, Router,
        extract::OriginalUri,
        http::{HeaderMap, StatusCode, header::LOCATION},
        routing::post,
    };
    let sink = Router::new().route(
        "/sink",
        post(|headers: HeaderMap| async move {
            assert!(!headers.contains_key("authorization"));
            Json(serde_json::json!({"ok":true}))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target = format!("http://{}/sink", listener.local_addr().unwrap());
    let sink_task = tokio::spawn(async move {
        axum::serve(listener, sink).await.unwrap();
    });
    let endpoint_target = target.clone();
    let app = Router::new().route(
        "/source",
        post(
            move |OriginalUri(uri): OriginalUri, headers: HeaderMap, body: axum::body::Bytes| {
                let target = endpoint_target.clone();
                async move {
                    let header = headers["authorization"]
                        .to_str()
                        .unwrap()
                        .strip_prefix("Hawk ")
                        .unwrap()
                        .parse::<hawk::Header>()
                        .unwrap();
                    let host = headers["host"].to_str().unwrap();
                    let url = url::Url::parse(&format!("http://{host}{uri}")).unwrap();
                    let hash = hawk::PayloadHasher::hash(
                        "application/octet-stream",
                        hawk::DigestAlgorithm::Sha256,
                        &body,
                    )
                    .unwrap();
                    let verifier = hawk::RequestBuilder::from_url("POST", &url)
                        .unwrap()
                        .hash(&hash[..])
                        .request();
                    assert!(verifier.validate_header(
                        &header,
                        &hawk::Key::new("private-key", hawk::DigestAlgorithm::Sha256).unwrap(),
                        Duration::from_secs(60)
                    ));
                    assert_eq!(body.as_ref(), [0, 255, 1]);
                    (StatusCode::TEMPORARY_REDIRECT, [(LOCATION, target)])
                }
            },
        ),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let source = format!("http://{}/source", listener.local_addr().unwrap());
    let source_task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let mut r = request();
    r.url = source;
    r.body_kind = "binary".into();
    r.body = serde_json::to_string(&BinaryBody {
        file_name: "raw.bin".into(),
        mime: "application/octet-stream".into(),
        base64: Some(STANDARD.encode([0, 255, 1])),
    })
    .unwrap();
    assert_eq!(
        execute(
            &r,
            None,
            NetworkPolicy {
                allow_private_network: true
            }
        )
        .await
        .unwrap()
        .status,
        200
    );
    source_task.abort();
    sink_task.abort();
}

#[test]
fn quoted_extra_data_matches_postman_wire_encoding_and_decodes_before_verification() {
    let mut config = config("sha256");
    config.ext = "raw\\value,\"quote\"".into();
    let mut request = reqwest::Request::new(
        reqwest::Method::POST,
        "http://example.com:8000/resource?b=1&a=2".parse().unwrap(),
    );
    *request.body_mut() = Some("Hello payload".into());
    request.headers_mut().insert(
        "content-type",
        reqwest::header::HeaderValue::from_static("Application/Problem+JSON; charset=utf-8"),
    );
    sign_hawk_request(&config, &mut request, SystemTime::now()).unwrap();
    let value = request.headers()["authorization"].to_str().unwrap();
    assert!(value.contains(r#"ext="raw\\value,\"quote\"""#));
    let header = value
        .strip_prefix("Hawk ")
        .unwrap()
        .parse::<hawk::Header>()
        .unwrap();
    assert_eq!(header.ext.as_deref(), Some(config.ext.as_str()));
    assert_eq!(
        STANDARD.encode(header.mac.as_ref().unwrap()),
        "3t/Nl+zEhwszmiWOGKu0ARLRaUvin0BfPqZ9SxLCK+s="
    );
    let hash = hawk::PayloadHasher::hash(
        "application/problem+json",
        hawk::DigestAlgorithm::Sha256,
        "Hello payload",
    )
    .unwrap();
    let verifier = hawk::RequestBuilder::new("POST", "example.com", 8000, "/resource?b=1&a=2")
        .hash(&hash[..])
        .request();
    assert!(verifier.validate_header(
        &header,
        &hawk::Key::new("published-fixture-key", hawk::DigestAlgorithm::Sha256).unwrap(),
        Duration::from_secs(u32::MAX as u64)
    ));
}

#[test]
fn modern_sdk_digests_match_independent_mac_vectors() {
    for (algorithm, expected) in [
        (
            "sha384",
            "AwnWQ9m8urlWhJiXYjnYl4Nwh3wccBusVtTHlJyqbMmsnGnrUFjn3jWgporkUINi",
        ),
        (
            "sha512",
            "sUEkHpKYsiGZUDkk73rJENS4qHxbOMoERt5n00ObXJy9nCG3gmdj0TuwT1B9vtYIQyBzfJ+/27JPdmq6BtdG9Q==",
        ),
    ] {
        let mut config = config(algorithm);
        config.include_payload_hash = false;
        let mut request = reqwest::Request::new(
            reqwest::Method::POST,
            "http://example.com:8000/resource?b=1&a=2".parse().unwrap(),
        );
        sign_hawk_request(&config, &mut request, SystemTime::now()).unwrap();
        let header = request.headers()["authorization"]
            .to_str()
            .unwrap()
            .strip_prefix("Hawk ")
            .unwrap()
            .parse::<hawk::Header>()
            .unwrap();
        assert_eq!(STANDARD.encode(header.mac.unwrap()), expected);
    }
}
#[tokio::test]
async fn multipart_and_same_origin_method_change_are_signed_after_body_preparation() {
    use axum::{
        Json, Router,
        extract::OriginalUri,
        http::{HeaderMap, StatusCode, header::LOCATION},
        routing::{get, post},
    };
    use std::sync::Arc;
    let macs = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let before = macs.clone();
    let after = macs.clone();
    let app = Router::new()
        .route(
            "/start",
            post(
                move |OriginalUri(uri): OriginalUri,
                      headers: HeaderMap,
                      body: axum::body::Bytes| {
                    let before = before.clone();
                    async move {
                        let parsed = headers["authorization"]
                            .to_str()
                            .unwrap()
                            .strip_prefix("Hawk ")
                            .unwrap()
                            .parse::<hawk::Header>()
                            .unwrap();
                        let content = headers["content-type"].to_str().unwrap();
                        assert!(content.starts_with("multipart/form-data;"));
                        assert!(body.windows(12).any(|v| v == b"field-value!"));
                        let hash = hawk::PayloadHasher::hash(
                            "multipart/form-data",
                            hawk::DigestAlgorithm::Sha256,
                            &body,
                        )
                        .unwrap();
                        let url = url::Url::parse(&format!(
                            "http://{}{uri}",
                            headers["host"].to_str().unwrap()
                        ))
                        .unwrap();
                        let check = hawk::RequestBuilder::from_url("POST", &url)
                            .unwrap()
                            .hash(&hash[..])
                            .request();
                        assert!(check.validate_header(
                            &parsed,
                            &hawk::Key::new("private-key", hawk::DigestAlgorithm::Sha256).unwrap(),
                            Duration::from_secs(60)
                        ));
                        before
                            .lock()
                            .await
                            .push(parsed.mac.unwrap().as_ref().to_vec());
                        (StatusCode::SEE_OTHER, [(LOCATION, "/after?method=get")])
                    }
                },
            ),
        )
        .route(
            "/after",
            get(
                move |OriginalUri(uri): OriginalUri,
                      headers: HeaderMap,
                      body: axum::body::Bytes| {
                    let after = after.clone();
                    async move {
                        assert!(body.is_empty());
                        assert!(!headers.contains_key("content-type"));
                        let parsed = headers["authorization"]
                            .to_str()
                            .unwrap()
                            .strip_prefix("Hawk ")
                            .unwrap()
                            .parse::<hawk::Header>()
                            .unwrap();
                        let hash = hawk::PayloadHasher::hash("", hawk::DigestAlgorithm::Sha256, [])
                            .unwrap();
                        let url = url::Url::parse(&format!(
                            "http://{}{uri}",
                            headers["host"].to_str().unwrap()
                        ))
                        .unwrap();
                        let check = hawk::RequestBuilder::from_url("GET", &url)
                            .unwrap()
                            .hash(&hash[..])
                            .request();
                        assert!(check.validate_header(
                            &parsed,
                            &hawk::Key::new("private-key", hawk::DigestAlgorithm::Sha256).unwrap(),
                            Duration::from_secs(60)
                        ));
                        after
                            .lock()
                            .await
                            .push(parsed.mac.unwrap().as_ref().to_vec());
                        Json(serde_json::json!({"ok":true}))
                    }
                },
            ),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/start", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let mut request = request();
    request.url = url;
    request.body_kind = "multipart".into();
    request.body = serde_json::to_string(&MultipartBody {
        parts: vec![
            MultipartPart {
                id: "text".into(),
                name: "text".into(),
                enabled: true,
                value: MultipartValue::Text {
                    text: "field-value!".into(),
                    mime: String::new(),
                },
            },
            MultipartPart {
                id: "file".into(),
                name: "file".into(),
                enabled: true,
                value: MultipartValue::File {
                    file: BinaryBody {
                        file_name: "file.bin".into(),
                        mime: "application/octet-stream".into(),
                        base64: Some(STANDARD.encode([0, 255, 1])),
                    },
                },
            },
        ],
    })
    .unwrap();
    assert_eq!(
        execute(
            &request,
            None,
            NetworkPolicy {
                allow_private_network: true
            }
        )
        .await
        .unwrap()
        .status,
        200
    );
    let macs = macs.lock().await;
    assert_eq!(macs.len(), 2);
    assert_ne!(macs[0], macs[1]);
    task.abort();
}
#[tokio::test]
async fn ipv6_authority_uses_unbracketed_hawk_host_and_actual_port() {
    use axum::{Json, Router, extract::OriginalUri, http::HeaderMap, routing::post};
    let listener = tokio::net::TcpListener::bind((std::net::Ipv6Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let app =
        Router::new().route(
            "/v6",
            post(
                move |OriginalUri(uri): OriginalUri,
                      headers: HeaderMap,
                      body: axum::body::Bytes| async move {
                    let parsed = headers["authorization"]
                        .to_str()
                        .unwrap()
                        .strip_prefix("Hawk ")
                        .unwrap()
                        .parse::<hawk::Header>()
                        .unwrap();
                    let hash = hawk::PayloadHasher::hash("", hawk::DigestAlgorithm::Sha256, &body)
                        .unwrap();
                    let resource = uri.to_string();
                    let check = hawk::RequestBuilder::new("POST", "::1", port, &resource)
                        .hash(&hash[..])
                        .request();
                    assert!(check.validate_header(
                        &parsed,
                        &hawk::Key::new("private-key", hawk::DigestAlgorithm::Sha256).unwrap(),
                        Duration::from_secs(60)
                    ));
                    Json(serde_json::json!({"ok":true}))
                },
            ),
        );
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let mut request = request();
    request.url = format!("http://[::1]:{port}/v6");
    assert_eq!(
        execute(
            &request,
            None,
            NetworkPolicy {
                allow_private_network: true
            }
        )
        .await
        .unwrap()
        .status,
        200
    );
    task.abort();
}
