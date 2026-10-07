mod common;
use common::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
#[tokio::test]
async fn offline_oauth2_acquire_select_refresh_history_isolation_and_delete_are_usable() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("oauth2.db")).await.unwrap();
    let counter = Arc::new(AtomicUsize::new(0));
    let calls = counter.clone();
    let app=Router::new().route("/token",axum::routing::post(move |axum::extract::Form(form):axum::extract::Form<std::collections::HashMap<String,String>>|{let calls=calls.clone();async move {
   let index=calls.fetch_add(1,Ordering::SeqCst);assert_eq!(form["client_id"],"client");assert_eq!(form["client_secret"],"scoped-client-secret");
   if index>0 {assert_eq!(form["grant_type"],"refresh_token");assert_eq!(form["refresh_token"],format!("refresh-{index}"));}
   axum::Json(json!({"access_token":format!("access-{}",index+1),"refresh_token":format!("refresh-{}",index+1),"token_type":"Bearer","expires_in":if index==0{0}else{3600}}))
 }})).route("/resource",axum::routing::get(|headers:axum::http::HeaderMap|async move{axum::Json(json!({"auth":headers["authorization"].to_str().unwrap()}))}));
    let (base, server) = serve(app).await;
    let mut data = example_data();
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = json!(format!("{base}/resource"));
    request["assertions"] = json!([]);
    let config = json!({"grant":"client_credentials","client_id":"client","client_secret":"{{client_secret}}","client_auth":"body","token_url":format!("{base}/token")});
    data["global_variables"] = json!([{"id":"private","key":"client_secret","value":"scoped-client-secret","enabled":true}]);
    let (status, w) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"oauth","name":"OAuth","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    let (status, token) = call(
        &router,
        "POST",
        "/api/oauth2/tokens/acquire",
        None,
        Some(json!({"workspace_id":"oauth","config":config,"label":"Development"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{token}");
    assert!(!token.to_string().contains("access-1"));
    assert!(!token.to_string().contains("refresh-1"));
    assert!(!token.to_string().contains("scoped-client-secret"));
    let id = token["id"].as_str().unwrap();
    let mut request = data["collections"][0]["requests"][0].clone();
    request["auth"] =
        json!({"kind":"oauth2","token":"","username":"","password":"","oauth2":config});
    request["auth"]["oauth2"]["token_id"] = json!(id);
    let (status, response) = call(
        &router,
        "POST",
        "/api/execute",
        None,
        Some(json!({"workspace_id":"oauth","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert!(
        response["body"]
            .as_str()
            .unwrap()
            .contains("Bearer access-2")
    );
    assert_eq!(counter.load(Ordering::SeqCst), 2);
    let (_, history) = call(&router, "GET", "/api/workspaces/oauth/history", None, None).await;
    assert!(!history.to_string().contains("access-2"));
    assert!(!history.to_string().contains("refresh-2"));
    let (status, secret) = call(
        &router,
        "GET",
        &format!("/api/workspaces/oauth/oauth2/tokens/{id}/secret"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(secret["refresh_token"], "refresh-2");
    let (status, refreshed) = call(
        &router,
        "POST",
        &format!("/api/workspaces/oauth/oauth2/tokens/{id}/refresh"),
        None,
        Some(json!({"workspace_id":"oauth","config":config})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{refreshed}");
    assert_eq!(counter.load(Ordering::SeqCst), 3);
    let (status, _) = call(
        &router,
        "PATCH",
        &format!("/api/workspaces/oauth/oauth2/tokens/{id}"),
        None,
        Some(json!({"label":"Renamed"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, list) = call(
        &router,
        "GET",
        "/api/workspaces/oauth/oauth2/tokens",
        None,
        None,
    )
    .await;
    assert_eq!(list[0]["label"], "Renamed");
    let (status, other) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"other","name":"Other","data":example_data()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{other}");
    assert_eq!(
        call(
            &router,
            "GET",
            &format!("/api/workspaces/other/oauth2/tokens/{id}/secret"),
            None,
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    request["auth"]["oauth2"]["client_id"] = json!("another-client");
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/execute",
            None,
            Some(json!({"workspace_id":"oauth","request":request}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &router,
            "DELETE",
            &format!("/api/workspaces/oauth/oauth2/tokens/{id}"),
            None,
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &router,
            "GET",
            &format!("/api/workspaces/oauth/oauth2/tokens/{id}/secret"),
            None,
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    server.abort();
}

#[tokio::test]
async fn owner_bound_code_flow_state_once_cancel_and_implicit_tokens_are_usable() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("flows.db")).await.unwrap();
    let mut data = example_data();
    data["global_variables"] = json!([]);
    let (_, w) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"flows","name":"Flows","data":data})),
    )
    .await;
    assert!(w["id"].is_string());
    let(app,_seen)=(Router::new().route("/token",axum::routing::post(|axum::extract::Form(form):axum::extract::Form<std::collections::HashMap<String,String>>|async move{assert_eq!(form["grant_type"],"authorization_code");assert_eq!(form["code"],"approved-code");assert!(form["code_verifier"].len()>=43);axum::Json(json!({"access_token":"code-access","refresh_token":"code-refresh","token_type":"Bearer","expires_in":3600}))})),());
    let (base, server) = serve(app).await;
    let config = json!({"grant":"authorization_code","authorization_url":format!("{base}/authorize"),"token_url":format!("{base}/token"),"redirect_url":"http://127.0.0.1/oauth2/callback","client_id":"native-client","pkce":true});
    let (status, flow) = call(
        &router,
        "POST",
        "/api/oauth2/flows",
        None,
        Some(json!({"workspace_id":"flows","config":config})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{flow}");
    let id = flow["id"].as_str().unwrap();
    let uri = url::Url::parse(flow["authorization_url"].as_str().unwrap()).unwrap();
    let nonce = uri
        .query_pairs()
        .find(|(k, _)| k == "state")
        .unwrap()
        .1
        .into_owned();
    assert_eq!(
        call(
            &router,
            "POST",
            &format!("/api/oauth2/flows/{id}/complete"),
            None,
            Some(json!({"state":"wrong","code":"approved-code"}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (status, completed) = call(
        &router,
        "POST",
        &format!("/api/oauth2/flows/{id}/complete"),
        None,
        Some(json!({"state":nonce,"code":"approved-code"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{completed}");
    assert_eq!(completed["stage"], "completed");
    assert!(!completed.to_string().contains("code-access"));
    assert_eq!(
        call(
            &router,
            "POST",
            &format!("/api/oauth2/flows/{id}/complete"),
            None,
            Some(json!({"state":nonce,"code":"approved-code"}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (_, cancelled) = call(
        &router,
        "POST",
        "/api/oauth2/flows",
        None,
        Some(json!({"workspace_id":"flows","config":config})),
    )
    .await;
    let id = cancelled["id"].as_str().unwrap();
    let uri = url::Url::parse(cancelled["authorization_url"].as_str().unwrap()).unwrap();
    let nonce = uri
        .query_pairs()
        .find(|(k, _)| k == "state")
        .unwrap()
        .1
        .into_owned();
    assert_eq!(
        call(
            &router,
            "POST",
            &format!("/api/oauth2/flows/{id}/cancel"),
            None,
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &router,
            "POST",
            &format!("/api/oauth2/flows/{id}/complete"),
            None,
            Some(json!({"state":nonce,"code":"approved-code"}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let mut implicit = config.clone();
    implicit["grant"] = json!("implicit");
    let (_, flow) = call(
        &router,
        "POST",
        "/api/oauth2/flows",
        None,
        Some(json!({"workspace_id":"flows","config":implicit})),
    )
    .await;
    let id = flow["id"].as_str().unwrap();
    let uri = url::Url::parse(flow["authorization_url"].as_str().unwrap()).unwrap();
    let nonce = uri
        .query_pairs()
        .find(|(k, _)| k == "state")
        .unwrap()
        .1
        .into_owned();
    let(status,complete)=call(&router,"POST",&format!("/api/oauth2/flows/{id}/complete"),None,Some(json!({"state":nonce,"implicit":{"access_token":"implicit-access","token_type":"Bearer","expires_in":3600}}))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(complete["stage"], "completed");
    server.abort();
}

#[tokio::test]
async fn anonymous_hosted_callback_is_single_use_owner_bound_and_secret_free() {
    let dir = tempfile::tempdir().unwrap();
    let router = hosted(config(
        format!(
            "sqlite://{}?mode=rwc",
            dir.path().join("callbacks.db").display()
        ),
        true,
    ))
    .await
    .unwrap();
    let owner = register(&router, "oauthowner").await;
    let other = register(&router, "oauthother").await;
    let (status, _) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"callbacks","name":"Callbacks","data":data()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let config = json!({"grant":"implicit","authorization_url":"https://provider.example/authorize","redirect_url":"https://mole.example/api/oauth2/callback","client_id":"browser-client"});
    let (status, flow) = call(
        &router,
        "POST",
        "/api/oauth2/flows",
        Some(&owner),
        Some(json!({"workspace_id":"callbacks","config":config,"callback_mode":"hosted"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{flow}");
    let url = url::Url::parse(flow["authorization_url"].as_str().unwrap()).unwrap();
    let state = url
        .query_pairs()
        .find(|(k, _)| k == "state")
        .unwrap()
        .1
        .to_string();
    let id = flow["id"].as_str().unwrap();
    let (status, _) = call(
        &router,
        "GET",
        &format!("/api/oauth2/flows/{id}"),
        Some(&other),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let payload = json!({"state":state,"implicit":{"access_token":"private-implicit-access","token_type":"Bearer","expires_in":3600}});
    let (status, _) = call(
        &router,
        "POST",
        "/api/oauth2/callback",
        None,
        Some(json!({"state":"forged","implicit":{"access_token":"forged","token_type":"Bearer"}})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/oauth2/callback")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let body = String::from_utf8(
        to_bytes(response.into_body(), 16384)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(body, "{\"ok\":true}");
    let (status, _) = call(&router, "POST", "/api/oauth2/callback", None, Some(payload)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (_, completed) = call(
        &router,
        "GET",
        &format!("/api/oauth2/flows/{id}"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(completed["stage"], "completed");
    assert!(!completed.to_string().contains("private-implicit-access"));
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/oauth2/callback")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        response.headers()["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("nonce-")
    );
    let html = String::from_utf8(
        to_bytes(response.into_body(), 16384)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(html.contains("history.replaceState"));
    assert!(!html.contains("private-implicit-access"));
}

#[tokio::test]
async fn native_loopback_callback_exchanges_code_and_releases_listener() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("loopback.db")).await.unwrap();
    call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"loopback","name":"Loopback","data":data()})),
    )
    .await;
    let reserve = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = reserve.local_addr().unwrap().port();
    drop(reserve);
    let redirect = format!("http://127.0.0.1:{port}/api/oauth2/callback");
    let expected = redirect.clone();
    let provider = Router::new().route(
        "/token",
        axum::routing::post(
            move |axum::extract::Form(form): axum::extract::Form<
                std::collections::HashMap<String, String>,
            >| {
                let expected = expected.clone();
                async move {
                    assert_eq!(form["redirect_uri"], expected);
                    assert_eq!(form["code"], "loopback-code");
                    assert!(form["code_verifier"].len() >= 43);
                    axum::Json(
                        json!({"access_token":"loopback-private-token","token_type":"Bearer"}),
                    )
                }
            },
        ),
    );
    let (base, server) = serve(provider).await;
    let config = json!({"grant":"authorization_code","authorization_url":format!("{base}/authorize"),"token_url":format!("{base}/token"),"redirect_url":redirect,"client_id":"native","pkce":true});
    let (status, flow) = call(
        &router,
        "POST",
        "/api/oauth2/flows",
        None,
        Some(json!({"workspace_id":"loopback","config":config,"callback_mode":"loopback"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{flow}");
    let state = url::Url::parse(flow["authorization_url"].as_str().unwrap())
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == "state")
        .unwrap()
        .1
        .to_string();
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let wrong = client
        .get(&redirect)
        .query(&[("state", "wrong"), ("code", "wrong")])
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(wrong.contains("failed"));
    let response = client
        .get(&redirect)
        .query(&[("state", state.as_str()), ("code", "loopback-code")])
        .send()
        .await
        .unwrap();
    assert_eq!(response.headers()["cache-control"], "no-store");
    let html = response.text().await.unwrap();
    assert!(html.contains("Authorization completed"));
    assert!(!html.contains("loopback-private-token"));
    let (_, completed) = call(
        &router,
        "GET",
        &format!("/api/oauth2/flows/{}", flow["id"].as_str().unwrap()),
        None,
        None,
    )
    .await;
    assert_eq!(completed["stage"], "completed");
    // Shutdown is graceful, including the response that completed this flow.
    let mut rebound = None;
    for _ in 0..50 {
        if let Ok(listener) = tokio::net::TcpListener::bind(("127.0.0.1", port)).await {
            rebound = Some(listener);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(rebound.is_some());
    server.abort();
}

async fn tls_provider(router: Router) -> (String, tokio::task::JoinHandle<()>) {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(
        vec![cert.cert.der().clone()],
        rustls::pki_types::PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der()).into(),
    )
    .unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "https://localhost:{}",
        listener.local_addr().unwrap().port()
    );
    let task = tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let acceptor = acceptor.clone();
            let router = router.clone();
            tokio::spawn(async move {
                if let Ok(stream) = acceptor.accept(stream).await {
                    let _ = hyper_util::server::conn::auto::Builder::new(
                        hyper_util::rt::TokioExecutor::new(),
                    )
                    .serve_connection(
                        hyper_util::rt::TokioIo::new(stream),
                        hyper_util::service::TowerToHyperService::new(router),
                    )
                    .await;
                }
            });
        }
    });
    (url, task)
}
#[tokio::test]
async fn sdk_introspection_and_tls_revocation_are_scoped_and_block_execution() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("manage.db")).await.unwrap();
    call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"manage","name":"Manage","data":example_data()})),
    )
    .await;
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    let provider=Router::new()
        .route("/token",axum::routing::post(||async{axum::Json(json!({"access_token":"managed-private-access","refresh_token":"managed-private-refresh","token_type":"Bearer"}))}))
        .route("/inspect",axum::routing::post(|axum::extract::Form(form):axum::extract::Form<std::collections::HashMap<String,String>>|async move {assert_eq!(form["token"],"managed-private-access");axum::Json(json!({"active":true,"scope":"read write","exp":2000000000,"username":"private-user","client_id":"private-copy","provider_secret":"managed-private-access"}))}))
        .route("/revoke",axum::routing::post(move |axum::extract::Form(form):axum::extract::Form<std::collections::HashMap<String,String>>|{let seen=seen.clone();async move{assert_eq!(form["token"],"managed-private-refresh");assert_eq!(form["token_type_hint"],"refresh_token");seen.fetch_add(1,Ordering::SeqCst);StatusCode::OK}}));
    let (base, server) = tls_provider(provider).await;
    let config = json!({"grant":"client_credentials","token_url":format!("{base}/token"),"introspection_url":format!("{base}/inspect"),"revocation_url":format!("{base}/revoke"),"client_id":"manage-client"});
    let input = json!({"workspace_id":"manage","config":config,"verify_tls":false});
    let (status, token) = call(
        &router,
        "POST",
        "/api/oauth2/tokens/acquire",
        None,
        Some(input.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{token}");
    let id = token["id"].as_str().unwrap();
    let path = format!("/api/workspaces/manage/oauth2/tokens/{id}");
    let (status, inspection) = call(
        &router,
        "POST",
        &format!("{path}/introspect"),
        None,
        Some(input.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{inspection}");
    assert_eq!(
        inspection,
        json!({"active":true,"expires_at":2000000000_i64,"scopes":["read","write"]})
    );
    let mut mismatch = input.clone();
    mismatch["config"]["client_id"] = json!("wrong");
    let (status, _) = call(
        &router,
        "POST",
        &format!("{path}/introspect"),
        None,
        Some(mismatch),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let mut verified = input.clone();
    verified["verify_tls"] = json!(true);
    let (status, _) = call(
        &router,
        "POST",
        &format!("{path}/revoke"),
        None,
        Some(verified),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(count.load(Ordering::SeqCst), 0);
    let mut revoke = input.clone();
    revoke["refresh_token"] = json!(true);
    let (status, revoked) = call(
        &router,
        "POST",
        &format!("{path}/revoke"),
        None,
        Some(revoke),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{revoked}");
    assert_eq!(revoked["revoked"], true);
    assert_eq!(count.load(Ordering::SeqCst), 1);
    let mut request = example_data()["collections"][0]["requests"][0].clone();
    request["auth"] =
        json!({"kind":"oauth2","token":"","username":"","password":"","oauth2":config});
    request["auth"]["oauth2"]["token_id"] = json!(id);
    let (status, _) = call(
        &router,
        "POST",
        "/api/execute",
        None,
        Some(json!({"workspace_id":"manage","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(
        &router,
        "POST",
        &format!("{path}/refresh"),
        None,
        Some(input),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    server.abort();
}

#[tokio::test]
async fn imported_tokens_remain_private_and_expired_refresh_never_executes() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("import.db")).await.unwrap();
    call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"imported","name":"Imported","data":example_data()})),
    )
    .await;
    let resource = Arc::new(AtomicUsize::new(0));
    let seen = resource.clone();
    let provider = Router::new()
        .route(
            "/token",
            axum::routing::post(|| async {
                axum::Json(
                    json!({"access_token":"already-expired","token_type":"Bearer","expires_in":0}),
                )
            }),
        )
        .route(
            "/resource",
            axum::routing::get(move || {
                let seen = seen.clone();
                async move {
                    seen.fetch_add(1, Ordering::SeqCst);
                    StatusCode::OK
                }
            }),
        );
    let (base, server) = serve(provider).await;
    let config = json!({"grant":"client_credentials","token_url":format!("{base}/token"),"client_id":"import-client"});
    let(status,token)=call(&router,"POST","/api/oauth2/tokens/import",None,Some(json!({"workspace_id":"imported","config":config,"label":"External","token":{"access_token":"imported-access","refresh_token":"imported-refresh","token_type":"Bearer","expires_in":0}}))).await;
    assert_eq!(status, StatusCode::OK, "{token}");
    assert!(!token.to_string().contains("imported-access"));
    assert!(!token.to_string().contains("imported-refresh"));
    let id = token["id"].as_str().unwrap();
    let mut request = example_data()["collections"][0]["requests"][0].clone();
    request["url"] = json!(format!("{base}/resource"));
    request["auth"] =
        json!({"kind":"oauth2","token":"","username":"","password":"","oauth2":config});
    request["auth"]["oauth2"]["token_id"] = json!(id);
    let (status, response) = call(
        &router,
        "POST",
        "/api/execute",
        None,
        Some(json!({"workspace_id":"imported","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{response}");
    assert!(response.to_string().contains("expired token"), "{response}");
    assert_eq!(resource.load(Ordering::SeqCst), 0);
    let (_, export) = call(&router, "GET", "/api/workspaces/imported", None, None).await;
    assert!(!export.to_string().contains("imported-access"));
    server.abort();
}
#[tokio::test]
async fn logout_during_device_start_prevents_late_polling() {
    let dir = tempfile::tempdir().unwrap();
    let mut settings = config(
        format!(
            "sqlite://{}?mode=rwc",
            dir.path().join("device.db").display()
        ),
        false,
    );
    settings.allow_private_network = true;
    let router = hosted(settings).await.unwrap();
    let owner = register(&router, "deviceowner").await;
    call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"device","name":"Device","data":data()})),
    )
    .await;
    let started = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Semaphore::new(0));
    let count = Arc::new(AtomicUsize::new(0));
    let enter = started.clone();
    let barrier = release.clone();
    let calls = count.clone();
    let provider=Router::new().route("/device",axum::routing::post(move||{let enter=enter.clone();let barrier=barrier.clone();async move{enter.notify_one();barrier.acquire().await.unwrap().forget();axum::Json(json!({"device_code":"device-private","user_code":"USER-CODE","verification_uri":"https://issuer.test/device","expires_in":60,"interval":1}))}})).route("/token",axum::routing::post(move||{let calls=calls.clone();async move{calls.fetch_add(1,Ordering::SeqCst);axum::Json(json!({"access_token":"late-device-access","token_type":"Bearer"}))}}));
    let (base, server) = serve(provider).await;
    let caller = router.clone();
    let credential = owner.clone();
    let begin = tokio::spawn(async move {
        call(&caller,"POST","/api/oauth2/flows",Some(&credential),Some(json!({"workspace_id":"device","config":{"grant":"device_code","device_url":format!("{base}/device"),"token_url":format!("{base}/token"),"client_id":"device-client"}}))).await
    });
    tokio::time::timeout(std::time::Duration::from_secs(3), started.notified())
        .await
        .unwrap();
    let (status, _) = call(&router, "POST", "/api/auth/logout", Some(&owner), None).await;
    assert_eq!(status, StatusCode::OK);
    release.add_permits(1);
    let (status, _) = begin.await.unwrap();
    assert!(status == StatusCode::BAD_REQUEST || status == StatusCode::UNAUTHORIZED);
    assert_eq!(count.load(Ordering::SeqCst), 0);
    server.abort();
}
#[tokio::test]
async fn device_authorization_polls_with_sdk_and_workspace_delete_cancels_pending_flow() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("device-native.db")).await.unwrap();
    let (_, workspace) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"device-native","name":"Device","data":data()})),
    )
    .await;
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    let provider=Router::new().route("/device",axum::routing::post(||async{axum::Json(json!({"device_code":"device-code","user_code":"USER-CODE","verification_uri":"https://issuer.test/device","expires_in":60,"interval":1}))})).route("/token",axum::routing::post(move |axum::extract::Form(form):axum::extract::Form<std::collections::HashMap<String,String>>|{let calls=calls.clone();async move{assert_eq!(form["grant_type"],"urn:ietf:params:oauth:grant-type:device_code");assert_eq!(form["device_code"],"device-code");if calls.fetch_add(1,Ordering::SeqCst)==0 {(StatusCode::BAD_REQUEST,axum::Json(json!({"error":"authorization_pending"})))} else {(StatusCode::OK,axum::Json(json!({"access_token":"device-access","token_type":"Bearer"})))}}}));
    let (base, server) = serve(provider).await;
    let input = json!({"workspace_id":"device-native","config":{"grant":"device_code","device_url":format!("{base}/device"),"token_url":format!("{base}/token"),"client_id":"client"}});
    let (status, flow) = call(
        &router,
        "POST",
        "/api/oauth2/flows",
        None,
        Some(input.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{flow}");
    assert_eq!(flow["user_code"], "USER-CODE");
    let mut completed = false;
    for _ in 0..60 {
        let (_, status) = call(
            &router,
            "GET",
            &format!("/api/oauth2/flows/{}", flow["id"].as_str().unwrap()),
            None,
            None,
        )
        .await;
        if status["stage"] == "completed" {
            completed = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(completed);
    assert_eq!(count.load(Ordering::SeqCst), 2);
    let (status, pending) = call(&router, "POST", "/api/oauth2/flows", None, Some(input)).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(
        &router,
        "DELETE",
        "/api/workspaces/device-native",
        None,
        Some(json!({"expected_revision":workspace["revision"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(
        &router,
        "GET",
        &format!("/api/oauth2/flows/{}", pending["id"].as_str().unwrap()),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let before = count.load(Ordering::SeqCst);
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    assert_eq!(count.load(Ordering::SeqCst), before);
    server.abort();
}

#[tokio::test]
async fn token_refresh_is_fenced_across_instances_and_recovers_expired_leases() {
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("multi.db");
    let router = local(&path).await.unwrap();
    let peer = local(&path).await.unwrap();
    call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"multi","name":"Multi","data":data()})),
    )
    .await;
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Semaphore::new(0));
    let calls = Arc::new(AtomicUsize::new(0));
    let event = entered.clone();
    let barrier = release.clone();
    let counter = calls.clone();
    let provider=Router::new().route("/token",axum::routing::post(move||{let event=event.clone();let barrier=barrier.clone();let counter=counter.clone();async move{counter.fetch_add(1,Ordering::SeqCst);event.notify_one();barrier.acquire().await.unwrap().forget();axum::Json(json!({"access_token":"rotated-access","token_type":"Bearer","expires_in":3600}))}}));
    let (base, server) = serve(provider).await;
    let config = json!({"grant":"client_credentials","token_url":format!("{base}/token"),"client_id":"multi-client"});
    let input = json!({"workspace_id":"multi","config":config});
    let(status,token)=call(&router,"POST","/api/oauth2/tokens/import",None,Some(json!({"workspace_id":"multi","config":config,"token":{"access_token":"original-access","token_type":"Bearer"}}))).await;
    assert_eq!(status, StatusCode::OK);
    let id = token["id"].as_str().unwrap().to_owned();
    let endpoint = format!("/api/workspaces/multi/oauth2/tokens/{id}/refresh");
    let caller = router.clone();
    let route = endpoint.clone();
    let payload = input.clone();
    let first =
        tokio::spawn(async move { call(&caller, "POST", &route, None, Some(payload)).await });
    tokio::time::timeout(std::time::Duration::from_secs(3), entered.notified())
        .await
        .unwrap();
    let (status, _) = call(&peer, "POST", &endpoint, None, Some(input.clone())).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    release.add_permits(1);
    assert_eq!(first.await.unwrap().0, StatusCode::OK);
    let database = Database::connect(format!("sqlite://{}?mode=rwc", path.display()))
        .await
        .unwrap();
    database.execute(Statement::from_sql_and_values(DbBackend::Sqlite,"UPDATE documents SET payload=json_set(payload, '$.lease', json(?)), revision=revision+1 WHERE kind='oauth2-token' AND json_extract(payload,'$.id')=?",[json!({"id":"abandoned-lease","expires_at":0}).to_string().into(),id.clone().into()])).await.unwrap();
    release.add_permits(1);
    let (status, renewed) = call(&peer, "POST", &endpoint, None, Some(input)).await;
    assert_eq!(status, StatusCode::OK, "{renewed}");
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(renewed["refreshing"], false);
    server.abort();
}
#[tokio::test]
async fn invalid_refresh_expiry_releases_lease_for_retry() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("invalid-refresh.db")).await.unwrap();
    call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"retry","name":"Retry","data":data()})),
    )
    .await;
    let count = Arc::new(AtomicUsize::new(0));
    let counter = count.clone();
    let provider=Router::new().route("/token",axum::routing::post(move||{let counter=counter.clone();async move{let expiry=if counter.fetch_add(1,Ordering::SeqCst)==0{u64::MAX}else{3600};axum::Json(json!({"access_token":"refreshed-access","token_type":"Bearer","expires_in":expiry}))}}));
    let (base, server) = serve(provider).await;
    let config = json!({"grant":"client_credentials","token_url":format!("{base}/token"),"client_id":"retry-client"});
    let input = json!({"workspace_id":"retry","config":config});
    let(status,token)=call(&router,"POST","/api/oauth2/tokens/import",None,Some(json!({"workspace_id":"retry","config":config,"token":{"access_token":"original-access","token_type":"Bearer"}}))).await;
    assert_eq!(status, StatusCode::OK);
    let endpoint = format!(
        "/api/workspaces/retry/oauth2/tokens/{}/refresh",
        token["id"].as_str().unwrap()
    );
    let (status, _) = call(&router, "POST", &endpoint, None, Some(input.clone())).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (_, list) = call(
        &router,
        "GET",
        "/api/workspaces/retry/oauth2/tokens",
        None,
        None,
    )
    .await;
    assert_eq!(list[0]["refreshing"], false);
    let (status, _) = call(&router, "POST", &endpoint, None, Some(input)).await;
    assert_eq!(status, StatusCode::OK);
    server.abort();
}

#[tokio::test]
async fn device_slow_down_and_expiry_follow_sdk_polling_rules() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("device-rules.db")).await.unwrap();
    call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"device-rules","name":"Device rules","data":data()})),
    )
    .await;
    let times = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let seen = times.clone();
    let provider=Router::new().route("/device",axum::routing::post(||async{axum::Json(json!({"device_code":"slow-device","user_code":"SLOW","verification_uri":"https://issuer.test/device","expires_in":30,"interval":1}))}))
    .route("/short",axum::routing::post(||async{axum::Json(json!({"device_code":"expired-device","user_code":"SHORT","verification_uri":"https://issuer.test/device","expires_in":1,"interval":1}))}))
    .route("/token",axum::routing::post(move |axum::extract::Form(form):axum::extract::Form<std::collections::HashMap<String,String>>|{let seen=seen.clone();async move{if form["device_code"]=="expired-device"{return (StatusCode::BAD_REQUEST,axum::Json(json!({"error":"authorization_pending"})));}let mut times=seen.lock().await;times.push(tokio::time::Instant::now());if times.len()==1{(StatusCode::BAD_REQUEST,axum::Json(json!({"error":"slow_down"})))}else{(StatusCode::OK,axum::Json(json!({"access_token":"slow-access","token_type":"Bearer"})))}}}));
    let (base, server) = serve(provider).await;
    let mut input = json!({"workspace_id":"device-rules","config":{"grant":"device_code","device_url":format!("{base}/device"),"token_url":format!("{base}/token"),"client_id":"rules-client"}});
    let (status, flow) = call(
        &router,
        "POST",
        "/api/oauth2/flows",
        None,
        Some(input.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let mut completed = false;
    for _ in 0..120 {
        let (_, current) = call(
            &router,
            "GET",
            &format!("/api/oauth2/flows/{}", flow["id"].as_str().unwrap()),
            None,
            None,
        )
        .await;
        if current["stage"] == "completed" {
            completed = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(completed);
    let timestamps = times.lock().await;
    assert_eq!(timestamps.len(), 2);
    assert!(timestamps[1].duration_since(timestamps[0]) >= std::time::Duration::from_secs(5));
    drop(timestamps);
    input["config"]["device_url"] = json!(format!("{base}/short"));
    let (status, expired) = call(&router, "POST", "/api/oauth2/flows", None, Some(input)).await;
    assert_eq!(status, StatusCode::OK);
    tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
    let (status, _) = call(
        &router,
        "GET",
        &format!("/api/oauth2/flows/{}", expired["id"].as_str().unwrap()),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (_, tokens) = call(
        &router,
        "GET",
        "/api/workspaces/device-rules/oauth2/tokens",
        None,
        None,
    )
    .await;
    assert_eq!(tokens.as_array().unwrap().len(), 1);
    server.abort();
}
