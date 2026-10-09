mod common;
use common::*;
#[tokio::test]
async fn source_bundle_generation_uses_capped_worker_and_screens_referenced_and_unused_documents() {
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("source-bundle.db")).await.unwrap();
    let root = json!({"openapi":"3.0.3","info":{"title":"Source bundle","version":"1"},"components":{"schemas":{"Health":{"$ref":"models/health.json"}}},"paths":{"/health":{"get":{"operationId":"health","responses":{"200":{"description":"Healthy","content":{"application/json":{"schema":{"$ref":"#/components/schemas/Health"}}}}}}}}});
    let model = json!({"type":"object","properties":{"password":{"type":"string","format":"password","example":"referenced_bundle_secret"},"ok":{"type":"boolean"}}});
    let unused = json!({"type":"string","format":"password","example":"unused_bundle_secret"});
    let source = json!({"format":"moleapi-openapi-source-v1","entry_file":"api.json","files":[{"path":"api.json","content":root.to_string()},{"path":"models/health.json","content":model.to_string()},{"path":"unused.json","content":unused.to_string()}]}).to_string();
    let mut data = example_data();
    data["specifications"] = json!([{"id":"bundle","name":"Source bundle","kind":"openapi","dialect":"3.0.3","source":source}]);
    let (status, _) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"bundle","name":"Source bundle","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let input = json!({"workspace_id":"bundle","specification_id":"bundle","job_id":"bundle-safe","target":"rust-progenitor"});
    let (status, safe) = call(
        &router,
        "POST",
        "/api/generation/projects",
        None,
        Some(input.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{safe}");
    assert!(!safe.to_string().contains("referenced_bundle_secret"));
    assert!(!safe.to_string().contains("unused_bundle_secret"));
    assert!(
        !safe
            .to_string()
            .contains("x-moleapi-source-bundle-privacy-documents")
    );
    assert!(safe["files"].as_array().unwrap().iter().any(|file| {
        file["path"] == "src/lib.rs"
            && file["content"]
                .as_str()
                .unwrap()
                .contains("pub async fn health")
    }));
    let mut options = input.clone();
    options["job_id"] = "bundle-private-option".into();
    options["options"] = json!({"packageName":"unused_bundle_secret"});
    let (status, _) = call(
        &router,
        "POST",
        "/api/generation/projects",
        None,
        Some(options),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let mut explicit = input;
    explicit["job_id"] = "bundle-explicit".into();
    explicit["include_secrets"] = true.into();
    let (status, included) = call(
        &router,
        "POST",
        "/api/generation/projects",
        None,
        Some(explicit),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{included}");
    assert!(included.to_string().contains("referenced_bundle_secret"));
    let (_, stored) = call(&router, "GET", "/api/workspaces/bundle", None, None).await;
    assert_eq!(stored["data"]["specifications"][0]["source"], source);
    let workspace: moleapi_core::Workspace = serde_json::from_value(stored).unwrap();
    let export = moleapi_formats::export(&workspace, "moleapi", false).unwrap();
    assert!(!export.content.contains("referenced_bundle_secret"));
    assert!(
        moleapi_formats::export(&workspace, "moleapi", true)
            .unwrap()
            .content
            .contains("unused_bundle_secret")
    );
}
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
    let (status, baseline) = call(&router, "POST", "/api/generation/projects/import", None,
        Some(json!({"workspace_id":"project","job_id":"import-baseline","role":"previous","source":{"kind":"zip","archive_base64":result["archive_base64"]}}))).await;
    assert_eq!(status, StatusCode::OK, "{baseline}");
    let mut expected_files = result["files"].as_array().unwrap().clone();
    expected_files.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
    assert_eq!(baseline["files"], json!(expected_files));
    let mut edited = baseline["files"].as_array().unwrap().iter().filter(|file| file["path"] != "README.md")
        .map(|file| json!({"path":file["path"],"encoding":file["encoding"],"content":file["content"],"executable":file["executable"]})).collect::<Vec<_>>();
    edited.push(
        json!({"path":"authored.txt","encoding":"utf8","content":"local code","executable":false}),
    );
    let (status, current) = call(&router, "POST", "/api/generation/projects/import", None,
        Some(json!({"workspace_id":"project","job_id":"import-edited","role":"working","source":{"kind":"files","files":edited}}))).await;
    assert_eq!(status, StatusCode::OK, "{current}");
    let working = current["files"].as_array().unwrap().iter().map(|file| json!({"path":file["path"],"encoding":file["encoding"],"content":file["content"],"executable":file["executable"]})).collect::<Vec<_>>();
    let (status, merged) = call(&router, "POST", "/api/generation/projects/regenerate", None,
        Some(json!({"workspace_id":"project","job_id":"merge-imported","previous":baseline["files"],"working":working,"next":result["files"]}))).await;
    assert_eq!(status, StatusCode::OK, "{merged}");
    assert_eq!(merged["conflicts"], 0);
    assert!(
        merged["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["path"] == "authored.txt" && file["content"] == "local code")
    );
    assert!(
        merged["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["path"] == "README.md" && file["status"] == "deleted")
    );
    let (status, _) = call(&router, "POST", "/api/generation/projects/import", None,
        Some(json!({"workspace_id":"project","job_id":"import-invalid","role":"previous","source":{"kind":"zip","archive_base64":"invalid"}}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status,_)=call(&router,"POST","/api/generation/projects",None,Some(json!({"workspace_id":"missing","specification_id":"spec","job_id":"missing-project","target":"rust-progenitor"}))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn project_import_is_workspace_owned_and_unauthorized_imports_never_start() {
    let temp = tempfile::tempdir().unwrap();
    let router = hosted(config(
        format!(
            "sqlite://{}?mode=rwc",
            temp.path().join("imports.db").display()
        ),
        true,
    ))
    .await
    .unwrap();
    let first = register(&router, "import-first").await;
    let second = register(&router, "import-second").await;
    let (status, _) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&first),
        Some(json!({"id":"private","name":"Private","data":example_data()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let input = json!({"workspace_id":"private","job_id":"owned-import","role":"working","source":{"kind":"files","files":[]}});
    let (status, _) = call(
        &router,
        "POST",
        "/api/generation/projects/import",
        Some(&second),
        Some(input.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(
        &router,
        "POST",
        "/api/generation/projects/import",
        None,
        Some(input.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, result) = call(
        &router,
        "POST",
        "/api/generation/projects/import",
        Some(&first),
        Some(input),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["files"], json!([]));
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
#[tokio::test]
async fn native_protobuf_project_route_generates_shared_client_server_artifact_from_saved_bundle() {
    let temp = tempfile::tempdir().unwrap();
    let router = moleapi_server::local_with_worker(
        &temp.path().join("proto-project.db"),
        std::path::Path::new(env!("CARGO_BIN_EXE_moleapi-server")),
    )
    .await
    .unwrap();
    let source=json!({"kind":"proto","files":[{"path":"fixture.proto","content":"syntax=\"proto3\"; package fixture; message Echo {string text=1;} service EchoService {rpc Call(Echo) returns(Echo);}"}],"entry_files":["fixture.proto"]}).to_string();
    let mut data = example_data();
    data["specifications"] = json!([{"id":"proto","name":"Proto project","kind":"protobuf","dialect":"proto3","source":source}]);
    let (status, _) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"proto","name":"Proto generation","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status,result)=call(&router,"POST","/api/generation/projects",None,Some(json!({"workspace_id":"proto","specification_id":"proto","job_id":"proto-native","target":"rust-tonic"}))).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["engine"], "tonic-prost-build@0.14.6");
    assert!(
        result["files"].as_array().unwrap().iter().any(|f| f["path"]
            .as_str()
            .unwrap()
            .ends_with("fixture.rs")
            && f["content"].as_str().unwrap().contains("EchoServiceClient"))
    );
    let (_, saved) = call(&router, "GET", "/api/workspaces/proto", None, None).await;
    assert_eq!(saved["data"]["specifications"][0]["source"], source);
}
