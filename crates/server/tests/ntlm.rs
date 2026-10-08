mod common;
#[path = "../../core/tests/common/ntlm_fixture.rs"]
mod ntlm_fixture;
use common::*;
#[tokio::test]
async fn parent_environment_ntlm_identity_and_generated_response_values_are_private() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("ntlm.db")).await.unwrap();
    let fixture = ntlm_fixture::serve_ntlm(false).await;
    let mut source = example_data();
    source["collections"][0]["auth"] = json!({"kind":"ntlm","token":"","username":"{{principal}}","password":"{{credential}}","ntlm":{"domain":"{{domain}}","workstation":"{{station}}"}});
    source["collections"][0]["requests"][0]["url"] = json!(format!("{}/resource", fixture.url));
    source["collections"][0]["requests"][0]["auth"]["kind"] = json!("inherit");
    source["collections"][0]["requests"][0]["assertions"] = json!([]);
    source["environments"] = json!([{"id":"dev","name":"Dev","variables":[{"id":"u","key":"principal","value":"User","enabled":true},{"id":"p","key":"credential","value":"Password","enabled":true},{"id":"d","key":"domain","value":"DOMAIN","enabled":true},{"id":"w","key":"station","value":"WORKSTATION","enabled":true}]}]);
    source["active_environment_id"] = json!("dev");
    let (status, w) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"ntlm","name":"NTLM","data":source})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    let (status, response) = call(
        &router,
        "POST",
        "/api/execute",
        None,
        Some(json!({"workspace_id":"ntlm","request":w["data"]["collections"][0]["requests"][0]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], 200);
    let result: Value = serde_json::from_str(response["body"].as_str().unwrap()).unwrap();
    let auth = result["auth"].as_str().unwrap();
    assert!(auth.starts_with("NTLM "));
    let (_, history) = call(&router, "GET", "/api/workspaces/ntlm/history", None, None).await;
    assert!(!history.to_string().contains(auth));
    fixture.task.abort();
}
