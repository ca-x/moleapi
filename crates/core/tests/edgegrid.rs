use base64::{Engine, engine::general_purpose::STANDARD};
use moleapi_core::*;
use serde_json::{Value, json};
use std::time::{Duration, UNIX_EPOCH};
#[test]
fn independent_postman_vectors_match_query_headers_binary_prefix_and_put_rules() {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/edgegrid/postman-vectors.json")).unwrap();
    for case in cases {
        let config: EdgeGridAuth = serde_json::from_value(case["config"].clone()).unwrap();
        let mut request = reqwest::Request::new(
            case["method"].as_str().unwrap().parse().unwrap(),
            case["url"].as_str().unwrap().parse().unwrap(),
        );
        for row in case["headers"].as_array().unwrap() {
            request.headers_mut().append(
                row[0]
                    .as_str()
                    .unwrap()
                    .parse::<reqwest::header::HeaderName>()
                    .unwrap(),
                row[1].as_str().unwrap().parse().unwrap(),
            );
        }
        let body = STANDARD
            .decode(case["body_base64"].as_str().unwrap())
            .unwrap();
        *request.body_mut() = Some(body.clone().into());
        let private =
            sign_edgegrid_request(&config, &mut request, UNIX_EPOCH + Duration::from_secs(1))
                .unwrap();
        assert_eq!(
            request.headers()["authorization"].to_str().unwrap(),
            case["expected"].as_str().unwrap(),
            "{}",
            case["name"]
        );
        assert_eq!(request.body().unwrap().as_bytes().unwrap(), body);
        assert!(private.iter().any(|v| case["expected"].as_str() == Some(v)));
    }
}
#[test]
fn source_bounds_injection_duplicate_headers_and_time_format_reject() {
    let mut c: EdgeGridAuth = serde_json::from_value(
        json!({"access_token":"access","client_token":"client","client_secret":"secret"}),
    )
    .unwrap();
    for raw in [
        "2024-01-01T12:00:00Z",
        "20240101T12:00:00+0100",
        "20241301T12:00:00+0000",
    ] {
        c.timestamp = raw.into();
        assert!(validate_edgegrid(&c, false).is_err());
    }
    c.timestamp.clear();
    c.client_token = "client;signature=injected".into();
    assert!(validate_edgegrid(&c, false).is_err());
    c.client_token = "client".into();
    c.headers_to_sign = vec!["X-Custom".into(), "x-custom".into()];
    assert!(validate_edgegrid(&c, false).is_err());
    c.headers_to_sign = vec!["host".into()];
    assert!(validate_edgegrid(&c, false).is_err());
    c.headers_to_sign = vec!["x-custom".into()];
    let mut request = reqwest::Client::new()
        .get("https://example.com/")
        .header("x-custom", "one")
        .header("x-custom", "two")
        .build()
        .unwrap();
    assert!(sign_edgegrid_request(&c, &mut request, UNIX_EPOCH + Duration::from_secs(1)).is_err());
}
#[tokio::test]
async fn real_post_sends_untruncated_bytes_and_redirects_resign_without_cross_origin_credentials() {
    use axum::{
        Json, Router,
        http::{HeaderMap, StatusCode},
        routing::post,
    };
    async fn echo(
        method: axum::http::Method,
        headers: HeaderMap,
        body: axum::body::Bytes,
    ) -> Json<Value> {
        Json(
            json!({"method":method.to_string(),"authorization":headers.get("authorization").and_then(|h|h.to_str().ok()),"body":String::from_utf8_lossy(&body)}),
        )
    }
    async fn serve(app: Router) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        (
            url,
            tokio::spawn(async move { axum::serve(listener, app).await.unwrap() }),
        )
    }
    let (other, other_task) = serve(Router::new().route("/echo", post(echo))).await;
    let target = other.clone();
    let (base, task) = serve(
        Router::new()
            .route("/echo", post(echo).get(echo))
            .route(
                "/same",
                post(|| async { (StatusCode::SEE_OTHER, [("location", "/echo")]) }),
            )
            .route(
                "/cross",
                post(move || {
                    let url = format!("{target}/echo");
                    async move { (StatusCode::TEMPORARY_REDIRECT, [("location", url)]) }
                }),
            ),
    )
    .await;
    let mut r:RequestSpec=serde_json::from_value(json!({"id":"r","name":"EdgeGrid","url":format!("{base}/echo"),"method":"POST","description":"","query":[],"headers":[],"body_kind":"text","body":"prefix-unhashed-tail","auth":{"kind":"edgegrid","token":"","username":"","password":"","edgegrid":{"access_token":"access","client_token":"client","client_secret":"secret","max_body_bytes":3}},"timeout_ms":3000,"verify_tls":true,"follow_redirects":true,"assertions":[],"examples":[]})).unwrap();
    let policy = NetworkPolicy {
        allow_private_network: true,
    };
    let result = execute(&r, None, policy).await.unwrap();
    let value: Value = serde_json::from_str(&result.body).unwrap();
    assert_eq!(value["body"], "prefix-unhashed-tail");
    assert!(
        value["authorization"]
            .as_str()
            .unwrap()
            .starts_with("EG1-HMAC-SHA256 ")
    );
    r.url = format!("{base}/same");
    let result = execute(&r, None, policy).await.unwrap();
    let value: Value = serde_json::from_str(&result.body).unwrap();
    assert_eq!(value["method"], "GET");
    assert_eq!(value["body"], "");
    assert!(
        value["authorization"]
            .as_str()
            .unwrap()
            .starts_with("EG1-HMAC-SHA256 ")
    );
    r.url = format!("{base}/cross");
    let result = execute(&r, None, policy).await.unwrap();
    let value: Value = serde_json::from_str(&result.body).unwrap();
    assert!(value["authorization"].is_null());
    assert_eq!(value["body"], "prefix-unhashed-tail");
    task.abort();
    other_task.abort();
}
