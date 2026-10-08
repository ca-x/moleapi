use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use moleapi_core::*;
use serde_json::{Value, json};
use std::time::{Duration, UNIX_EPOCH};
fn keys() -> Value {
    serde_json::from_str(include_str!("fixtures/asap/keys.json")).unwrap()
}
fn config(algorithm: &str) -> AsapAuth {
    let family = if algorithm.starts_with("ES") {
        algorithm
    } else {
        "RSA"
    };
    AsapAuth {
        algorithm: algorithm.into(),
        private_key: keys()[family]["pkcs8"].as_str().unwrap().into(),
        key_id: format!("fixture-{algorithm}"),
        issuer: "fixture-issuer".into(),
        audience: vec!["fixture-api".into(), "fixture-other".into()],
        claims_source: r#"{"iat":1700000000,"exp":1700003600,"jti":"fixture-id","custom":"claim"}"#
            .into(),
        ..Default::default()
    }
}
fn decode(token: &str) -> (Value, Value) {
    let parts: Vec<_> = token.split('.').collect();
    (
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[0]).unwrap()).unwrap(),
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[1]).unwrap()).unwrap(),
    )
}
#[test]
fn nine_algorithms_generate_valid_shape_and_preserve_claims_for_independent_verification() {
    let mut tokens = Vec::new();
    for algorithm in [
        "RS256", "RS384", "RS512", "PS256", "PS384", "PS512", "ES256", "ES384", "ES512",
    ] {
        let c = config(algorithm);
        let token = sign_asap(&c, UNIX_EPOCH + Duration::from_secs(1700000000)).unwrap();
        let (header, claims) = decode(&token);
        assert_eq!(
            header,
            json!({"alg":algorithm,"kid":format!("fixture-{algorithm}")})
        );
        assert_eq!(claims["iss"], "fixture-issuer");
        assert_eq!(claims["sub"], "fixture-issuer");
        assert_eq!(claims["aud"], json!(["fixture-api", "fixture-other"]));
        assert_eq!(claims["custom"], "claim");
        assert_eq!(claims["iat"], 1700000000u64);
        assert_eq!(claims["exp"], 1700003600u64);
        tokens.push(json!({"algorithm":algorithm,"token":token}));
    }
    if let Ok(file) = std::env::var("MOLEAPI_ASAP_CAPTURE") {
        std::fs::write(file, serde_json::to_vec_pretty(&tokens).unwrap()).unwrap();
    }
}
#[test]
fn key_formats_and_data_uri_kid_matching_use_mature_decoders() {
    for algorithm in ["RS256", "ES256", "ES384", "ES512"] {
        let mut c = config(algorithm);
        let family = if algorithm.starts_with("ES") {
            algorithm
        } else {
            "RSA"
        };
        c.private_key = keys()[family]["traditional"].as_str().unwrap().into();
        assert!(sign_asap(&c, UNIX_EPOCH + Duration::from_secs(1700000000)).is_ok());
        c.private_key = format!(
            "data:application/pkcs8;kid={};base64,{}",
            c.key_id,
            keys()[family]["der_base64"].as_str().unwrap()
        );
        assert!(sign_asap(&c, UNIX_EPOCH + Duration::from_secs(1700000000)).is_ok());
        c.key_id = "wrong-id".into();
        assert!(sign_asap(&c, UNIX_EPOCH + Duration::from_secs(1700000000)).is_err());
    }
    let mut c = config("RS256");
    c.private_key = keys()["ES256"]["pkcs8"].as_str().unwrap().into();
    assert!(sign_asap(&c, UNIX_EPOCH + Duration::from_secs(1)).is_err());
    c.algorithm = "HS256".into();
    assert!(sign_asap(&c, UNIX_EPOCH + Duration::from_secs(1)).is_err());
}
#[test]
fn claims_override_unused_templates_without_corrupting_quoted_values_or_source() {
    let mut c = config("RS256");
    c.issuer = "{{unused_issuer}}".into();
    c.subject = "{{unused_subject}}".into();
    c.key_id = "{{unused_kid}}".into();
    c.audience = vec!["{{unused_audience}}".into()];
    c.claims_source=r#"{"iss":"issuer-override","sub":"subject-override","kid":"kid-override","aud":["audience-override"],"extra":"{{quoted}}"}"#.into();
    let source:RequestSpec=serde_json::from_value(json!({"id":"r","name":"ASAP","method":"GET","url":"https://example.com/","description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"asap","username":"","password":"","token":"","asap":c},"timeout_ms":1000,"verify_tls":true,"follow_redirects":true,"assertions":[],"examples":[]})).unwrap();
    let env:Environment=serde_json::from_value(json!({"id":"e","name":"Dev","variables":[{"id":"q","key":"quoted","value":"quoted \" value 雪","enabled":true}]})).unwrap();
    let resolved = resolve_request(&source, Some(&env)).unwrap();
    let c = resolved.auth.asap.unwrap();
    let (header, claims) = decode(&sign_asap(&c, UNIX_EPOCH + Duration::from_secs(10)).unwrap());
    assert_eq!(header["kid"], "kid-override");
    assert_eq!(claims["sub"], "subject-override");
    assert_eq!(claims["extra"], "quoted \" value 雪");
    assert_eq!(claims["exp"], 3610u64);
    assert_eq!(source.auth.asap.unwrap().issuer, "{{unused_issuer}}");
}
#[tokio::test]
async fn real_bearer_send_and_redirects_regenerate_jti_without_cross_origin_tokens() {
    use axum::{
        Json, Router,
        http::{HeaderMap, StatusCode},
        routing::{get, post},
    };
    async fn echo(method: axum::http::Method, headers: HeaderMap) -> Json<Value> {
        Json(
            json!({"method":method.to_string(),"auth":headers.get("authorization").and_then(|v|v.to_str().ok())}),
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
    let (other, other_task) = serve(Router::new().route("/", post(echo))).await;
    let target = other.clone();
    let (base, task) = serve(
        Router::new()
            .route("/resource", get(echo).post(echo))
            .route(
                "/same",
                post(|| async { (StatusCode::SEE_OTHER, [("location", "/resource")]) }),
            )
            .route(
                "/cross",
                post(move || {
                    let url = target.clone();
                    async move { (StatusCode::TEMPORARY_REDIRECT, [("location", url)]) }
                }),
            ),
    )
    .await;
    let mut c = config("ES512");
    c.claims_source = "{}".into();
    let mut r:RequestSpec=serde_json::from_value(json!({"id":"r","name":"ASAP","url":format!("{base}/resource"),"method":"POST","description":"","query":[],"headers":[],"body_kind":"json","body":"{}","auth":{"kind":"asap","token":"","username":"","password":"","asap":c},"timeout_ms":3000,"verify_tls":true,"follow_redirects":true,"assertions":[],"examples":[]})).unwrap();
    let policy = NetworkPolicy {
        allow_private_network: true,
    };
    let a = execute(&r, None, policy).await.unwrap();
    let b = execute(&r, None, policy).await.unwrap();
    let first: Value = serde_json::from_str(&a.body).unwrap();
    let second: Value = serde_json::from_str(&b.body).unwrap();
    let token = first["auth"]
        .as_str()
        .unwrap()
        .strip_prefix("Bearer ")
        .unwrap();
    let other = second["auth"]
        .as_str()
        .unwrap()
        .strip_prefix("Bearer ")
        .unwrap();
    assert_ne!(decode(token).1["jti"], decode(other).1["jti"]);
    assert!(a.private_auth_values.contains(&token.into()));
    r.url = format!("{base}/same");
    let response = execute(&r, None, policy).await.unwrap();
    let value: Value = serde_json::from_str(&response.body).unwrap();
    assert_eq!(value["method"], "GET");
    assert!(value["auth"].as_str().unwrap().starts_with("Bearer "));
    r.url = format!("{base}/cross");
    let response = execute(&r, None, policy).await.unwrap();
    assert!(serde_json::from_str::<Value>(&response.body).unwrap()["auth"].is_null());
    task.abort();
    other_task.abort();
}
#[test]
fn unsafe_algorithm_missing_identity_and_invalid_numeric_claims_reject() {
    let mut c = config("RS256");
    c.algorithm = "none".into();
    assert!(sign_asap(&c, UNIX_EPOCH + Duration::from_secs(10)).is_err());
    c = config("RS256");
    c.issuer.clear();
    assert!(sign_asap(&c, UNIX_EPOCH + Duration::from_secs(10)).is_err());
    c = config("RS256");
    c.claims_source = r#"{"iat":9007199254740992}"#.into();
    assert!(sign_asap(&c, UNIX_EPOCH + Duration::from_secs(10)).is_err());
    c.claims_source = r#"{"aud":[]}"#.into();
    assert!(sign_asap(&c, UNIX_EPOCH + Duration::from_secs(10)).is_err());
    c = config("RS256");
    c.claims_source = r#"{"iat":10,"exp":5}"#.into();
    assert!(
        sign_asap(&c, UNIX_EPOCH + Duration::from_secs(10)).is_ok(),
        "explicitly expired token remains testable"
    );
}
