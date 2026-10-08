mod common;
use common::*;
#[tokio::test]
async fn parent_hawk_environment_credentials_work_and_generated_headers_are_private() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("hawk.db")).await.unwrap();
    let provider = Router::new().route(
        "/hawk",
        axum::routing::post(
            |axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
             headers: axum::http::HeaderMap,
             body: axum::body::Bytes| async move {
                let authorization = headers["authorization"].to_str().unwrap();
                let parsed = authorization
                    .strip_prefix("Hawk ")
                    .unwrap()
                    .parse::<hawk::Header>()
                    .unwrap();
                assert_eq!(parsed.id.as_deref(), Some("parent-hawk-id"));
                assert_eq!(parsed.app.as_deref(), Some("application"));
                assert_eq!(parsed.dlg.as_deref(), Some("delegate"));
                let host = headers["host"].to_str().unwrap();
                let url = url::Url::parse(&format!("http://{host}{uri}")).unwrap();
                let hash = hawk::PayloadHasher::hash(
                    "application/json",
                    hawk::DigestAlgorithm::Sha256,
                    &body,
                )
                .unwrap();
                let request = hawk::RequestBuilder::from_url("POST", &url)
                    .unwrap()
                    .hash(&hash[..])
                    .request();
                assert!(request.validate_header(
                    &parsed,
                    &hawk::Key::new("environment-hawk-key", hawk::DigestAlgorithm::Sha256).unwrap(),
                    std::time::Duration::from_secs(60)
                ));
                axum::Json(
                    json!({"authorization":authorization,"copied_key":"environment-hawk-key"}),
                )
            },
        ),
    );
    let (base, server) = serve(provider).await;
    let mut source = example_data();
    let request = &mut source["collections"][0]["requests"][0];
    request["url"] = json!(format!("{base}/hawk"));
    request["method"] = json!("POST");
    request["body_kind"] = json!("json");
    request["body"] = json!("{\"message\":\"actual body\"}");
    request["auth"]["kind"] = json!("inherit");
    request["assertions"] = json!([]);
    source["collections"][0]["auth"] = json!({"kind":"hawk","token":"","username":"","password":"","hawk":{"id":"parent-hawk-id","key":"{{hawk_key}}","algorithm":"sha256","include_payload_hash":true,"app":"application","delegation":"delegate"}});
    source["environments"] = json!([{"id":"dev","name":"Dev","variables":[{"id":"key","key":"hawk_key","value":"environment-hawk-key","enabled":true}]}]);
    source["active_environment_id"] = json!("dev");
    let (status, workspace) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"hawk","name":"Hawk","data":source})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{workspace}");
    let request = workspace["data"]["collections"][0]["requests"][0].clone();
    let (status, response) = call(
        &router,
        "POST",
        "/api/execute",
        None,
        Some(json!({"workspace_id":"hawk","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert!(response["body"].as_str().unwrap().contains("Hawk id="));
    let (_, history) = call(&router, "GET", "/api/workspaces/hawk/history", None, None).await;
    assert!(!history.to_string().contains("parent-hawk-id"));
    assert!(!history.to_string().contains("environment-hawk-key"));
    assert!(
        !history
            .to_string()
            .contains(response["body"].as_str().unwrap())
    );
    server.abort();
}
