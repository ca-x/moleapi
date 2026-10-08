use moleapi_core::*;
use std::time::{Duration, SystemTime};
fn config() -> AwsAuth {
    AwsAuth {
        access_key: "AKIDEXAMPLE".into(),
        secret_key: "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY".into(),
        region: "us-east-1".into(),
        service: "service".into(),
        expires_seconds: 3600,
        ..Default::default()
    }
}
fn time() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1440938160)
}
#[test]
fn sdk_signature_matches_independent_published_header_and_query_vectors() {
    let mut request = reqwest::Request::new(
        reqwest::Method::GET,
        "https://example.amazonaws.com/".parse().unwrap(),
    );
    sign_aws_request(&config(), &mut request, time()).unwrap();
    let expected = include_str!("fixtures/aws/get-vanilla-header.txt")
        .lines()
        .find_map(|line| line.strip_prefix("Authorization:"))
        .unwrap();
    assert_eq!(request.headers()["authorization"], expected);
    assert_eq!(request.headers()["x-amz-date"], "20150830T123600Z");
    let mut presigned = reqwest::Request::new(
        reqwest::Method::GET,
        "https://example.amazonaws.com/".parse().unwrap(),
    );
    let mut query = config();
    query.location = AuthLocation::Query;
    sign_aws_request(&query, &mut presigned, time()).unwrap();
    let path = include_str!("fixtures/aws/get-vanilla-query.txt")
        .lines()
        .next()
        .unwrap()
        .strip_prefix("GET ")
        .unwrap()
        .strip_suffix(" HTTP/1.1")
        .unwrap();
    let expected_url = url::Url::parse(&format!("https://example.amazonaws.com{path}")).unwrap();
    let mut expected_pairs = expected_url.query_pairs().collect::<Vec<_>>();
    expected_pairs.sort();
    let mut actual_pairs = presigned.url().query_pairs().collect::<Vec<_>>();
    actual_pairs.sort();
    assert_eq!(actual_pairs, expected_pairs);
    assert!(
        presigned
            .url()
            .query()
            .unwrap()
            .contains("AKIDEXAMPLE%2F20150830%2Fus-east-1%2Fservice%2Faws4_request")
    );
    assert!(!presigned.headers().contains_key("authorization"));
}
#[test]
fn sdk_signs_session_tokens_exact_bytes_duplicate_queries_and_s3_paths() {
    let mut config = config();
    config.service = "s3".into();
    config.session_token = "synthetic-session-token".into();
    let make = || {
        let mut request = reqwest::Request::new(
            reqwest::Method::POST,
            "https://example.amazonaws.com/a//b/../c?q=%E4%BA%8C&q=a+b"
                .parse()
                .unwrap(),
        );
        *request.body_mut() = Some(vec![0, 255, 1, 2].into());
        request
    };
    let mut request = make();
    sign_aws_request(&config, &mut request, time()).unwrap();
    assert_eq!(
        request.headers()["x-amz-security-token"],
        "synthetic-session-token"
    );
    assert!(request.headers().contains_key("x-amz-content-sha256"));
    let first = request.headers()["authorization"].clone();
    let mut changed = make();
    *changed.body_mut() = Some(vec![0, 255, 1, 3].into());
    sign_aws_request(&config, &mut changed, time()).unwrap();
    assert_ne!(first, changed.headers()["authorization"]);
    let mut query = make();
    config.location = AuthLocation::Query;
    sign_aws_request(&config, &mut query, time()).unwrap();
    let pairs = query.url().query_pairs().collect::<Vec<_>>();
    assert_eq!(pairs.iter().filter(|(key, _)| key == "q").count(), 2);
    assert!(pairs.iter().any(|(key,value)|key=="X-Amz-Security-Token"&&value=="synthetic-session-token"));
}
fn request() -> RequestSpec {
    serde_json::from_value(serde_json::json!({"id":"aws","name":"AWS","method":"GET","url":"https://example.test/","description":"","headers":[],"query":[],"body_kind":"none","body":"","auth":{"kind":"aws","token":"","username":"","password":"","aws":config()},"timeout_ms":30000,"follow_redirects":true,"verify_tls":true,"assertions":[],"examples":[]})).unwrap()
}
#[test]
fn credentials_are_resolved_only_for_active_aws_and_incompatible_protocols_reject() {
    let mut source = request();
    source.auth.aws.as_mut().unwrap().secret_key = "{{missing-secret}}".into();
    source.auth.kind = "none".into();
    assert!(resolve_request(&source, None).is_ok());
    source.auth.kind = "aws".into();
    assert!(resolve_request(&source, None).is_err());
    let mut source = request();
    source.protocol = Protocol::Sse;
    assert!(validate_request(&source, false).is_err());
    let mut dormant = request();
    dormant.auth.kind = "none".into();
    let draft = dormant.auth.aws.as_mut().unwrap();
    draft.region.clear();
    draft.service = "invalid draft service".into();
    draft.expires_seconds = 0;
    assert!(validate_request(&dormant, false).is_ok());
    assert!(resolve_request(&dormant, None).is_ok());
    let mut already_signed = reqwest::Request::new(
        reqwest::Method::GET,
        "https://example.test/".parse().unwrap(),
    );
    already_signed.headers_mut().insert(
        "authorization",
        reqwest::header::HeaderValue::from_static("manual signature"),
    );
    assert!(sign_aws_request(&config(), &mut already_signed, time()).is_err());
    let mut invalid = config();
    invalid.expires_seconds = 0;
    assert!(validate_aws(&invalid, false).is_err());
    invalid = config();
    invalid.region = "us-east-1\nprivate".into();
    assert!(validate_aws(&invalid, false).is_err());
    let mut duplicate = reqwest::Request::new(
        reqwest::Method::GET,
        "https://example.test/?X-Amz-Signature=stale"
            .parse()
            .unwrap(),
    );
    assert!(sign_aws_request(&config(), &mut duplicate, time()).is_err());
}
#[tokio::test]
async fn finite_http_resigns_same_origin_and_never_signs_cross_origin_redirects() {
    use axum::{
        Json, Router,
        http::{HeaderMap, StatusCode, header::LOCATION},
        routing::get,
    };
    use std::sync::Arc;
    let received = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let seen = received.clone();
    let outside = Router::new().route(
        "/outside",
        get(|headers: HeaderMap| async move {
            assert!(!headers.contains_key("authorization"));
            assert!(!headers.contains_key("x-amz-security-token"));
            Json(serde_json::json!({"ok":true}))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let external = format!("http://{}/outside", listener.local_addr().unwrap());
    let external_task = tokio::spawn(async move {
        axum::serve(listener, outside).await.unwrap();
    });
    let target = external.clone();
    let first = seen.clone();
    let second = seen.clone();
    let third = seen.clone();
    let app = Router::new()
        .route(
            "/start",
            get(move |headers: HeaderMap| {
                let first = first.clone();
                async move {
                    first
                        .lock()
                        .await
                        .push(headers["authorization"].to_str().unwrap().to_owned());
                    (StatusCode::TEMPORARY_REDIRECT, [(LOCATION, "/final")])
                }
            }),
        )
        .route(
            "/final",
            get(move |headers: HeaderMap| {
                let second = second.clone();
                async move {
                    second
                        .lock()
                        .await
                        .push(headers["authorization"].to_str().unwrap().to_owned());
                    Json(serde_json::json!({"ok":true}))
                }
            }),
        )
        .route(
            "/cross",
            get(move |headers: HeaderMap| {
                let target = target.clone();
                let third = third.clone();
                async move {
                    third
                        .lock()
                        .await
                        .push(headers["authorization"].to_str().unwrap().to_owned());
                    (StatusCode::TEMPORARY_REDIRECT, [(LOCATION, target)])
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let mut r = request();
    r.url = format!("{base}/start");
    let response = execute(
        &r,
        None,
        NetworkPolicy {
            allow_private_network: true,
        },
    )
    .await
    .unwrap();
    assert_eq!(response.status, 200);
    let signed = received.lock().await;
    assert_eq!(signed.len(), 2);
    assert_ne!(signed[0], signed[1]);
    drop(signed);
    r.url = format!("{base}/cross");
    let response = execute(
        &r,
        None,
        NetworkPolicy {
            allow_private_network: true,
        },
    )
    .await
    .unwrap();
    assert_eq!(response.status, 200);
    server.abort();
    external_task.abort();
}

#[tokio::test]
async fn binary_and_multipart_checksum_matches_actual_wire_bytes_and_s3_path() {
    use axum::{Json, Router, extract::OriginalUri, http::HeaderMap, routing::post};
    use base64::{Engine, engine::general_purpose::STANDARD};
    use sha2::{Digest, Sha256};
    let server=Router::new().route("/a//b",post(|OriginalUri(uri):OriginalUri,headers:HeaderMap,body:axum::body::Bytes|async move{
     assert_eq!(uri.path(),"/a//b");assert_eq!(headers["x-amz-content-sha256"].to_str().unwrap(),hex::encode(Sha256::digest(&body)));
     let content=headers.get("content-type").and_then(|v|v.to_str().ok()).unwrap_or_default();
     if content.starts_with("multipart/form-data") {assert!(content.contains("boundary="));assert!(body.windows(12).any(|part|part==b"field-value!"));assert!(body.windows(3).any(|part|part==[0,255,1]));}else{assert_eq!(body.as_ref(),[0,255,1]);}
     Json(serde_json::json!({"ok":true}))
 }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/a//b", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, server).await.unwrap();
    });
    let mut request = request();
    request.method = "POST".into();
    request.url = url;
    request.auth.aws.as_mut().unwrap().service = "s3".into();
    let file = BinaryBody {
        file_name: "bytes.bin".into(),
        mime: "application/octet-stream".into(),
        base64: Some(STANDARD.encode([0, 255, 1])),
    };
    request.body_kind = "binary".into();
    request.body = serde_json::to_string(&file).unwrap();
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
    request.body_kind = "multipart".into();
    request.body = serde_json::to_string(&MultipartBody {
        parts: vec![
            MultipartPart {
                id: "field".into(),
                name: "field".into(),
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
                value: MultipartValue::File { file },
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
    task.abort();
}
#[tokio::test]
async fn query_presigning_does_not_leak_to_another_origin() {
    use axum::{
        Json, Router,
        extract::OriginalUri,
        http::{StatusCode, header::LOCATION},
        routing::get,
    };
    let sink = Router::new().route(
        "/outside",
        get(|OriginalUri(uri): OriginalUri| async move {
            assert!(
                !uri.query()
                    .unwrap_or("")
                    .to_ascii_lowercase()
                    .contains("x-amz-")
            );
            Json(serde_json::json!({"ok":true}))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target = format!(
        "http://{}/outside?ordinary=kept",
        listener.local_addr().unwrap()
    );
    let sink_task = tokio::spawn(async move {
        axum::serve(listener, sink).await.unwrap();
    });
    let source = Router::new().route(
        "/query",
        get(move |OriginalUri(uri): OriginalUri| {
            let target = target.clone();
            async move {
                let query = url::form_urlencoded::parse(uri.query().unwrap().as_bytes())
                    .collect::<Vec<_>>();
                assert!(query.iter().any(
                    |(key, value)| key == "X-Amz-Security-Token" && value == "presign-session"
                ));
                assert!(query.iter().any(|(key, _)| key == "X-Amz-Signature"));
                (StatusCode::TEMPORARY_REDIRECT, [(LOCATION, target)])
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!(
        "http://{}/query?X-Amz-Ordinary=value",
        listener.local_addr().unwrap()
    );
    let source_task = tokio::spawn(async move {
        axum::serve(listener, source).await.unwrap();
    });
    let mut request = request();
    request.url = endpoint;
    let config = request.auth.aws.as_mut().unwrap();
    config.location = AuthLocation::Query;
    config.session_token = "presign-session".into();
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
    source_task.abort();
    sink_task.abort();
}

#[test]
fn s3_query_presign_matches_independent_postman_aws4_vector() {
    // Independently generated by the aws4 1.13.2 library used by Postman Runtime.
    // S3 query authentication requires UNSIGNED-PAYLOAD even when the source flag is false.
    let mut config = config();
    config.service = "s3".into();
    config.location = AuthLocation::Query;
    let mut request = reqwest::Request::new(
        reqwest::Method::GET,
        "https://examplebucket.s3.amazonaws.com/a//b"
            .parse()
            .unwrap(),
    );
    sign_aws_request(&config, &mut request, time()).unwrap();
    let signature = request
        .url()
        .query_pairs()
        .find(|(key, _)| key == "X-Amz-Signature")
        .unwrap()
        .1
        .to_string();
    assert_eq!(
        signature,
        "8e214675647ba615e4d898c3ccff63cfa2c4796d3eb35f52ca53f1cc0cee72d3"
    );
}
#[test]
fn selecting_network_settings_does_not_allow_unsupported_live_request_signing() {
    let mut r = request();
    r.network = Some(Box::new(moleapi_core::RequestNetwork::default()));
    for protocol in [Protocol::Sse, Protocol::Websocket] {
        r.protocol = protocol;
        assert!(
            validate_request(&r, false)
                .unwrap_err()
                .to_string()
                .contains("finite HTTP")
        );
    }
}
