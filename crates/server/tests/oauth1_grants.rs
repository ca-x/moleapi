mod common;
use common::*;
use std::collections::HashMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
struct Provider {
    base: String,
    task: tokio::task::JoinHandle<()>,
    callbacks: Arc<Mutex<HashMap<String, String>>>,
    entered: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
    access_calls: Arc<AtomicUsize>,
}
async fn provider(delayed: bool) -> Provider {
    let callbacks = Arc::new(Mutex::new(HashMap::new()));
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let counter = Arc::new(AtomicUsize::new(0));
    let access_calls = Arc::new(AtomicUsize::new(0));
    let stored = callbacks.clone();
    let request_count = counter.clone();
    let started = entered.clone();
    let unblock = release.clone();
    let calls = access_calls.clone();
    let router=Router::new().route("/request",axum::routing::post(move|headers:axum::http::HeaderMap|{let stored=stored.clone();let counter=request_count.clone();async move{
   let auth=headers["authorization"].to_str().unwrap();assert!(auth.contains("oauth_consumer_key=\"consumer\""));assert!(!auth.contains("stale")&&!auth.contains("oauth_token="));
   let callback=auth.split("oauth_callback=\"").nth(1).unwrap().split('"').next().unwrap();let callback=percent_encoding::percent_decode_str(callback).decode_utf8().unwrap().into_owned();
   let id=format!("temporary-{}",counter.fetch_add(1,Ordering::SeqCst));stored.lock().unwrap().insert(id.clone(),callback);format!("oauth_token={id}&oauth_token_secret=temporary-secret&oauth_callback_confirmed=true")
 }})).route("/access",axum::routing::post(move|headers:axum::http::HeaderMap|{let started=started.clone();let unblock=unblock.clone();let calls=calls.clone();async move{
   let auth=headers["authorization"].to_str().unwrap();assert!(auth.contains("oauth_token=\"temporary-"));assert!(auth.contains("oauth_verifier=\"known-pin\""));assert!(!auth.contains("oauth_callback="));calls.fetch_add(1,Ordering::SeqCst);started.notify_one();if delayed{unblock.notified().await;}
   "oauth_token=private-access-token&oauth_token_secret=private-access-secret&ignored_identity=private-person"
 }})).route("/echo",axum::routing::get(|headers:axum::http::HeaderMap|async move{axum::Json(json!({"auth":headers["authorization"].to_str().unwrap(),"copied_token":"private-access-token","copied_secret":"private-access-secret"}))}));
    let (base, task) = serve(router).await;
    Provider {
        base,
        task,
        callbacks,
        entered,
        release,
        access_calls,
    }
}
fn cfg(base: &str) -> Value {
    json!({"consumer_key":"consumer","consumer_secret":"{{consumer_secret}}","token":"{{inactive_token}}","token_secret":"{{inactive_secret}}","nonce":"{{unused_nonce}}","timestamp":"{{unused_timestamp}}","grant":{"request_token_url":format!("{base}/request"),"authorization_url":format!("{base}/authorize"),"access_token_url":format!("{base}/access"),"callback_url":"oob"}})
}
async fn workspace(router: &Router, token: Option<&str>, p: &Provider) {
    let mut data = example_data();
    data["global_variables"] = json!([{"id":"v","key":"consumer_secret","value":"private-consumer-secret","enabled":true}]);
    data["collections"][0]["requests"][0]["url"] = json!(format!("{}/echo", p.base));
    data["collections"][0]["requests"][0]["assertions"] = json!([]);
    let (status, w) = call(
        router,
        "POST",
        "/api/workspaces",
        token,
        Some(json!({"id":"oauth1-grant","name":"OAuth1","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
}
async fn pending(router: &Router, token: Option<&str>, id: &str) -> Value {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let (status, value) = call(
                router,
                "GET",
                &format!("/api/oauth1/flows/{id}"),
                token,
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{value}");
            if value["stage"] != "requesting" {
                assert_eq!(value["stage"], "pending", "{value}");
                return value;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}
async fn begin(router: &Router, token: Option<&str>, config: Value, mode: &str) -> Value {
    let(status,flow)=call(router,"POST","/api/oauth1/flows",token,Some(json!({"workspace_id":"oauth1-grant","config":config,"label":"Development","callback_mode":mode}))).await;
    assert_eq!(status, StatusCode::OK, "{flow}");
    pending(router, token, flow["id"].as_str().unwrap()).await
}
#[tokio::test]
async fn manual_flow_private_vault_selection_profile_metadata_and_source_are_usable() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("manual.db")).await.unwrap();
    let p = provider(false).await;
    workspace(&router, None, &p).await;
    let mut config = cfg(&p.base);
    let flow = begin(&router, None, config.clone(), "manual").await;
    let id = flow["id"].as_str().unwrap();
    let url = url::Url::parse(flow["authorization_url"].as_str().unwrap()).unwrap();
    let temporary = url
        .query_pairs()
        .find(|(k, _)| k == "oauth_token")
        .unwrap()
        .1
        .into_owned();
    assert_eq!(p.callbacks.lock().unwrap()[&temporary], "oob");
    let (status, _) = call(
        &router,
        "POST",
        &format!("/api/oauth1/flows/{id}/complete"),
        None,
        Some(json!({"verifier":"known-pin","token":"wrong-token"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, result) = call(
        &router,
        "POST",
        &format!("/api/oauth1/flows/{id}/complete"),
        None,
        Some(json!({"verifier":"known-pin"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["stage"], "completed");
    assert!(!result.to_string().contains("private-access"));
    let token_id = result["token_id"].as_str().unwrap();
    let (status, _) = call(
        &router,
        "POST",
        &format!("/api/oauth1/flows/{id}/complete"),
        None,
        Some(json!({"verifier":"known-pin"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(p.access_calls.load(Ordering::SeqCst), 1);
    let base = "/api/workspaces/oauth1-grant/oauth1/tokens";
    let (_, list) = call(&router, "GET", base, None, None).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert!(
        !list.to_string().contains("private-access")
            && !list.to_string().contains("private-person")
    );
    let path = format!("{base}/{token_id}/secret");
    let raw = router
        .clone()
        .oneshot(Request::builder().uri(&path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(raw.headers()["cache-control"], "no-store");
    let (_, secret) = call(&router, "GET", &path, None, None).await;
    assert_eq!(
        secret,
        json!({"token":"private-access-token","secret":"private-access-secret"})
    );
    let (_, w) = call(&router, "GET", "/api/workspaces/oauth1-grant", None, None).await;
    let mut request = w["data"]["collections"][0]["requests"][0].clone();
    config["token_id"] = json!(token_id);
    config["nonce"] = json!("");
    config["timestamp"] = json!("");
    request["auth"] =
        json!({"kind":"oauth1","token":"","username":"","password":"","oauth1":config});
    let (status, response) = call(
        &router,
        "POST",
        "/api/execute",
        None,
        Some(json!({"workspace_id":"oauth1-grant","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert!(
        response["body"]
            .as_str()
            .unwrap()
            .contains("private-access-token")
    );
    let (_, history) = call(
        &router,
        "GET",
        "/api/workspaces/oauth1-grant/history",
        None,
        None,
    )
    .await;
    assert!(
        !history.to_string().contains("private-access-token")
            && !history.to_string().contains("private-access-secret")
    );
    let mut different_scope = request.clone();
    different_scope["auth"]["oauth1"]["grant"]["request_params"] = json!([{"id":"scope","key":"scope","value":"different-scope","enabled":true,"secret":true}]);
    let (status, _) = call(
        &router,
        "POST",
        "/api/execute",
        None,
        Some(json!({"workspace_id":"oauth1-grant","request":different_scope})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    request["auth"]["oauth1"]["consumer_key"] = json!("different-consumer");
    let (status, _) = call(
        &router,
        "POST",
        "/api/execute",
        None,
        Some(json!({"workspace_id":"oauth1-grant","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, rename) = call(
        &router,
        "PATCH",
        &format!("{base}/{token_id}"),
        None,
        Some(json!({"label":"Renamed"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{rename}");
    assert_eq!(rename["label"], "Renamed");
    let (status, result) = call(&router, "DELETE", &format!("{base}/{token_id}"), None, None).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let (status, _) = call(&router, "GET", &path, None, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    p.task.abort();
}
#[tokio::test]
async fn hosted_callback_state_token_denial_and_owner_bound_import_are_private() {
    let dir = tempfile::tempdir().unwrap();
    let mut settings = config(
        format!(
            "sqlite://{}?mode=rwc",
            dir.path().join("hosted.db").display()
        ),
        true,
    );
    settings.allow_private_network = true;
    let router = hosted(settings).await.unwrap();
    let first = register(&router, "oauthfirst").await;
    let second = register(&router, "oauthsecond").await;
    let p = provider(false).await;
    workspace(&router, Some(&first), &p).await;
    workspace(&router, Some(&second), &p).await;
    let mut config = cfg(&p.base);
    config["grant"]["callback_url"] = json!("https://moleapi.example/api/oauth1/callback");
    let flow = begin(&router, Some(&first), config.clone(), "hosted").await;
    let id = flow["id"].as_str().unwrap();
    let auth = url::Url::parse(flow["authorization_url"].as_str().unwrap()).unwrap();
    let temporary = auth
        .query_pairs()
        .find(|(k, _)| k == "oauth_token")
        .unwrap()
        .1
        .into_owned();
    let callback = p.callbacks.lock().unwrap()[&temporary].clone();
    let mut callback = url::Url::parse(&callback).unwrap();
    let state = callback
        .query_pairs()
        .find(|(k, _)| k == "state")
        .unwrap()
        .1
        .into_owned();
    assert_eq!(state.len(), 43);
    let (status, _) = call(
        &router,
        "GET",
        &format!("/api/oauth1/flows/{id}"),
        Some(&second),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    callback
        .query_pairs_mut()
        .append_pair("oauth_token", "wrong")
        .append_pair("oauth_verifier", "known-pin");
    let (_, bad) = call(
        &router,
        "GET",
        &format!("{}?{}", callback.path(), callback.query().unwrap()),
        None,
        None,
    )
    .await;
    assert!(bad["raw"].as_str().unwrap().contains("failed"));
    assert_eq!(p.access_calls.load(Ordering::SeqCst), 0);
    callback.set_query(None);
    callback
        .query_pairs_mut()
        .append_pair("state", &state)
        .append_pair("oauth_token", &temporary)
        .append_pair("oauth_verifier", "known-pin");
    let (_, page) = call(
        &router,
        "GET",
        &format!("{}?{}", callback.path(), callback.query().unwrap()),
        None,
        None,
    )
    .await;
    assert!(
        page["raw"]
            .as_str()
            .unwrap()
            .contains("Authorization completed")
    );
    assert!(
        !page.to_string().contains("private-access") && !page.to_string().contains("known-pin")
    );
    let (_, status) = call(
        &router,
        "GET",
        &format!("/api/oauth1/flows/{id}"),
        Some(&first),
        None,
    )
    .await;
    assert_eq!(status["stage"], "completed");
    let access_id = status["token_id"].as_str().unwrap();
    let (status, _) = call(
        &router,
        "GET",
        &format!("/api/workspaces/oauth1-grant/oauth1/tokens/{access_id}/secret"),
        Some(&second),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let denied = begin(&router, Some(&first), config.clone(), "hosted").await;
    let url = url::Url::parse(denied["authorization_url"].as_str().unwrap()).unwrap();
    let temporary = url
        .query_pairs()
        .find(|(k, _)| k == "oauth_token")
        .unwrap()
        .1
        .into_owned();
    let mut callback = url::Url::parse(&p.callbacks.lock().unwrap()[&temporary]).unwrap();
    callback.query_pairs_mut().append_pair("denied", &temporary);
    let (_, page) = call(
        &router,
        "GET",
        &format!("{}?{}", callback.path(), callback.query().unwrap()),
        None,
        None,
    )
    .await;
    assert!(page["raw"].as_str().unwrap().contains("failed"));
    let (_, result) = call(
        &router,
        "GET",
        &format!("/api/oauth1/flows/{}", denied["id"].as_str().unwrap()),
        Some(&first),
        None,
    )
    .await;
    assert_eq!(result["stage"], "failed");
    config["consumer_secret"] = json!("{{not_required_for_import}}");
    let(status,imported)=call(&router,"POST","/api/oauth1/tokens/import",Some(&first),Some(json!({"workspace_id":"oauth1-grant","config":config,"token":{"token":"imported-private","secret":"imported-secret"}}))).await;
    assert_eq!(status, StatusCode::OK, "{imported}");
    assert!(!imported.to_string().contains("imported-private"));
    p.task.abort();
}
#[tokio::test]
async fn cancelling_or_logging_out_during_access_exchange_never_persists_late_tokens() {
    for logout in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = config(
            format!("sqlite://{}?mode=rwc", dir.path().join("race.db").display()),
            true,
        );
        settings.allow_private_network = true;
        let router = hosted(settings).await.unwrap();
        let token = register(&router, "oauthrace").await;
        let p = provider(true).await;
        workspace(&router, Some(&token), &p).await;
        let flow = begin(&router, Some(&token), cfg(&p.base), "manual").await;
        let id = flow["id"].as_str().unwrap().to_owned();
        let pending_router = router.clone();
        let pending_token = token.clone();
        let flow_id = id.clone();
        let completion = tokio::spawn(async move {
            call(
                &pending_router,
                "POST",
                &format!("/api/oauth1/flows/{flow_id}/complete"),
                Some(&pending_token),
                Some(json!({"verifier":"known-pin"})),
            )
            .await
        });
        p.entered.notified().await;
        if logout {
            let (status, result) =
                call(&router, "POST", "/api/auth/logout", Some(&token), None).await;
            assert_eq!(status, StatusCode::OK, "{result}");
        } else {
            let (status, result) = call(
                &router,
                "POST",
                &format!("/api/oauth1/flows/{id}/cancel"),
                Some(&token),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{result}");
            assert_eq!(result["stage"], "cancelled");
        }
        p.release.notify_one();
        let (_, result) = tokio::time::timeout(std::time::Duration::from_secs(5), completion)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result["stage"], "cancelled");
        let fresh = if logout {
            let (_, login) = call(
                &router,
                "POST",
                "/api/auth/login",
                None,
                Some(json!({"username":"oauthrace","password":"goodpassword123"})),
            )
            .await;
            login["token"].as_str().unwrap().to_owned()
        } else {
            token
        };
        let (_, list) = call(
            &router,
            "GET",
            "/api/workspaces/oauth1-grant/oauth1/tokens",
            Some(&fresh),
            None,
        )
        .await;
        assert_eq!(list, json!([]));
        p.task.abort();
    }
}
#[tokio::test]
async fn native_loopback_callback_occupied_port_and_workspace_deletion_are_real() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("native.db")).await.unwrap();
    let p = provider(false).await;
    workspace(&router, None, &p).await;
    let reservation = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = reservation.local_addr().unwrap().port();
    let mut config = cfg(&p.base);
    config["grant"]["callback_url"] = json!(format!("http://127.0.0.1:{port}/api/oauth1/callback"));
    let (status, _) = call(
        &router,
        "POST",
        "/api/oauth1/flows",
        None,
        Some(json!({"workspace_id":"oauth1-grant","config":config,"callback_mode":"loopback"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    drop(reservation);
    let flow = begin(&router, None, config, "loopback").await;
    let auth = url::Url::parse(flow["authorization_url"].as_str().unwrap()).unwrap();
    let temporary = auth
        .query_pairs()
        .find(|(k, _)| k == "oauth_token")
        .unwrap()
        .1
        .into_owned();
    let mut callback = url::Url::parse(&p.callbacks.lock().unwrap()[&temporary]).unwrap();
    callback
        .query_pairs_mut()
        .append_pair("oauth_token", &temporary)
        .append_pair("oauth_verifier", "known-pin");
    let response = reqwest::Client::builder()
        .no_proxy()
        .build()
        .unwrap()
        .get(callback)
        .send()
        .await
        .unwrap();
    assert_eq!(response.headers()["cache-control"], "no-store");
    let html = response.text().await.unwrap();
    assert!(html.contains("Authorization completed") && !html.contains("private-access"));
    let (_, list) = call(
        &router,
        "GET",
        "/api/workspaces/oauth1-grant/oauth1/tokens",
        None,
        None,
    )
    .await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    let (status, result) = call(
        &router,
        "DELETE",
        "/api/workspaces/oauth1-grant",
        None,
        Some(json!({"expected_revision":1})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let (status, _) = call(
        &router,
        "GET",
        &format!("/api/oauth1/flows/{}", flow["id"].as_str().unwrap()),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    p.task.abort();
}
#[tokio::test]
async fn deleting_workspace_during_exchange_prevents_late_credentials_and_recreation_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("delete.db")).await.unwrap();
    let p = provider(true).await;
    workspace(&router, None, &p).await;
    let flow = begin(&router, None, cfg(&p.base), "manual").await;
    let id = flow["id"].as_str().unwrap().to_owned();
    let running = router.clone();
    let completion = tokio::spawn(async move {
        call(
            &running,
            "POST",
            &format!("/api/oauth1/flows/{id}/complete"),
            None,
            Some(json!({"verifier":"known-pin"})),
        )
        .await
    });
    p.entered.notified().await;
    let (status, result) = call(
        &router,
        "DELETE",
        "/api/workspaces/oauth1-grant",
        None,
        Some(json!({"expected_revision":1})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    p.release.notify_one();
    let (_, result) = tokio::time::timeout(std::time::Duration::from_secs(5), completion)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result["stage"], "cancelled");
    workspace(&router, None, &p).await;
    let (_, tokens) = call(
        &router,
        "GET",
        "/api/workspaces/oauth1-grant/oauth1/tokens",
        None,
        None,
    )
    .await;
    assert_eq!(tokens, json!([]));
    p.task.abort();
}
