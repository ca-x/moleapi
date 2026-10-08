use moleapi_core::*;
use std::time::{Duration, UNIX_EPOCH};
fn c() -> OAuth1Auth {
    OAuth1Auth {
        consumer_key: "9djdj82h48djs9d2".into(),
        consumer_secret: "j49sk3j29djd".into(),
        token: "kkk9d7dh3k39sjv7".into(),
        token_secret: "dh893hdasih9".into(),
        nonce: "7d8f3e4a".into(),
        timestamp: "137131201".into(),
        include_version: false,
        ..Default::default()
    }
}
#[test]
fn published_rfc5849_duplicate_query_and_form_vector() {
    let client = reqwest::Client::new();
    let mut request = client
        .post("http://example.com/request?b5=%3D%253D&a3=a&c%40=&a2=r%20b")
        .header("content-type", "application/x-www-form-urlencoded")
        .body("c2&a3=2+q")
        .build()
        .unwrap();
    let private =
        sign_oauth1_request(&c(), &mut request, UNIX_EPOCH + Duration::from_secs(1)).unwrap();
    let header = request.headers()["authorization"].to_str().unwrap();
    assert!(
        header.contains("oauth_signature=\"r6%2FTJjbCOr97%2F%2BUU0NsvSne7s5g%3D\""),
        "{header}"
    );
    assert!(private.iter().any(|v| v == header));
}
#[test]
fn placement_generates_only_oauth_parameters_and_retains_original_body_bytes() {
    for location in [OAuth1Location::Query, OAuth1Location::Body] {
        let mut config = c();
        config.location = location;
        let mut request = reqwest::Client::new()
            .post("http://example.com/request?keep=%2F")
            .header("content-type", "application/x-www-form-urlencoded")
            .body("same=a&same=b&empty=")
            .build()
            .unwrap();
        sign_oauth1_request(&config, &mut request, UNIX_EPOCH + Duration::from_secs(1)).unwrap();
        if location == OAuth1Location::Body {
            let body = std::str::from_utf8(request.body().unwrap().as_bytes().unwrap()).unwrap();
            assert!(body.starts_with("same=a&same=b&empty=&oauth_"), "{body}");
            assert!(!body.contains("keep="));
        } else {
            assert_eq!(
                request
                    .url()
                    .query_pairs()
                    .filter(|(k, _)| k == "keep")
                    .count(),
                1
            );
            assert_eq!(
                request.body().unwrap().as_bytes().unwrap(),
                b"same=a&same=b&empty="
            );
        }
    }
}
#[test]
fn independent_postman_all_seven_algorithms_unicode_duplicates_and_exact_binary_body_hash() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    const RESERVED: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'.')
        .remove(b'_')
        .remove(b'~');
    let vectors: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/oauth1/postman-vectors.json")).unwrap();
    for vector in vectors {
        let config: OAuth1Auth = serde_json::from_value(vector["config"].clone()).unwrap();
        let binary = vector["binary"].as_bool().unwrap();
        let body = if binary {
            STANDARD.decode(vector["body"].as_str().unwrap()).unwrap()
        } else {
            vector["body"].as_str().unwrap().as_bytes().to_vec()
        };
        let mut request = reqwest::Client::new()
            .post(vector["url"].as_str().unwrap())
            .header(
                "content-type",
                if binary {
                    "application/octet-stream"
                } else {
                    "application/x-www-form-urlencoded; charset=utf-8"
                },
            )
            .body(body)
            .build()
            .unwrap();
        sign_oauth1_request(&config, &mut request, UNIX_EPOCH + Duration::from_secs(1)).unwrap();
        let expected =
            percent_encoding::utf8_percent_encode(vector["expected"].as_str().unwrap(), RESERVED)
                .to_string();
        assert!(
            request.headers()["authorization"]
                .to_str()
                .unwrap()
                .contains(&format!("oauth_signature=\"{expected}\"")),
            "{} binary={binary}: {:?}",
            config.algorithm,
            request.headers()
        );
    }
}
#[test]
fn generated_decoded_signatures_are_private_and_invalid_wire_inputs_reject() {
    let mut config = c();
    config.algorithm = "PLAINTEXT".into();
    config.verifier = "verifier/雪".into();
    config.consumer_secret = "secret&unicode雪".into();
    let mut request = reqwest::Client::new()
        .get("https://example.com/")
        .build()
        .unwrap();
    let values =
        sign_oauth1_request(&config, &mut request, UNIX_EPOCH + Duration::from_secs(1)).unwrap();
    assert!(values.contains(&"secret%26unicode%E9%9B%AA&dh893hdasih9".into()));
    assert!(values.contains(&"verifier%2F%E9%9B%AA".into()));
    for url in [
        "https://example.com/?oauth_nonce=collision",
        "https://example.com/?invalid=%FF",
    ] {
        let mut request = reqwest::Client::new().get(url).build().unwrap();
        assert!(
            sign_oauth1_request(&c(), &mut request, UNIX_EPOCH + Duration::from_secs(1)).is_err()
        );
    }
    let mut request = reqwest::Client::new()
        .get("https://example.com/")
        .header("authorization", "manual")
        .build()
        .unwrap();
    assert!(sign_oauth1_request(&c(), &mut request, UNIX_EPOCH + Duration::from_secs(1)).is_err());
}
#[tokio::test]
async fn real_http_placements_and_redirects_use_hop_method_without_cross_origin_auth() {
    use axum::{
        Json, Router,
        extract::OriginalUri,
        http::{HeaderMap, StatusCode},
        routing::{get, post},
    };
    use serde_json::{Value, json};
    async fn echo(
        OriginalUri(uri): OriginalUri,
        method: axum::http::Method,
        headers: HeaderMap,
        body: axum::body::Bytes,
    ) -> Json<Value> {
        Json(
            json!({"method":method.to_string(),"authorization":headers.get("authorization").and_then(|v|v.to_str().ok()),"query":uri.query(),"body":String::from_utf8_lossy(&body)}),
        )
    }
    async fn serve(app: Router) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        (
            base,
            tokio::spawn(async move { axum::serve(listener, app).await.unwrap() }),
        )
    }
    let (other, other_server) = serve(Router::new().route("/echo", post(echo))).await;
    let target = other.clone();
    let (base, server) = serve(
        Router::new()
            .route("/echo", get(echo).post(echo))
            .route(
                "/same",
                post(|| async { (StatusCode::SEE_OTHER, [("location", "/echo")]) }),
            )
            .route(
                "/cross",
                post(move || {
                    let target = format!("{target}/echo");
                    async move { (StatusCode::TEMPORARY_REDIRECT, [("location", target)]) }
                }),
            ),
    )
    .await;
    let mut request:RequestSpec=serde_json::from_value(json!({"id":"r","name":"OAuth1","url":format!("{base}/echo"),"method":"POST","description":"","query":[],"headers":[],"body_kind":"form","body":"original=yes","auth":{"kind":"oauth1","username":"","password":"","token":"","oauth1":c()},"timeout_ms":3000,"verify_tls":true,"follow_redirects":true,"assertions":[],"examples":[]})).unwrap();
    let policy = NetworkPolicy {
        allow_private_network: true,
    };
    for location in [
        OAuth1Location::Header,
        OAuth1Location::Query,
        OAuth1Location::Body,
        OAuth1Location::Automatic,
    ] {
        request.auth.oauth1.as_mut().unwrap().location = location;
        let result = execute(&request, None, policy).await.unwrap();
        let value: Value = serde_json::from_str(&result.body).unwrap();
        match location {
            OAuth1Location::Header => assert!(
                value["authorization"]
                    .as_str()
                    .unwrap()
                    .starts_with("OAuth ")
            ),
            OAuth1Location::Query => assert!(
                value["query"]
                    .as_str()
                    .unwrap()
                    .contains("oauth_signature=")
            ),
            _ => assert!(
                value["body"]
                    .as_str()
                    .unwrap()
                    .starts_with("original=yes&oauth_")
            ),
        }
        request.url = format!("{base}/cross");
        let result = execute(&request, None, policy).await.unwrap();
        let value: Value = serde_json::from_str(&result.body).unwrap();
        assert!(value["authorization"].is_null());
        assert_eq!(value["body"], "original=yes");
        assert!(value["query"].is_null());
        request.url = format!("{base}/echo");
    }
    request.auth.oauth1.as_mut().unwrap().location = OAuth1Location::Header;
    request.url = format!("{base}/same");
    let result = execute(&request, None, policy).await.unwrap();
    let value: Value = serde_json::from_str(&result.body).unwrap();
    assert_eq!(value["method"], "GET");
    assert_eq!(value["body"], "");
    assert!(
        value["authorization"]
            .as_str()
            .unwrap()
            .starts_with("OAuth ")
    );
    server.abort();
    other_server.abort();
}
#[test]
fn templated_algorithm_ignores_inactive_credentials_without_changing_saved_source() {
    use serde_json::json;
    let source:RequestSpec=serde_json::from_value(json!({"id":"r","name":"OAuth1","url":"https://example.com/","method":"GET","description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"oauth1","username":"","password":"","token":"","oauth1":{"consumer_key":"consumer","consumer_secret":"{{secret}}","private_key":"{{unused_pem}}","algorithm":"{{algorithm}}","location":"query","realm":"{{unused_realm}}","token_secret":"{{unused_token_secret}}"}},"timeout_ms":1000,"verify_tls":true,"follow_redirects":true,"assertions":[],"examples":[]})).unwrap();
    let env:Environment=serde_json::from_value(json!({"id":"e","name":"Dev","variables":[{"id":"a","key":"algorithm","value":"HMAC-SHA512","enabled":true},{"id":"s","key":"secret","value":"resolved-secret","enabled":true}]})).unwrap();
    let resolved = resolve_request(&source, Some(&env)).unwrap();
    let c = resolved.auth.oauth1.unwrap();
    assert_eq!(c.algorithm, "HMAC-SHA512");
    assert_eq!(c.consumer_secret, "resolved-secret");
    assert_eq!(c.private_key, "");
    assert_eq!(c.token_secret, "");
    assert_eq!(c.realm, "");
    assert_eq!(source.auth.oauth1.unwrap().private_key, "{{unused_pem}}");
}
