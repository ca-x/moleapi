#![allow(dead_code)]
pub use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
pub use serde_json::{Value, json};
pub use tower::ServiceExt;

pub async fn call(
    router: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(path);
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let body = if let Some(body) = body {
        builder = builder.header("content-type", "application/json");
        Body::from(body.to_string())
    } else {
        Body::empty()
    };
    let response = router
        .clone()
        .oneshot(builder.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 30 * 1024 * 1024)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| json!({"raw":String::from_utf8_lossy(&bytes)})),
    )
}
pub fn data() -> Value {
    json!({"schema_version":1,"collections":[],"environments":[],"active_environment_id":null})
}
pub fn example_data() -> Value {
    json!({"schema_version":1,"collections":[{"id":"c","name":"Collection","description":"","requests":[{"id":"r","name":"Request","method":"GET","url":"https://example.com/","description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":true,"verify_tls":true,"assertions":[],"examples":[{"id":"e","name":"Example","status":201,"headers":[{"id":"h","key":"x-example","value":"yes","enabled":true},{"id":"h2","key":"connection","value":"x-hop","enabled":true},{"id":"h3","key":"x-hop","value":"blocked","enabled":true}],"body":"example body"}]}]}],"environments":[],"active_environment_id":null})
}
pub fn config(url: String, registration: bool) -> moleapi_server::Config {
    moleapi_server::Config {
        database_url: url,
        setup_token: "setup-secret".into(),
        allow_registration: registration,
        allow_private_network: false,
    }
}
pub async fn register(router: &Router, name: &str) -> String {
    let (status, body) = call(
        router,
        "POST",
        "/api/auth/register",
        None,
        Some(json!({"username":name,"password":"goodpassword123","setup_token":"setup-secret"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["token"].as_str().unwrap().into()
}

pub async fn serve(router: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (url, handle)
}
pub async fn rename(router: &Router, token: Option<&str>, name: &str, revision: i64) -> Value {
    let (status, w) = call(
        router,
        "PUT",
        "/api/workspaces/sync-workspace",
        token,
        Some(json!({"name":name,"data":data(),"expected_revision":revision})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    w
}
