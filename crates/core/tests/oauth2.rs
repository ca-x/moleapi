use axum::{
    Json, Router,
    extract::{Form, State},
    http::HeaderMap,
    routing::post,
};
use moleapi_core::*;
use oauth2::TokenResponse;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
type Captures = Arc<Mutex<Vec<(HeaderMap, HashMap<String, String>)>>>;
async fn token(
    State(seen): State<Captures>,
    headers: HeaderMap,
    Form(fields): Form<HashMap<String, String>>,
) -> Json<Value> {
    seen.lock().unwrap().push((headers, fields.clone()));
    Json(
        json!({"access_token":format!("issued-{}",fields["grant_type"]),"refresh_token":"rotated-refresh","token_type":"Bearer","expires_in":3600,"scope":"read write"}),
    )
}
async fn server() -> (String, Captures, tokio::task::JoinHandle<()>) {
    let seen = Arc::new(Mutex::new(vec![]));
    let app = Router::new()
        .route("/token", post(token))
        .with_state(seen.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    (
        url,
        seen,
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() }),
    )
}
const LOCAL: NetworkPolicy = NetworkPolicy {
    allow_private_network: true,
};
fn config(url: &str) -> OAuth2Auth {
    OAuth2Auth {
        token_url: format!("{url}/token"),
        client_id: "client & 雪".into(),
        client_secret: "secret : + 空格".into(),
        scopes: vec!["read".into(), "write".into()],
        ..Default::default()
    }
}
#[tokio::test]
async fn mature_sdk_client_credentials_password_and_rotating_refresh_send_exact_parameters() {
    use base64::Engine;
    let (url, seen, task) = server().await;
    let mut cfg = config(&url);
    cfg.grant = OAuth2Grant::ClientCredentials;
    let response = oauth2_acquire(&cfg, LOCAL, true).await.unwrap();
    assert_eq!(
        response.access_token().secret(),
        "issued-client_credentials"
    );
    assert_eq!(
        response.refresh_token().unwrap().secret(),
        "rotated-refresh"
    );
    cfg.client_auth = OAuth2ClientAuth::Body;
    cfg.grant = OAuth2Grant::Password;
    cfg.username = "user 雪 +".into();
    cfg.password = "pass & +".into();
    oauth2_acquire(&cfg, LOCAL, true).await.unwrap();
    oauth2_refresh(&cfg, "original-refresh", LOCAL, true)
        .await
        .unwrap();
    let data = seen.lock().unwrap();
    assert_eq!(data.len(), 3);
    let encode =
        |text: &str| url::form_urlencoded::byte_serialize(text.as_bytes()).collect::<String>();
    assert_eq!(
        data[0].0["authorization"].to_str().unwrap(),
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!(
                "{}:{}",
                encode(&cfg.client_id),
                encode(&cfg.client_secret)
            ))
        )
    );
    assert!(!data[0].1.contains_key("client_secret"));
    assert_eq!(data[0].1["scope"], "read write");
    assert_eq!(data[1].1["client_id"], cfg.client_id);
    assert_eq!(data[1].1["client_secret"], cfg.client_secret);
    assert_eq!(data[1].1["username"], cfg.username);
    assert_eq!(data[1].1["password"], cfg.password);
    assert_eq!(data[2].1["refresh_token"], "original-refresh");
    task.abort();
}
#[tokio::test]
async fn actual_pkce_authorization_code_exchange_uses_sdk_state_challenge_and_verifier() {
    let (url, seen, task) = server().await;
    let mut cfg = config(&url);
    cfg.authorization_url = format!("{url}/authorize");
    cfg.redirect_url = "http://127.0.0.1/oauth/callback".into();
    let authorization = oauth2_authorize(&cfg).unwrap();
    let params = url::Url::parse(&authorization.url)
        .unwrap()
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect::<HashMap<_, _>>();
    assert_eq!(params["response_type"], "code");
    assert_eq!(params["state"], *authorization.state.secret());
    assert_eq!(params["code_challenge_method"], "S256");
    let verifier = authorization.verifier.unwrap();
    let challenge = oauth2::PkceCodeChallenge::from_code_verifier_sha256(&verifier);
    assert_eq!(params["code_challenge"], *challenge.as_str());
    let original = verifier.secret().clone();
    let response = oauth2_exchange_code(&cfg, "code + 雪", Some(verifier), LOCAL, true)
        .await
        .unwrap();
    assert_eq!(
        response.access_token().secret(),
        "issued-authorization_code"
    );
    let data = seen.lock().unwrap();
    assert_eq!(data[0].1["code"], "code + 雪");
    assert_eq!(data[0].1["code_verifier"], original);
    assert_eq!(data[0].1["redirect_uri"], cfg.redirect_url);
    task.abort();
}
#[test]
fn implicit_url_reserved_parameters_and_dormant_configs_have_explicit_behavior() {
    let mut cfg = config("https://example.test");
    cfg.grant = OAuth2Grant::Implicit;
    cfg.authorization_url = "https://example.test/authorize".into();
    cfg.redirect_url = "moleapi://oauth/callback".into();
    let auth = oauth2_authorize(&cfg).unwrap();
    let url = url::Url::parse(&auth.url).unwrap();
    assert!(
        url.query_pairs()
            .any(|(k, v)| k == "response_type" && v == "token")
    );
    assert!(auth.verifier.is_none());
    cfg.token_params.push(Pair {
        id: "bad".into(),
        key: "grant_type".into(),
        value: "evil".into(),
        enabled: true,
        secret: None,
        local_value: None,
    });
    assert!(validate_oauth2(&cfg, false, true).is_err());
    cfg.token_params.clear();
    cfg.authorization_url = "https://example.test/authorize?state=stale".into();
    assert!(oauth2_authorize(&cfg).is_err());
    let mut request:RequestSpec=serde_json::from_value(json!({"id":"r","name":"OAuth","method":"GET","url":"https://example.test","description":"","headers":[],"query":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":"","oauth2":{"client_secret":"{{inactive}}","authorization_url":"{{inactive}}"}},"timeout_ms":3000,"verify_tls":true,"follow_redirects":true,"assertions":[],"examples":[]})).unwrap();
    assert!(resolve_request(&request, None).is_ok());
    request.auth.kind = "oauth2".into();
    assert!(prepare_authentication(&request).is_err());
}
#[tokio::test]
async fn provider_error_and_redirect_cannot_expose_or_forward_client_secrets() {
    let app = Router::new()
        .route(
            "/error",
            post(|| async {
                (
                    axum::http::StatusCode::BAD_REQUEST,
                    Json(json!({"error":"invalid_client","error_description":"secret : + 空格"})),
                )
            }),
        )
        .route(
            "/redirect",
            post(|| async {
                (
                    axum::http::StatusCode::TEMPORARY_REDIRECT,
                    [("location", "http://127.0.0.1:1/leak")],
                )
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut cfg = config(&url);
    cfg.grant = OAuth2Grant::ClientCredentials;
    for path in ["error", "redirect"] {
        cfg.token_url = format!("{url}/{path}");
        let error = oauth2_acquire(&cfg, LOCAL, true)
            .await
            .unwrap_err()
            .to_string();
        assert!(!error.contains(&cfg.client_secret));
    }
    cfg.token_url = format!("{url}/error");
    assert!(
        oauth2_acquire(
            &cfg,
            NetworkPolicy {
                allow_private_network: false
            },
            true
        )
        .await
        .is_err()
    );
    task.abort();
}
