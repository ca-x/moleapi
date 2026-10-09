mod common;
use common::*;
#[tokio::test]
async fn native_project_api_uses_original_owned_definition_and_excludes_credential_examples() {
    let temp = tempfile::tempdir().unwrap();
    let router = moleapi_server::local_with_worker(
        &temp.path().join("project.db"),
        std::path::Path::new(env!("CARGO_BIN_EXE_moleapi-server")),
    )
    .await
    .unwrap();
    let spec=json!({"openapi":"3.0.3","info":{"title":"Project fixture","version":"1"},"paths":{"/health":{"get":{"operationId":"health","responses":{"200":{"description":"OK","content":{"application/json":{"schema":{"$ref":"#/components/schemas/Health"}}}}}}}},"components":{"schemas":{"Health":{"type":"object","properties":{"ok":{"type":"boolean"},"password":{"type":"string","format":"password","example":"unshared-project-password"}}}}}}).to_string();
    let mut data = example_data();
    data["specifications"] =
        json!([{"id":"spec","name":"Project","kind":"openapi","dialect":"3.0.3","source":spec}]);
    let (status, w) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"project","name":"Project","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    let (status,result)=call(&router,"POST","/api/generation/projects",None,Some(json!({"workspace_id":"project","specification_id":"spec","job_id":"native-project","target":"rust-progenitor"}))).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["engine"], "progenitor@0.15.0");
    assert!(!result.to_string().contains("unshared-project-password"));
    assert!(
        result["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["path"] == "src/lib.rs")
    );
    let (_, saved) = call(&router, "GET", "/api/workspaces/project", None, None).await;
    assert_eq!(saved["data"]["specifications"][0]["source"], spec);
    let (status,_)=call(&router,"POST","/api/generation/projects",None,Some(json!({"workspace_id":"missing","specification_id":"spec","job_id":"missing-project","target":"rust-progenitor"}))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
#[tokio::test]
async fn regeneration_route_preserves_authored_changes_and_rejects_corrupted_input() {
    use sha2::{Digest, Sha256};
    let temp = tempfile::tempdir().unwrap();
    let router = moleapi_server::local_with_worker(
        &temp.path().join("regeneration.db"),
        std::path::Path::new(env!("CARGO_BIN_EXE_moleapi-server")),
    )
    .await
    .unwrap();
    let (status, _) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"Regeneration","data":example_data()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let f = |text: &str| json!({"path":"lib.txt","encoding":"utf8","content":text,"sha256":format!("{:x}",Sha256::digest(text.as_bytes())),"bytes":text.len(),"executable":false});
    let input = json!({"workspace_id":"w","job_id":"merge-first","previous":[f("first\nmiddle\nlast\n")],"working":[{"path":"lib.txt","encoding":"utf8","content":"user-first\nmiddle\nlast\n"}],"next":[f("first\nmiddle\ngen-last\n")]});
    let (status, result) = call(
        &router,
        "POST",
        "/api/generation/projects/regenerate",
        None,
        Some(input.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["conflicts"], 0);
    assert_eq!(
        result["files"][0]["content"],
        "user-first\nmiddle\ngen-last\n"
    );
    assert!(result["archive_base64"].is_string());
    let mut broken = input;
    broken["job_id"] = json!("merge-broken");
    broken["previous"][0]["sha256"] = json!("forged");
    let (status, _) = call(
        &router,
        "POST",
        "/api/generation/projects/regenerate",
        None,
        Some(broken),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
