mod common;
use common::*;
#[tokio::test]
async fn source_bundle_generation_uses_capped_worker_and_screens_referenced_and_unused_documents() {
    source_bundle_worker_case(false, false, false).await;
}
#[tokio::test]
async fn static_schema_resources_use_owned_worker_and_preserve_source_privacy() {
    source_bundle_worker_case(true, false, false).await;
}
#[tokio::test]
async fn schema_reference_siblings_survive_owned_worker_model_generation_and_privacy() {
    source_bundle_worker_case(true, true, false).await;
}
#[tokio::test]
async fn mapping_only_subtypes_generate_in_owned_worker_with_source_privacy() {
    source_bundle_worker_case(true, false, true).await;
}
async fn source_bundle_worker_case(
    static_resources: bool,
    schema_siblings: bool,
    promoted_subtypes: bool,
) {
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("source-bundle.db")).await.unwrap();
    let root = json!({"openapi":"3.0.3","info":{"title":"Source bundle","version":"1"},"components":{"schemas":{"Health":{"$ref":"models/health.json#%2Fdefinitions%2FHealth%20%25"}}},"paths":{"/health":{"get":{"operationId":"health","responses":{"200":{"description":"Healthy","content":{"application/json":{"schema":{"$ref":"#/components/schemas/Health"}}}}}}}}});
    let model = json!({"type":"object","properties":{"password":{"type":"string","format":"password","example":"referenced_bundle_secret"},"ok":{"type":"boolean"}}});
    let model = json!({"definitions":{"Health %":model}});
    let unused = json!({"type":"string","format":"password","example":"unused_bundle_secret"});
    let mut root = root;
    let mut model = model;
    if static_resources {
        root["openapi"] = "3.1.0".into();
        root["components"]["schemas"]["Health"]["$ref"] =
            "https://schemas.example.test/health#health".into();
        model["$defs"] = model
            .as_object_mut()
            .unwrap()
            .remove("definitions")
            .unwrap();
        model["$defs"]["Health %"]["$id"] = "https://schemas.example.test/health".into();
        model["$defs"]["Health %"]["$anchor"] = "health".into();
    }
    if schema_siblings {
        root["components"]["schemas"]["Health"]["properties"] = json!({"status":{"type":"string"}});
        root["components"]["schemas"]["Health"]["required"] = json!(["status"]);
    }
    if promoted_subtypes {
        model["$defs"]["Health %"]["discriminator"] = json!({"propertyName":"kind","mapping":{"subtype":"moleapi://source-bundle/models/health.json#/$defs/Other"}});
        model["$defs"]["Other"] = json!({"type":"object","properties":{"subtypeMarker":{"type":"string"},"password":{"type":"string","format":"password","example":"subtype_bundle_secret"}}});
    }
    let source = json!({"format":"moleapi-openapi-source-v1","entry_file":"api.json","files":[{"path":"api.json","content":root.to_string()},{"path":"models/health.json","content":model.to_string()},{"path":"unused.json","content":unused.to_string()}]}).to_string();
    let mut data = example_data();
    data["specifications"] = json!([{"id":"bundle","name":"Source bundle","kind":"openapi","dialect":if static_resources {"3.1.0"} else {"3.0.3"},"source":source}]);
    let (status, _) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"bundle","name":"Source bundle","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let input = json!({"workspace_id":"bundle","specification_id":"bundle","job_id":"bundle-safe","target":if static_resources {"model-go"} else {"rust-progenitor"}});
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
    assert!(!safe.to_string().contains("subtype_bundle_secret"));
    if promoted_subtypes {
        assert!(safe["files"].as_array().unwrap().iter().any(|file| {
            file["path"] != "moleapi-generation.json"
                && file["content"].as_str().unwrap().contains("SubtypeMarker")
        }));
        let mut copied = input.clone();
        copied["job_id"] = "bundle-subtype-copy".into();
        copied["options"] = json!({"package":"subtype_bundle_secret"});
        let (status, _) = call(
            &router,
            "POST",
            "/api/generation/projects",
            None,
            Some(copied),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    assert!(
        !safe
            .to_string()
            .contains("x-moleapi-source-bundle-privacy-documents")
    );
    assert!(safe["files"].as_array().unwrap().iter().any(|file| {
        let content = file["content"].as_str().unwrap();
        if static_resources {
            file["path"] != "moleapi-generation.json" && content.contains("type Health struct")
        } else {
            file["path"] == "src/lib.rs" && content.contains("pub async fn health")
        }
    }));
    if schema_siblings {
        assert!(safe["files"].as_array().unwrap().iter().any(|file| {
            file["path"] != "moleapi-generation.json"
                && file["content"]
                    .as_str()
                    .unwrap()
                    .contains("json:\"status\"")
        }));
    }
    let mut options = input.clone();
    options["job_id"] = "bundle-private-option".into();
    options["options"] = if static_resources {
        json!({"package":"unused_bundle_secret"})
    } else {
        json!({"packageName":"unused_bundle_secret"})
    };
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
    if static_resources {
        explicit["options"] = json!({"package":"unused_bundle_secret"});
    }
    let (status, included) = call(
        &router,
        "POST",
        "/api/generation/projects",
        None,
        Some(explicit),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{included}");
    assert!(included.to_string().contains(if static_resources {
        "unused_bundle_secret"
    } else {
        "referenced_bundle_secret"
    }));
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
async fn ordinary_single_json_and_yaml_resources_use_worker_privacy_and_keep_raw_source() {
    for yaml in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let router = local(&temp.path().join("single.db")).await.unwrap();
        let root = json!({"openapi":"3.1.0","info":{"title":"Single resources","version":"1"},"paths":{},"components":{"schemas":{
            "Pet":{"$id":"https://schemas.test/single","$anchor":"pet","type":"object","properties":{"name":{"type":"string"},"password":{"type":"string","format":"password","example":"ordinary_private_password"}}},
            "Alias":{"$ref":"https://schemas.test/single#pet"}
        }}});
        let source = if yaml {
            format!(
                "# preserve original YAML comment\n{}\n# preserve trailing comment\n",
                root
            )
        } else {
            root.to_string()
        };
        let mut data = example_data();
        // Stored version metadata must not override the actual source dialect.
        data["specifications"] = json!([{"id":"single","name":"Single","kind":"openapi","dialect":"3.0.3","source":source}]);
        let (status, _) = call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"single","name":"Single","data":data})),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let input = json!({"workspace_id":"single","specification_id":"single","job_id":"single-safe","target":"model-go"});
        let (status, safe) = call(
            &router,
            "POST",
            "/api/generation/projects",
            None,
            Some(input.clone()),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{safe}");
        assert!(!safe.to_string().contains("ordinary_private_password"));
        assert!(
            !safe
                .to_string()
                .contains("x-moleapi-source-bundle-privacy-documents")
        );
        assert!(safe["files"].as_array().unwrap().iter().any(|file| {
            file["content"]
                .as_str()
                .unwrap()
                .contains("type Pet struct")
        }));
        let mut copied = input.clone();
        copied["job_id"] = "single-copy".into();
        copied["options"] = json!({"package":"ordinary_private_password"});
        let (status, _) = call(
            &router,
            "POST",
            "/api/generation/projects",
            None,
            Some(copied.clone()),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        copied["job_id"] = "single-explicit".into();
        copied["include_secrets"] = true.into();
        let (status, included) = call(
            &router,
            "POST",
            "/api/generation/projects",
            None,
            Some(copied),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{included}");
        assert!(included.to_string().contains("ordinary_private_password"));
        let (_, saved) = call(&router, "GET", "/api/workspaces/single", None, None).await;
        assert_eq!(saved["data"]["specifications"][0]["source"], source);
        let mut invalid = root.clone();
        invalid["components"]["schemas"]["Alias"]["$ref"] = "https://outside.test/missing".into();
        let revision = saved["revision"].as_u64().unwrap();
        let mut changed = saved["data"].clone();
        changed["specifications"][0]["source"] = invalid.to_string().into();
        let (status, _) = call(
            &router,
            "PUT",
            "/api/workspaces/single",
            None,
            Some(json!({"name":"Single","expected_revision":revision,"data":changed})),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let mut missing = input;
        missing["job_id"] = "single-missing".into();
        let (status, _) = call(
            &router,
            "POST",
            "/api/generation/projects",
            None,
            Some(missing),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
}

#[tokio::test]
async fn hosted_single_source_preparation_stays_owner_bound_before_resource_work() {
    let temp = tempfile::tempdir().unwrap();
    let router = hosted(config(
        format!(
            "sqlite://{}?mode=rwc",
            temp.path().join("single-owner.db").display()
        ),
        true,
    ))
    .await
    .unwrap();
    let owner = register(&router, "single-owner").await;
    let other = register(&router, "single-other").await;
    let source=json!({"openapi":"3.1.0","info":{"title":"Private","version":"1"},"paths":{},"components":{"schemas":{"Pet":{"$id":"https://schemas.test/private","$anchor":"pet","type":"object","properties":{"name":{"type":"string"}}},"Alias":{"$ref":"https://schemas.test/private#pet"}}}}).to_string();
    let mut data = example_data();
    data["specifications"] =
        json!([{"id":"spec","name":"Private","kind":"openapi","dialect":"3.1.0","source":source}]);
    let (status, _) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"private-single","name":"Private","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let input = json!({"workspace_id":"private-single","specification_id":"spec","job_id":"owner-single","target":"model-go"});
    let (status, _) = call(
        &router,
        "POST",
        "/api/generation/projects",
        Some(&other),
        Some(input.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(
        &router,
        "POST",
        "/api/generation/projects",
        None,
        Some(input.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, result) = call(
        &router,
        "POST",
        "/api/generation/projects",
        Some(&owner),
        Some(input),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
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

#[tokio::test]
async fn upgraded_cli_generation_uses_owned_worker_and_preserves_source_privacy() {
    let temp = tempfile::tempdir().unwrap();
    let router = hosted(config(
        format!(
            "sqlite://{}?mode=rwc",
            temp.path().join("upgrade.db").display()
        ),
        true,
    ))
    .await
    .unwrap();
    let owner = register(&router, "upgrade-owner").await;
    let other = register(&router, "upgrade-other").await;
    let source = json!({"openapi":"3.0.3","info":{"title":"Owned upgrade","version":"1"},"paths":{
        "/pages":{"get":{"operationId":"pages","x-dropshot-pagination":{},"parameters":[
            {"in":"query","name":"limit","schema":{"type":"integer","minimum":1,"maximum":4294967295u64}},
            {"in":"query","name":"page_token","schema":{"type":"string"}}
        ],"responses":{"200":{"description":"Page","content":{"application/json":{"schema":{"type":"object","required":["items"],"properties":{"items":{"type":"array","items":{"type":"string"}},"next_page":{"type":"string","nullable":true}}}}}},"400":{"description":"Raw error","content":{"application/octet-stream":{"schema":{"type":"string","format":"binary"}}}}}}},
        "/optional-text":{"post":{"operationId":"optional_text","requestBody":{"required":false,"content":{"text/plain":{"schema":{"type":"string"}}}},"responses":{"204":{"description":"Accepted"}}}},
        "/socket":{"get":{"operationId":"socket","x-dropshot-websocket":true,"responses":{
            "101":{"description":"Upgraded"},
            "400":{"description":"Rejected","content":{"application/json":{"schema":{"$ref":"#/components/schemas/Failure"}}}}
        }}}
    },"components":{"schemas":{"Failure":{"type":"object","properties":{"password":{"type":"string","format":"password","example":"private-upgrade-fixture"}}}}}}).to_string();
    let mut data = example_data();
    data["specifications"] =
        json!([{"id":"spec","name":"Upgrade","kind":"openapi","dialect":"3.0.3","source":source}]);
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"upgrade","name":"Upgrade","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let input = json!({"workspace_id":"upgrade","specification_id":"spec","job_id":"upgrade-fixture","target":"rust-progenitor-cli"});
    let (status, _) = call(
        &router,
        "POST",
        "/api/generation/projects",
        Some(&other),
        Some(input.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(
        &router,
        "POST",
        "/api/generation/projects",
        None,
        Some(input.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, artifact) = call(
        &router,
        "POST",
        "/api/generation/projects",
        Some(&owner),
        Some(input.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{artifact}");
    assert!(!artifact.to_string().contains("private-upgrade-fixture"));
    let files = artifact["files"].as_array().unwrap();
    assert!(files.iter().any(|f| {
        f["path"] == "src/cli.rs"
            && f["content"]
                .as_str()
                .unwrap()
                .contains("self.config.success_upgrade(r, true).await")
    }));
    assert!(files.iter().any(|f| {
        f["path"] == "src/main.rs"
            && f["content"]
                .as_str()
                .unwrap()
                .contains("WebSocketStream::from_raw_socket")
    }));
    assert!(files.iter().any(|f| {
        f["path"] == "src/cli.rs"
            && f["content"]
                .as_str()
                .unwrap()
                .contains("tokio::fs::read_to_string(path)")
    }));
    assert!(files.iter().any(|f| {
        f["path"] == "src/lib.rs"
            && f["content"]
                .as_str()
                .unwrap()
                .contains("if let Some(body) = body")
    }));
    assert!(files.iter().any(|file| {
        file["path"] == "src/cli.rs"
            && file["content"]
                .as_str()
                .unwrap()
                .contains("request.stream()")
            && !file["content"].as_str().unwrap().contains("todo!()")
    }));
    let (_, saved) = call(
        &router,
        "GET",
        "/api/workspaces/upgrade",
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(saved["data"]["specifications"][0]["source"], source);
    let mut explicit = input;
    explicit["job_id"] = "upgrade-explicit".into();
    explicit["include_secrets"] = true.into();
    let (status, included) = call(
        &router,
        "POST",
        "/api/generation/projects",
        Some(&owner),
        Some(explicit),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{included}");
    assert!(included.to_string().contains("private-upgrade-fixture"));
}
