use axum::{
    Router,
    extract::Request,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use moleapi_core::*;
use serde_json::json;
fn request(url: String) -> RequestSpec {
    serde_json::from_value(json!({"id":"r","name":"request","method":"GET","url":url,"description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":true,"verify_tls":true,"assertions":[],"examples":[]})).unwrap()
}
async fn serve(router: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (url, handle)
}
const LOCAL: NetworkPolicy = NetworkPolicy {
    allow_private_network: true,
};
#[tokio::test]
async fn execute_interpolation_query_headers_body_and_json_assertion() {
    let router = Router::new().route(
        "/echo",
        post(|req: Request| async move {
            assert_eq!(req.headers()["authorization"], "Bearer credential");
            assert_eq!(req.headers()["x-test"], "value");
            assert_eq!(req.uri().query(), Some("search=value"));
            let body = axum::body::to_bytes(req.into_body(), MAX_BODY)
                .await
                .unwrap();
            assert_eq!(body.as_ref(), b"{\"hello\":\"value\"}");
            (
                [("set-cookie", "session=private")],
                "{\"result\":{\"hello\":\"value\"}}",
            )
        }),
    );
    let (url, server) = serve(router).await;
    let mut r = request(format!("{url}/echo"));
    r.method = "POST".into();
    r.body_kind = "json".into();
    r.body = "{\"hello\":\"{{word}}\"}".into();
    r.auth.kind = "bearer".into();
    r.auth.token = "credential".into();
    r.query = vec![Pair {
        id: "q".into(),
        key: "search".into(),
        value: "{{word}}".into(),
        enabled: true,
        secret: None,
    }];
    r.headers = vec![Pair {
        id: "h".into(),
        key: "x-test".into(),
        value: "{{word}}".into(),
        enabled: true,
        secret: None,
    }];
    r.assertions = vec![Assertion {
        id: "a".into(),
        name: "nested value".into(),
        kind: "json".into(),
        target: "/result/hello".into(),
        expected: "\"value\"".into(),
    }];
    let env = Environment {
        id: "e".into(),
        name: "test".into(),
        variables: vec![Pair {
            id: "v".into(),
            key: "word".into(),
            value: "value".into(),
            enabled: true,
            secret: None,
        }],
    };
    let response = execute(&r, Some(&env), LOCAL).await.unwrap();
    assert_eq!(response.status, 200);
    assert!(response.tests[0].passed);
    assert_eq!(
        response
            .headers
            .iter()
            .find(|h| h.key == "set-cookie")
            .unwrap()
            .value,
        "[REDACTED]"
    );
    server.abort();
}
#[tokio::test]
async fn private_network_policy_applies_before_connection_and_redirects() {
    let (url, server) = serve(Router::new().route("/", get(|| async { "ok" }))).await;
    let error = execute(
        &request(format!("{url}/")),
        None,
        NetworkPolicy {
            allow_private_network: false,
        },
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("blocked"));
    assert_eq!(
        execute(&request(format!("{url}/")), None, LOCAL)
            .await
            .unwrap()
            .body,
        "ok"
    );
    server.abort();
}
#[tokio::test]
async fn cross_origin_redirect_strips_credentials_and_post_becomes_get() {
    let (dest, dest_server) = serve(Router::new().route(
        "/target",
        get(|headers: HeaderMap| async move {
            assert!(!headers.contains_key("authorization"));
            assert!(!headers.contains_key("cookie"));
            "done"
        }),
    ))
    .await;
    let location = format!("{dest}/target");
    let (source, source_server) = serve(Router::new().route(
        "/",
        post(move || {
            let location = location.clone();
            async move { (StatusCode::FOUND, [("location", location)]).into_response() }
        }),
    ))
    .await;
    let mut r = request(format!("{source}/"));
    r.method = "POST".into();
    r.body_kind = "text".into();
    r.body = "payload".into();
    r.auth.kind = "bearer".into();
    r.auth.token = "secret".into();
    r.headers.push(Pair {
        id: "h".into(),
        key: "cookie".into(),
        value: "secret=1".into(),
        enabled: true,
        secret: None,
    });
    assert_eq!(execute(&r, None, LOCAL).await.unwrap().body, "done");
    source_server.abort();
    dest_server.abort();
}
#[tokio::test]
async fn preserve_307_method_body_and_follow_redirects_option() {
    let router = Router::new()
        .route(
            "/",
            post(|| async { (StatusCode::TEMPORARY_REDIRECT, [("location", "/target")]) }),
        )
        .route("/target", post(|body: String| async move { body }));
    let (url, server) = serve(router).await;
    let mut r = request(format!("{url}/"));
    r.method = "POST".into();
    r.body_kind = "text".into();
    r.body = "payload".into();
    assert_eq!(execute(&r, None, LOCAL).await.unwrap().body, "payload");
    r.follow_redirects = false;
    assert_eq!(execute(&r, None, LOCAL).await.unwrap().status, 307);
    server.abort();
}
#[tokio::test]
async fn bounds_response_stream_binary_and_timeout() {
    let router = Router::new()
        .route("/large", get(|| async { vec![b'x'; MAX_BODY + 128] }))
        .route("/binary", get(|| async { vec![0xff, 0xfe, 0x00] }))
        .route(
            "/slow",
            get(|| async {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                "late"
            }),
        );
    let (url, server) = serve(router).await;
    let response = execute(&request(format!("{url}/large")), None, LOCAL)
        .await
        .unwrap();
    assert!(response.truncated);
    assert_eq!(response.size_bytes, MAX_BODY);
    assert_eq!(response.body.len(), MAX_BODY);
    let response = execute(&request(format!("{url}/binary")), None, LOCAL)
        .await
        .unwrap();
    assert_eq!(response.body_base64.as_deref(), Some("//4A"));
    let mut r = request(format!("{url}/slow"));
    r.timeout_ms = 20;
    assert!(
        execute(&r, None, LOCAL)
            .await
            .unwrap_err()
            .to_string()
            .contains("timed out")
    );
    server.abort();
}
#[tokio::test]
async fn redirect_scheme_and_loop_fail_closed() {
    let router = Router::new()
        .route(
            "/scheme",
            get(|| async { (StatusCode::FOUND, [("location", "file:///etc/passwd")]) }),
        )
        .route(
            "/loop",
            get(|| async { (StatusCode::FOUND, [("location", "/loop")]) }),
        );
    let (url, server) = serve(router).await;
    assert!(
        execute(&request(format!("{url}/scheme")), None, LOCAL)
            .await
            .is_err()
    );
    assert!(
        execute(&request(format!("{url}/loop")), None, LOCAL)
            .await
            .unwrap_err()
            .to_string()
            .contains("Too many redirects")
    );
    server.abort();
}
#[test]
fn malformed_fields_are_rejected() {
    let mut r = request("https://example.com".into());
    r.headers.push(Pair {
        id: "h".into(),
        key: "x-test".into(),
        value: "bad\r\nInjected: x".into(),
        enabled: true,
        secret: None,
    });
    assert!(validate_request(&r, false).is_err());
    r.headers.clear();
    r.assertions.push(Assertion {
        id: "a".into(),
        name: "invalid".into(),
        kind: "json".into(),
        target: "/x".into(),
        expected: "unquoted string".into(),
    });
    assert!(validate_request(&r, false).is_err());
    r.assertions.clear();
    r.url = "{{unknown}}".into();
    assert!(resolve_request(&r, None).is_err());
}
#[tokio::test]
async fn form_interpolation_preserves_delimiters_and_resolves_encoded_templates() {
    let (url, server) = serve(Router::new().route(
        "/form",
        post(|headers: HeaderMap, body: String| async move {
            assert_eq!(headers["content-type"], "application/x-www-form-urlencoded");
            let fields: Vec<_> = url::form_urlencoded::parse(body.as_bytes()).collect();
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].0, "field");
            assert_eq!(fields[0].1, "a&b=x");
            "ok"
        }),
    ))
    .await;
    let environment = Environment {
        id: "e".into(),
        name: "Form".into(),
        variables: vec![Pair {
            id: "v".into(),
            key: "value".into(),
            value: "a&b=x".into(),
            enabled: true,
            secret: None,
        }],
    };
    let mut r = request(format!("{url}/form"));
    r.method = "POST".into();
    r.body_kind = "form".into();
    for body in ["field={{value}}", "field=%7B%7Bvalue%7D%7D"] {
        r.body = body.into();
        assert_eq!(
            execute(&r, Some(&environment), LOCAL).await.unwrap().body,
            "ok"
        );
    }
    server.abort();
}
