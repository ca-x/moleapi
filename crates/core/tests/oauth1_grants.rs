use axum::{
    Router,
    http::{HeaderMap, StatusCode},
    routing::post,
};
use moleapi_core::*;
async fn serve(router: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    (
        url,
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() }),
    )
}
fn c() -> OAuth1Auth {
    OAuth1Auth {
        consumer_key: "consumer".into(),
        consumer_secret: "consumer-secret".into(),
        algorithm: "HMAC-SHA512".into(),
        nonce: "saved-debug-nonce".into(),
        timestamp: "1".into(),
        token: "stale-token".into(),
        token_secret: "stale-secret".into(),
        token_id: Some("stale-vault-id".into()),
        ..Default::default()
    }
}
const LOCAL: NetworkPolicy = NetworkPolicy {
    allow_private_network: true,
};
#[tokio::test]
async fn signed_request_and_access_token_exchanges_override_stale_credentials_and_debug_values() {
    let app=Router::new().route("/request",post(|headers:HeaderMap,body:String|async move{let auth=headers["authorization"].to_str().unwrap();assert!(auth.contains("oauth_callback=\"oob\""));assert!(!auth.contains("oauth_token="));assert!(!auth.contains("stale")&&!auth.contains("saved-debug-nonce")&&!auth.contains("oauth_timestamp=\"1\""));assert_eq!(body,"scope=read+write");"oauth_token=temporary%2Btoken&oauth_token_secret=temporary%26secret&oauth_callback_confirmed=true"}))
 .route("/access",post(|headers:HeaderMap|async move{let auth=headers["authorization"].to_str().unwrap();assert!(auth.contains("oauth_token=\"temporary%2Btoken\""));assert!(auth.contains("oauth_verifier=\"pin%2F%E9%9B%AA\""));assert!(!auth.contains("oauth_callback="));"oauth_token=access%2Ftoken&oauth_token_secret=access%26secret&ignored_private_identity=not-returned"}));
    let (base, server) = serve(app).await;
    let grant = OAuth1Grant {
        request_token_url: format!("{base}/request"),
        access_token_url: format!("{base}/access"),
        request_params: vec![
            serde_json::from_value(
                serde_json::json!({"id":"s","key":"scope","value":"read write","enabled":true}),
            )
            .unwrap(),
        ],
        ..Default::default()
    };
    let temp = oauth1_request_token(&c(), &grant, "oob", LOCAL, true)
        .await
        .unwrap();
    assert_eq!(temp.token, "temporary+token");
    assert_eq!(temp.secret, "temporary&secret");
    let access = oauth1_access_token(&c(), &grant, &temp, "pin/雪", LOCAL, true)
        .await
        .unwrap();
    assert_eq!(access.token, "access/token");
    assert_eq!(access.secret, "access&secret");
    server.abort();
}
#[tokio::test]
async fn provider_response_limits_duplicates_callback_confirmation_and_redirects_reject() {
    for body in [
        "oauth_token=t&oauth_token=t2&oauth_token_secret=s&oauth_callback_confirmed=true".into(),
        "oauth_token=t&oauth_token_secret=s&oauth_callback_confirmed=false".into(),
        "oauth_token=%FF&oauth_token_secret=s&oauth_callback_confirmed=true".into(),
        "x".repeat(65537),
    ] {
        let app = Router::new().route(
            "/request",
            post(move || {
                let body = body.clone();
                async move { body }
            }),
        );
        let (base, server) = serve(app).await;
        let g = OAuth1Grant {
            request_token_url: format!("{base}/request"),
            ..Default::default()
        };
        assert!(
            oauth1_request_token(&c(), &g, "oob", LOCAL, true)
                .await
                .is_err()
        );
        server.abort();
    }
    let (base, server) = serve(Router::new().route(
        "/request",
        post(|| async {
            (
                StatusCode::FOUND,
                [("location", "/unsafe")],
                "provider-secret",
            )
        }),
    ))
    .await;
    let g = OAuth1Grant {
        request_token_url: format!("{base}/request"),
        ..Default::default()
    };
    let error = oauth1_request_token(&c(), &g, "oob", LOCAL, true)
        .await
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("HTTP 302") && !error.contains("provider-secret"));
    server.abort();
}
