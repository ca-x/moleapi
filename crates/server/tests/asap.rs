mod common;
use common::*;
#[tokio::test]
async fn inherited_environment_asap_fields_sign_and_tokens_are_private_in_history() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("asap.db")).await.unwrap();
    let keys: Value =
        serde_json::from_str(include_str!("../../core/tests/fixtures/asap/keys.json")).unwrap();
    let (base, task) = serve(Router::new().route(
        "/",
        axum::routing::get(|headers: axum::http::HeaderMap| async move {
            axum::Json(json!({"auth":headers["authorization"].to_str().unwrap()}))
        }),
    ))
    .await;
    let mut source = example_data();
    source["collections"][0]["auth"] = json!({"kind":"asap","token":"","username":"","password":"","asap":{"algorithm":"ES512","private_key":"{{private_key}}","key_id":"key-id","issuer":"{{issuer}}","audience":["fixture-api"],"claims_source":"{\"custom\":\"{{subject}}\"}"}});
    source["global_variables"] = json!([{"id":"k","key":"private_key","value":keys["ES512"]["pkcs8"],"enabled":true},{"id":"i","key":"issuer","value":"fixture-issuer","enabled":true},{"id":"s","key":"subject","value":"quoted \" value","enabled":true}]);
    source["collections"][0]["requests"][0]["url"] = json!(base);
    source["collections"][0]["requests"][0]["auth"]["kind"] = json!("inherit");
    source["collections"][0]["requests"][0]["assertions"] = json!([]);
    let (status, w) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"asap","name":"ASAP","data":source})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    let (status, response) = call(
        &router,
        "POST",
        "/api/execute",
        None,
        Some(json!({"workspace_id":"asap","request":w["data"]["collections"][0]["requests"][0]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], 200);
    let body: Value = serde_json::from_str(response["body"].as_str().unwrap()).unwrap();
    let token = body["auth"]
        .as_str()
        .unwrap()
        .strip_prefix("Bearer ")
        .unwrap();
    let (_, history) = call(&router, "GET", "/api/workspaces/asap/history", None, None).await;
    assert!(!history.to_string().contains(token));
    task.abort();
}
