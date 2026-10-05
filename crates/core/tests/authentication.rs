use axum::{
    Json, Router,
    extract::Request,
    http::StatusCode,
    routing::{get, post},
};
use moleapi_core::*;
use serde_json::{Value, json};
fn request(url: String, auth: Value) -> RequestSpec {
    serde_json::from_value(json!({"id":"r","name":"auth","method":"GET","url":url,"description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":auth,"timeout_ms":3000,"verify_tls":true,"follow_redirects":true,"assertions":[],"examples":[]})).unwrap()
}
fn auth(kind: &str, extra: Value) -> Value {
    let mut auth = json!({"kind":kind,"username":"Mufasa","password":"Circle of Life","token":""});
    auth.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    auth
}
async fn serve(router: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    (
        url,
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() }),
    )
}
const LOCAL: NetworkPolicy = NetworkPolicy {
    allow_private_network: true,
};
#[tokio::test]
async fn actual_api_key_header_query_override_and_cross_origin_stripping() {
    let(target,other)=serve(Router::new().route("/",get(|r:Request|async move{Json(json!({"header":r.headers().get("x-api-key").and_then(|v|std::str::from_utf8(v.as_bytes()).ok()),"query":r.uri().query()}))}))).await;
    let redirect = target.clone();
    let(base,server)=serve(Router::new().route("/echo",get(|r:Request|async move{Json(json!({"header":r.headers().get("x-api-key").and_then(|v|std::str::from_utf8(v.as_bytes()).ok()),"query":r.uri().query()}))})).route("/redirect",get(move||{let next=format!("{redirect}/?token=private&keep=1");async move{(StatusCode::TEMPORARY_REDIRECT,[("location",next)])}}))).await;
    for location in ["header", "query"] {
        let mut r = request(
            format!("{base}/echo"),
            auth(
                "apikey",
                json!({"api_key":{"name":"X-API-Key","value":"{{key}}","location":location}}),
            ),
        );
        r.headers.push(Pair {
            id: "manual".into(),
            key: "X-API-Key".into(),
            value: "stale".into(),
            enabled: true,
            secret: None,
            local_value: None,
        });
        r.query.push(Pair {
            id: "manualq".into(),
            key: "X-API-Key".into(),
            value: "stale".into(),
            enabled: true,
            secret: None,
            local_value: None,
        });
        let env:Environment=serde_json::from_value(json!({"id":"e","name":"e","variables":[{"id":"v","key":"key","value":"雪 & private","enabled":true}]})).unwrap();
        let response = execute(&r, Some(&env), LOCAL).await.unwrap();
        assert_eq!(response.status, 200);
        let result: Value = serde_json::from_str(&response.body).unwrap();
        if location == "header" {
            assert_eq!(result["header"], "雪 & private");
        } else {
            let pairs = url::form_urlencoded::parse(result["query"].as_str().unwrap().as_bytes())
                .collect::<Vec<_>>();
            assert_eq!(pairs.iter().filter(|(k, _)| k == "X-API-Key").count(), 1);
            assert_eq!(
                pairs.iter().find(|(k, _)| k == "X-API-Key").unwrap().1,
                "雪 & private"
            );
        }
        r.url = format!("{base}/redirect");
        let response = execute(&r, Some(&env), LOCAL).await.unwrap();
        let result: Value = serde_json::from_str(&response.body).unwrap();
        assert!(result["header"].is_null());
        assert_eq!(result["query"], "keep=1");
    }
    server.abort();
    other.abort();
}
#[tokio::test]
async fn jwt_header_verifies_with_mature_decoder_and_claim_templates_are_exact() {
    let (base, server) = serve(Router::new().route(
        "/",
        get(|r: Request| async move {
            let value = r.headers()["authorization"].to_str().unwrap();
            let token = value.strip_prefix("Bearer ").unwrap();
            let claims = jsonwebtoken::decode::<Value>(
                token,
                &jsonwebtoken::DecodingKey::from_secret(b"signing-key"),
                &jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256),
            )
            .unwrap();
            Json(claims.claims)
        }),
    ))
    .await;
    let cfg = JwtAuth {
        key: "{{secret}}".into(),
        claims_source: r#"{"sub":"{{subject}}","role":"owner"}"#.into(),
        ..Default::default()
    };
    let r = request(base, auth("jwt", json!({"jwt":cfg})));
    let env:Environment=serde_json::from_value(json!({"id":"e","name":"e","variables":[{"id":"s","key":"secret","value":"signing-key","enabled":true,"secret":true},{"id":"n","key":"subject","value":"quoted \" subject 雪","enabled":true}]})).unwrap();
    let response = execute(&r, Some(&env), LOCAL).await.unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&response.body).unwrap()["sub"],
        "quoted \" subject 雪"
    );
    server.abort();
}
#[tokio::test]
async fn real_digest_auth_int_uses_actual_method_query_body_and_private_answers_do_not_serialize() {
    let(base,server)=serve(Router::new().route("/digest",post(|r:Request|async move{let challenge=r#"Digest realm="test",qop="auth-int",algorithm=SHA-256,nonce="nonce",opaque="opaque""#;let header=r.headers().get("authorization").and_then(|v|std::str::from_utf8(v.as_bytes()).ok()).map(str::to_owned);let target=r.uri().to_string();let bytes=axum::body::to_bytes(r.into_body(),5*1024*1024).await.unwrap();if let Some(header)=header{let answer=digest_auth::AuthorizationHeader::parse(&header).unwrap();let mut context=digest_auth::AuthContext::new_with_method("Mufasa","Circle of Life",target,Some(bytes.as_ref()),"POST".into());context.set_custom_cnonce(answer.cnonce.unwrap());let expected=digest_auth::parse(challenge).unwrap().respond(&context).unwrap();assert_eq!(answer.response,expected.response);(StatusCode::OK,Json(json!({"answer":header}))).into_response()}else{(StatusCode::UNAUTHORIZED,[("www-authenticate",challenge)],"").into_response()}}))).await;
    use axum::response::IntoResponse;
    let mut r = request(format!("{base}/digest?foo=bar"), auth("digest", json!({})));
    r.method = "POST".into();
    r.body_kind = "text".into();
    r.body = "actual 雪 body".into();
    let response = execute(&r, None, LOCAL).await.unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(response.private_auth_values.len(), 1);
    assert!(
        serde_json::to_value(&response)
            .unwrap()
            .get("private_auth_values")
            .is_none()
    );
    server.abort();
}

#[test]
fn all_advertised_jwt_signing_algorithms_verify_with_mature_decoder() {
    use jsonwebtoken::{Algorithm, DecodingKey, Validation};
    for algorithm in [
        Algorithm::HS256,
        Algorithm::HS384,
        Algorithm::HS512,
        Algorithm::RS256,
        Algorithm::RS384,
        Algorithm::RS512,
        Algorithm::PS256,
        Algorithm::PS384,
        Algorithm::PS512,
        Algorithm::ES256,
        Algorithm::ES384,
        Algorithm::EdDSA,
    ] {
        let (key, decode) = match algorithm {
            Algorithm::HS256 | Algorithm::HS384 | Algorithm::HS512 => {
                ("test-secret", DecodingKey::from_secret(b"test-secret"))
            }
            Algorithm::RS256
            | Algorithm::RS384
            | Algorithm::RS512
            | Algorithm::PS256
            | Algorithm::PS384
            | Algorithm::PS512 => (
                include_str!("fixtures/auth/rsa-test-private.pem"),
                DecodingKey::from_rsa_pem(include_bytes!("fixtures/auth/rsa-test-public.pem"))
                    .unwrap(),
            ),
            Algorithm::ES256 => (
                include_str!("fixtures/auth/ec256-test-private.pem"),
                DecodingKey::from_ec_pem(include_bytes!("fixtures/auth/ec256-test-public.pem"))
                    .unwrap(),
            ),
            Algorithm::ES384 => (
                include_str!("fixtures/auth/ec384-test-private.pem"),
                DecodingKey::from_ec_pem(include_bytes!("fixtures/auth/ec384-test-public.pem"))
                    .unwrap(),
            ),
            Algorithm::EdDSA => (
                include_str!("fixtures/auth/ed-test-private.pem"),
                DecodingKey::from_ed_pem(include_bytes!("fixtures/auth/ed-test-public.pem"))
                    .unwrap(),
            ),
            _ => unreachable!(),
        };
        let config = JwtAuth {
            algorithm: format!("{algorithm:?}"),
            key: key.into(),
            claims_source: r#"{"sub":"synthetic-user"}"#.into(),
            ..Default::default()
        };
        let token = sign_jwt(&config).unwrap();
        let verified =
            jsonwebtoken::decode::<Value>(&token, &decode, &Validation::new(algorithm)).unwrap();
        assert_eq!(verified.claims["sub"], "synthetic-user");
    }
}
#[test]
fn invalid_keys_algorithms_claims_and_dormant_templates_have_explicit_behavior() {
    let mut cfg = JwtAuth {
        key: "secret".into(),
        algorithm: "none".into(),
        ..Default::default()
    };
    assert!(sign_jwt(&cfg).is_err());
    cfg.algorithm = "RS256".into();
    assert!(sign_jwt(&cfg).is_err());
    cfg.algorithm = "HS256".into();
    cfg.claims_source = "[]".into();
    assert!(sign_jwt(&cfg).is_err());
    let mut r = request("https://example.com".into(), auth("none", json!({})));
    r.auth.jwt = Some(Box::new(JwtAuth {
        key: "{{unused}}".into(),
        claims_source: r#"{"sub":"{{unused}}"}"#.into(),
        ..Default::default()
    }));
    assert!(resolve_request(&r, None).is_ok());
}

#[test]
fn digest_accepts_bounded_dormant_credentials_after_mode_switch() {
    let r = request(
        "https://example.test".into(),
        auth(
            "digest",
            json!({
                "api_key":{"name":"X-API-Key","value":"{{inactive}}","location":"header"},
                "jwt":{"key":"{{inactive}}","claims_source":"invalid dormant JSON"}
            }),
        ),
    );
    validate_request(&r, true).unwrap();
    let resolved = resolve_request(&r, None).unwrap();
    validate_request(&resolved, false).unwrap();
    assert_eq!(
        resolved.auth.jwt.unwrap().claims_source,
        "invalid dormant JSON"
    );
}

#[tokio::test]
async fn byte_replay_materializes_selected_query_auth_and_removes_url_duplicates() {
    let (base, server) = serve(Router::new().route(
        "/echo",
        post(|r: Request| async move { Json(json!({"query":r.uri().query()})) }),
    ))
    .await;
    let mut r = request(
        format!("{base}/echo?access=stale&keep=1&access=old"),
        auth(
            "apikey",
            json!({
                "api_key":{"name":"access","value":"selected","location":"query"}
            }),
        ),
    );
    r.method = "POST".into();
    let response = execute_bytes(&r, vec![0, 255, 65], LOCAL).await.unwrap();
    let result: Value = serde_json::from_str(&response.body).unwrap();
    let pairs = url::form_urlencoded::parse(result["query"].as_str().unwrap().as_bytes())
        .collect::<Vec<_>>();
    assert_eq!(pairs.iter().filter(|(name, _)| name == "access").count(), 1);
    assert!(
        pairs
            .iter()
            .any(|(name, value)| name == "access" && value == "selected")
    );
    assert!(
        pairs
            .iter()
            .any(|(name, value)| name == "keep" && value == "1")
    );
    server.abort();
}

#[test]
fn query_auth_preserves_websocket_urls_and_overrides_encoded_names() {
    for scheme in ["ws", "wss"] {
        let r = request(
            format!("{scheme}://example.test/socket?ac%63ess=old&keep=yes#fragment"),
            auth(
                "apikey",
                json!({
                    "api_key":{"name":"access","value":"selected","location":"query"}
                }),
            ),
        );
        let prepared = prepare_authentication(&r).unwrap();
        let url = url::Url::parse(&prepared.url).unwrap();
        assert_eq!(url.scheme(), scheme);
        assert_eq!(url.query(), Some("keep=yes"));
        assert_eq!(url.fragment(), Some("fragment"));
        assert!(
            prepared.query.iter().any(|row| row.key == "access"
                && row.value == "selected"
                && row.secret == Some(true))
        );
    }
}

#[tokio::test]
async fn digest_stale_challenges_cannot_retry_forever() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let attempts = Arc::new(AtomicUsize::new(0));
    let counter = attempts.clone();
    let (base,server)=serve(Router::new().route("/",get(move ||{
        counter.fetch_add(1,Ordering::SeqCst);
        async move {(StatusCode::UNAUTHORIZED,[("www-authenticate",r#"Digest realm="test", nonce="fixture", algorithm=SHA-256, qop="auth", stale=true"#)])}
    }))).await;
    let response = execute(&request(base, auth("digest", json!({}))), None, LOCAL)
        .await
        .unwrap();
    assert_eq!(response.status, 401);
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
    assert_eq!(response.private_auth_values.len(), 2);
    server.abort();
}
