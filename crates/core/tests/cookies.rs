use axum::{
    Router,
    http::{HeaderMap, StatusCode},
    routing::get,
};
use moleapi_core::*;
use serde_json::json;
fn request(url: String) -> RequestSpec {
    serde_json::from_value(json!({"id":"r","name":"cookies","url":url,"method":"GET","description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","username":"","password":"","token":""},"timeout_ms":3000,"verify_tls":true,"follow_redirects":true,"assertions":[],"examples":[]})).unwrap()
}
#[test]
fn cookie_sdk_rules_prefixes_public_suffixes_limits_and_masking() {
    let jar = CookieJar::default();
    let url = url::Url::parse("https://api.example.com/a/resource").unwrap();
    jar.insert(&url, "sid=private; Path=/a; HttpOnly; Secure; SameSite=Lax")
        .unwrap();
    assert!(
        jar.snapshot(false)
            .cookies
            .iter()
            .all(|c| c.value == "[REDACTED]")
    );
    assert_eq!(jar.snapshot(true).cookies[0].value, "private");
    assert!(jar.insert(&url, "bad=x; Domain=com").is_err());
    assert!(jar.insert(&url, "bad=x; Domain=other.example.com").is_err());
    assert!(
        jar.insert(&url, "__Host-invalid=x; Secure; Path=/a")
            .is_err()
    );
    assert!(jar.insert(&url, "__Secure-invalid=x").is_err());
    assert!(
        jar.insert(&url, &format!("large={}", "x".repeat(8192)))
            .is_err()
    );
    assert!(
        jar.insert(
            &url::Url::parse("https://com/").unwrap(),
            "bad=x; Domain=com"
        )
        .is_err()
    );
    let http = url::Url::parse("http://api.example.com/a/resource").unwrap();
    assert!(jar.insert(&http, "sid=overwrite; Path=/a").is_err());
    jar.insert(&url, "sid=; Path=/a; Max-Age=0").unwrap();
    assert!(jar.snapshot(true).cookies.is_empty());
}
#[tokio::test]
async fn actual_redirect_cookie_path_expiry_manual_override_disable_and_response_privacy() {
    let app = Router::new()
        .route(
            "/login",
            get(|| async {
                (
                    StatusCode::FOUND,
                    [
                        ("location", "/a/echo"),
                        ("set-cookie", "sid=private-session; Path=/a; HttpOnly"),
                    ],
                    "",
                )
            }),
        )
        .route(
            "/a/echo",
            get(|headers: HeaderMap| async move {
                headers
                    .get("cookie")
                    .and_then(|h| h.to_str().ok())
                    .unwrap_or("missing")
                    .to_owned()
            }),
        )
        .route(
            "/echo",
            get(|headers: HeaderMap| async move {
                headers
                    .get("cookie")
                    .and_then(|h| h.to_str().ok())
                    .unwrap_or("missing")
                    .to_owned()
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let jar = CookieJar::default();
    jar.configure(true, false);
    let policy = NetworkPolicy {
        allow_private_network: true,
    };
    let response =
        execute_with_cookies(&request(format!("{base}/login")), None, policy, Some(&jar))
            .await
            .unwrap();
    assert_eq!(response.body, "sid=private-session");
    assert!(
        response
            .private_auth_values
            .contains(&"private-session".into())
    );
    let response = execute_with_cookies(&request(format!("{base}/echo")), None, policy, Some(&jar))
        .await
        .unwrap();
    assert_eq!(response.body, "missing");
    let mut r = request(format!("{base}/a/echo"));
    r.headers.push(
        serde_json::from_value(
            json!({"id":"manual","key":"Cookie","value":"manual=yes","enabled":true}),
        )
        .unwrap(),
    );
    assert_eq!(
        execute_with_cookies(&r, None, policy, Some(&jar))
            .await
            .unwrap()
            .body,
        "manual=yes"
    );
    r.headers.clear();
    jar.configure(false, false);
    assert_eq!(
        execute_with_cookies(&r, None, policy, Some(&jar))
            .await
            .unwrap()
            .body,
        "missing"
    );
    jar.configure(true, true);
    assert!(jar.snapshot(true).cookies.is_empty());
    server.abort();
}

#[tokio::test]
async fn clearing_jar_fences_an_in_flight_set_cookie_response() {
    use std::sync::Arc;
    let started = Arc::new(tokio::sync::Notify::new());
    let finish = Arc::new(tokio::sync::Notify::new());
    let start = started.clone();
    let release = finish.clone();
    let app = Router::new().route(
        "/",
        get(move || {
            let start = start.clone();
            let release = release.clone();
            async move {
                start.notify_one();
                release.notified().await;
                ([("set-cookie", "late=private; Path=/")], "done")
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let jar = Arc::new(CookieJar::default());
    jar.configure(true, false);
    let running = jar.clone();
    let execution = tokio::spawn(async move {
        execute_with_cookies(
            &request(url),
            None,
            NetworkPolicy {
                allow_private_network: true,
            },
            Some(&running),
        )
        .await
        .unwrap()
    });
    started.notified().await;
    jar.configure(true, true);
    finish.notify_one();
    let response = execution.await.unwrap();
    assert_eq!(response.body, "done");
    assert!(jar.snapshot(true).cookies.is_empty());
    assert!(response.private_auth_values.contains(&"private".into()));
    server.abort();
}
