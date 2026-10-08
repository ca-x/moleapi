mod common;
use common::*;
#[tokio::test]
async fn inherited_environment_aws_credentials_execute_and_remain_private() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("aws.db")).await.unwrap();
    let provider=Router::new().route("/resource",axum::routing::post(|headers:axum::http::HeaderMap,body:axum::body::Bytes|async move{
     assert_eq!(headers["x-amz-security-token"],"private-aws-session");let auth=headers["authorization"].to_str().unwrap();assert!(auth.starts_with("AWS4-HMAC-SHA256 Credential=SYNTHETICACCESS/"));assert!(auth.contains("/us-east-1/service/aws4_request"));assert_eq!(body.as_ref(),b"exact request payload");axum::Json(json!({"authorization":auth,"session":headers["x-amz-security-token"].to_str().unwrap(),"credential":"private-aws-secret"}))
 }));
    let (base, server) = serve(provider).await;
    let mut source = example_data();
    let request = &mut source["collections"][0]["requests"][0];
    request["auth"]["kind"] = json!("inherit");
    request["method"] = json!("POST");
    request["url"] = json!(format!("{base}/resource"));
    request["body_kind"] = json!("text");
    request["body"] = json!("exact request payload");
    request["assertions"] = json!([]);
    source["auth"] = json!({"kind":"aws","token":"","username":"","password":"","aws":{"access_key":"{{aws_id}}","secret_key":"{{aws_secret}}","session_token":"{{aws_session}}","region":"{{aws_region}}","service":"service"}});
    source["environments"] = json!([{"id":"dev","name":"Development","variables":[{"id":"id","key":"aws_id","value":"SYNTHETICACCESS","enabled":true},{"id":"secret","key":"aws_secret","value":"private-aws-secret","enabled":true},{"id":"session","key":"aws_session","value":"private-aws-session","enabled":true},{"id":"region","key":"aws_region","value":"us-east-1","enabled":true}]}]);
    source["active_environment_id"] = json!("dev");
    let (status, workspace) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"aws","name":"AWS","data":source})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{workspace}");
    let request = workspace["data"]["collections"][0]["requests"][0].clone();
    let (status, response) = call(
        &router,
        "POST",
        "/api/execute",
        None,
        Some(json!({"workspace_id":"aws","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert!(
        response["body"]
            .as_str()
            .unwrap()
            .contains("AWS4-HMAC-SHA256")
    );
    let (_, history) = call(&router, "GET", "/api/workspaces/aws/history", None, None).await;
    for secret in [
        "SYNTHETICACCESS",
        "private-aws-secret",
        "private-aws-session",
    ] {
        assert!(!history.to_string().contains(secret), "{history}");
    }
    server.abort();
}
