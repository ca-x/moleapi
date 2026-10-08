mod common;
use axum::Json;
use common::*;
#[tokio::test]
async fn inherited_environment_oauth1_signing_and_signature_echoes_are_private_in_history() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("oauth1.db")).await.unwrap();
    let(base,server)=serve(Router::new().route("/echo",axum::routing::get(|headers:axum::http::HeaderMap|async move{Json(json!({"auth":headers["authorization"].to_str().unwrap(),"decoded_signature":"environment%26secret&token%2Fsecret"}))}))).await;
    let mut source = example_data();
    source["collections"][0]["requests"][0]["url"] = json!(format!("{base}/echo"));
    source["collections"][0]["requests"][0]["auth"]["kind"] = json!("inherit");
    source["collections"][0]["requests"][0]["assertions"] = json!([]);
    source["collections"][0]["auth"] = json!({"kind":"oauth1","token":"","username":"","password":"","oauth1":{"consumer_key":"parent-oauth1","consumer_secret":"{{consumer}}","token":"token","token_secret":"token/secret","private_key":"{{unused_pem}}","algorithm":"PLAINTEXT"}});
    source["environments"] = json!([{"id":"dev","name":"Development","variables":[{"id":"v","key":"consumer","value":"environment&secret","enabled":true}]}]);
    source["active_environment_id"] = json!("dev");
    let (status, w) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"oauth1","name":"OAuth1","data":source})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    let (status, result) = call(
        &router,
        "POST",
        "/api/execute",
        None,
        Some(json!({"workspace_id":"oauth1","request":w["data"]["collections"][0]["requests"][0]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert!(result["body"].as_str().unwrap().contains("oauth_signature"));
    let (_, history) = call(&router, "GET", "/api/workspaces/oauth1/history", None, None).await;
    for private in [
        "parent-oauth1",
        "environment&secret",
        "environment%26secret&token%2Fsecret",
    ] {
        assert!(!history.to_string().contains(private), "{history}");
    }
    let (_, stored) = call(&router, "GET", "/api/workspaces/oauth1", None, None).await;
    assert_eq!(
        stored["data"]["collections"][0]["auth"]["oauth1"]["private_key"],
        "{{unused_pem}}"
    );
    server.abort();
}
