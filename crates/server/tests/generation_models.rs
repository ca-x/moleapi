mod common;
use common::*;

#[tokio::test]
async fn model_generation_is_owned_typed_private_by_default_and_keeps_saved_source() {
    let temp = tempfile::tempdir().unwrap();
    let router = hosted(config(
        format!(
            "sqlite://{}?mode=rwc",
            temp.path().join("models.db").display()
        ),
        true,
    ))
    .await
    .unwrap();
    let owner = register(&router, "models-owner").await;
    let other = register(&router, "models-other").await;
    let source = json!({"openapi":"3.0.3","info":{"title":"Models","version":"1"},"paths":{},"components":{"schemas":{"Pet":{"type":"object","required":["id"],"properties":{"id":{"type":"integer"},"password":{"type":"string","format":"password","example":"model-private-example"}}},"Other":{"type":"object","required":["unrelated"],"properties":{"unrelated":{"type":"boolean"}}}}}}).to_string();
    let mut data = data();
    data["specifications"] =
        json!([{"id":"models","name":"Models","kind":"openapi","dialect":"3.0.3","source":source}]);
    let (status, _) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"models","name":"Models","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let request = json!({"workspace_id":"models","specification_id":"models","job_id":"models-fixture","target":"model-typescript","options":{"schemaName":"Pet","runtime-typecheck":true}});
    for token in [None, Some(other.as_str())] {
        let (status, _) = call(
            &router,
            "POST",
            "/api/generation/projects",
            token,
            Some(request.clone()),
        )
        .await;
        assert!(!status.is_success());
    }
    let (status, artifact) = call(
        &router,
        "POST",
        "/api/generation/projects",
        Some(&owner),
        Some(request.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{artifact}");
    assert_eq!(artifact["engine"], "quicktype-core@26.0.0");
    assert!(!artifact.to_string().contains("model-private-example"));
    let code = artifact["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["path"] == "models.ts")
        .unwrap()["content"]
        .as_str()
        .unwrap();
    assert!(code.contains("export interface Pet"));
    assert!(!code.contains("unrelated"));
    let mut invalid = request;
    invalid["job_id"] = "models-invalid".into();
    invalid["options"]["runtime-typecheck"] = "true".into();
    let (status, _) = call(
        &router,
        "POST",
        "/api/generation/projects",
        Some(&owner),
        Some(invalid),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (_, stored) = call(&router, "GET", "/api/workspaces/models", Some(&owner), None).await;
    assert_eq!(stored["data"]["specifications"][0]["source"], source);
    let (status, catalog) = call(
        &router,
        "GET",
        "/api/generation/projects/catalog",
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        catalog["targets"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|target| target["kind"] == "model")
            .count(),
        20
    );
}

#[cfg(unix)]
#[tokio::test]
async fn local_java_selection_updates_the_existing_router_without_a_configuration_http_route() {
    use moleapi_generation::project::ProjectRuntime;
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().unwrap();
    let runtime = ProjectRuntime::new(
        std::path::PathBuf::from(env!("CARGO_BIN_EXE_moleapi-server")),
        None,
    )
    .unwrap();
    let router = moleapi_server::local_with_project_runtime(
        &directory.path().join("native.db"),
        runtime.clone(),
    )
    .await
    .unwrap();
    let (status, catalog) = call(
        &router,
        "GET",
        "/api/generation/projects/catalog",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(catalog["java_available"], false);
    let java = directory.path().join("explicit-java");
    std::fs::write(
        &java,
        "#!/bin/sh\nprintf 'openjdk version \"21.0.12\"\\n' >&2\n",
    )
    .unwrap();
    std::fs::set_permissions(&java, std::fs::Permissions::from_mode(0o700)).unwrap();
    runtime
        .select_native_java(java, &directory.path().join("java-runtime.json"))
        .await
        .unwrap();
    let (_, catalog) = call(
        &router,
        "GET",
        "/api/generation/projects/catalog",
        None,
        None,
    )
    .await;
    assert_eq!(catalog["java_available"], true);
    let (status, _) = call(
        &router,
        "POST",
        "/api/generation/projects/runtime/java",
        None,
        Some(json!({"path":"/not-an-http-operation"})),
    )
    .await;
    assert!(!status.is_success());
}
