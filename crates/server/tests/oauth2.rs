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
