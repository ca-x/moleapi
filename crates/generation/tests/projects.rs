use moleapi_generation::project::*;
use serde_json::json;
use std::{collections::BTreeMap, path::PathBuf};
use tokio_util::sync::CancellationToken;
#[tokio::test]
#[ignore = "requires explicitly configured Java17+ test runtime"]
async fn explicit_templates_override_upstream_rendering_and_preserve_repeatable_source() {
    let java = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_JAVA").expect("explicit Java executable"),
    );
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), Some(java)).unwrap();
    let templates = TemplateBundle {format:"moleapi-codegen-templates-v1".into(),outputs:BTreeMap::new(),files:vec![TemplateFile {encoding:None,path:"models.mustache".into(),content:"{{#models}}{{#model}}export interface {{classname}} { readonly templateMarker: string; }\n{{/model}}{{/models}}".into()}]};
    let result=runtime.generate(ProjectInput {specification:json!({"openapi":"3.0.3","info":{"title":"Template fixture","version":"1"},"paths":{},"components":{"schemas":{"Pet":{"type":"object","properties":{"id":{"type":"integer"}}}}}}),target:"typescript-fetch".into(),options:BTreeMap::new(),include_secrets:false,templates:Some(templates.clone())},CancellationToken::new()).await.unwrap();
    assert!(result.files.iter().any(|file| {
        file.path.ends_with("Pet.ts")
            && file
                .content
                .contains("export interface Pet { readonly templateMarker: string; }")
    }));
    let source = result
        .files
        .iter()
        .find(|file| file.path == "moleapi-templates.json")
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&source.content).unwrap(),
        serde_json::to_value(templates).unwrap()
    );
    let manifest = result
        .files
        .iter()
        .find(|file| file.path == "moleapi-generation.json")
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&manifest.content).unwrap()["templates_sha256"],
        source.sha256
    );
}
#[tokio::test]
#[ignore = "requires explicitly configured Java17+ test runtime"]
async fn pinned_multi_language_engine_produces_sdk_and_server_project_artifacts() {
    let java = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_JAVA").expect("explicit Java executable"),
    );
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), Some(java)).unwrap();
    let spec = json!({"openapi":"3.0.3","info":{"title":"SDK fixture","version":"1"},"paths":{"/health":{"get":{"operationId":"health","responses":{"200":{"description":"OK","content":{"application/json":{"schema":{"$ref":"#/components/schemas/Health"}}}}}}}},"components":{"schemas":{"Health":{"type":"object","required":["ok"],"properties":{"ok":{"type":"boolean"}}}}}});
    let default_targets = [
        "typescript-fetch",
        "javascript",
        "python",
        "go",
        "java",
        "csharp",
        "php",
        "kotlin",
        "swift6",
        "dart",
        "go-server",
        "python-fastapi",
        "spring",
        "aspnetcore",
    ];
    let selected =
        std::env::var("MOLEAPI_CODEGEN_TEST_TARGETS").unwrap_or_else(|_| default_targets.join(","));
    for target in selected.split(',') {
        let artifact = runtime
            .generate(
                ProjectInput {
                    specification: spec.clone(),
                    target: target.into(),
                    options: BTreeMap::new(),
                    include_secrets: false,
                    templates: None,
                },
                CancellationToken::new(),
            )
            .await
            .unwrap_or_else(|e| panic!("{target}: {e}"));
        if let Some(root) = std::env::var_os("MOLEAPI_CODEGEN_TEST_OUTPUT") {
            let directory = PathBuf::from(root).join(target);
            std::fs::create_dir_all(&directory).unwrap();
            for file in &artifact.files {
                use base64::Engine;
                let bytes = if file.encoding == "base64" {
                    base64::engine::general_purpose::STANDARD
                        .decode(&file.content)
                        .unwrap()
                } else {
                    file.content.as_bytes().to_vec()
                };
                let output = directory.join(&file.path);
                std::fs::create_dir_all(output.parent().unwrap()).unwrap();
                std::fs::write(&output, bytes).unwrap();
                #[cfg(unix)]
                if file.executable {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(output, std::fs::Permissions::from_mode(0o755))
                        .unwrap();
                }
            }
        }
        println!("{target}: {} generated files", artifact.files.len());
        assert!(!artifact.files.is_empty());
        assert_eq!(artifact.target, target);
        assert!(!artifact.archive_base64.is_empty());
        assert!(artifact.files.iter().all(|f| f.bytes <= FILE_LIMIT));
    }
}
#[test]
fn external_spec_references_fail_before_engine_execution() {
    let value = json!({"openapi":"3.0.3","info":{"title":"Scope","version":"1"},"paths":{},"components":{"schemas":{"External":{"$ref":"file:///private.json"}}}});
    assert!(
        validate_project_specification(&value)
            .unwrap_err()
            .to_string()
            .contains("External")
    );
}
#[cfg(unix)]
#[tokio::test]
async fn explicit_generator_process_is_killed_and_reaped_on_cancel_without_silent_fallback() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let java = dir.path().join("controlled-java");
    let pid = dir.path().join("pid");
    std::fs::write(
        &java,
        format!(
            "#!/bin/sh\necho $$ > '{}'\nexec /bin/sleep 30\n",
            pid.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&java, std::fs::Permissions::from_mode(0o755)).unwrap();
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), Some(java)).unwrap();
    let cancel = CancellationToken::new();
    let signal = cancel.clone();
    let input = ProjectInput {
        target: "typescript-fetch".into(),
        specification: json!({"openapi":"3.0.3","info":{"title":"Cancel","version":"1"},"paths":{}}),
        options: BTreeMap::new(),
        include_secrets: false,
        templates: None,
    };
    let task = tokio::spawn(async move { runtime.generate(input, signal).await });
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while !pid.is_file() {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let process = std::fs::read_to_string(&pid).unwrap().trim().to_owned();
    cancel.cancel();
    let result = tokio::time::timeout(std::time::Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
    assert!(result.unwrap_err().to_string().contains("cancelled"));
    let status = std::process::Command::new("/bin/kill")
        .args(["-0", &process])
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(
        !status.success(),
        "Generator child remains alive or unreaped"
    );
}
#[tokio::test]
#[ignore = "requires explicitly configured protoc executable"]
async fn official_protoc_generates_all_builtin_language_messages_from_checked_descriptors() {
    let compiler =
        PathBuf::from(std::env::var_os("MOLEAPI_CODEGEN_TEST_PROTOC").expect("explicit protoc"));
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), None)
        .unwrap()
        .with_protoc(Some(compiler), BTreeMap::new())
        .unwrap();
    let spec = json!({"kind":"proto","files":[{"path":"fixture.proto","content":"syntax=\"proto3\"; package fixture; message Echo {string text=1;} service EchoService {rpc Call(Echo) returns(Echo);}"}],"entry_files":["fixture.proto"]});
    for language in [
        "cpp", "csharp", "java", "kotlin", "objc", "php", "python", "ruby",
    ] {
        let artifact = runtime
            .generate(
                ProjectInput {
                    target: format!("protobuf-{language}"),
                    specification: spec.clone(),
                    options: BTreeMap::new(),
                    include_secrets: false,
                    templates: None,
                },
                CancellationToken::new(),
            )
            .await
            .unwrap_or_else(|e| panic!("{language}: {e}"));
        assert!(artifact.files.len() > 3);
        println!("{language}: {} generated files", artifact.files.len());
        if let Some(root) = std::env::var_os("MOLEAPI_CODEGEN_TEST_OUTPUT") {
            let root = PathBuf::from(root).join(language);
            for file in &artifact.files {
                if file.encoding == "utf8" {
                    let path = root.join(&file.path);
                    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                    std::fs::write(path, &file.content).unwrap();
                }
            }
        }
    }
}
#[tokio::test]
#[ignore = "requires explicitly configured Python grpcio-tools compiler"]
async fn integrated_official_python_compiler_emits_message_and_grpc_service_code() {
    let compiler = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_GRPC_PYTHON")
            .expect("explicit grpcio-tools compiler"),
    );
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), None)
        .unwrap()
        .with_protoc(
            Some(compiler.clone()),
            BTreeMap::from([("python".into(), compiler)]),
        )
        .unwrap();
    let spec = json!({"kind":"proto","files":[{"path":"fixture.proto","content":"syntax=\"proto3\"; package fixture; message Echo {string text=1;} service EchoService {rpc Call(Echo) returns(Echo);}"}],"entry_files":["fixture.proto"]});
    let artifact = runtime
        .generate(
            ProjectInput {
                target: "protobuf-python".into(),
                specification: spec,
                options: BTreeMap::new(),
                include_secrets: false,
                templates: None,
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(artifact.files.iter().any(|f| f.path == "fixture_pb2.py"));
    assert!(
        artifact
            .files
            .iter()
            .any(|f| f.path == "fixture_pb2_grpc.py"
                && f.content.contains("EchoServiceStub")
                && f.content.contains("EchoServiceServicer"))
    );
    if let Some(root) = std::env::var_os("MOLEAPI_CODEGEN_TEST_OUTPUT") {
        let root = PathBuf::from(root);
        for file in artifact.files {
            if file.encoding == "utf8" {
                let path = root.join(file.path);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, file.content).unwrap();
            }
        }
    }
}
#[cfg(unix)]
#[tokio::test]
async fn protoc_plugin_process_group_is_terminated_and_reaped_on_cancellation() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let compiler = root.path().join("compiler");
    let pid = root.path().join("pid");
    std::fs::write(
        &compiler,
        format!(
            "#!/bin/sh\necho $$ > '{}'\nexec /bin/sleep 30\n",
            pid.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&compiler, std::fs::Permissions::from_mode(0o755)).unwrap();
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), None)
        .unwrap()
        .with_protoc(Some(compiler), BTreeMap::new())
        .unwrap();
    let cancel = CancellationToken::new();
    let signal = cancel.clone();
    let input = ProjectInput {
        target: "protobuf-python".into(),
        specification: json!({"kind":"proto","files":[{"path":"a.proto","content":"syntax=\"proto3\"; message A {}"}],"entry_files":["a.proto"]}),
        options: BTreeMap::new(),
        include_secrets: false,
        templates: None,
    };
    let task = tokio::spawn(async move { runtime.generate(input, signal).await });
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while !pid.is_file() {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let child = std::fs::read_to_string(pid).unwrap().trim().to_owned();
    cancel.cancel();
    let result = tokio::time::timeout(std::time::Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
    assert!(result.unwrap_err().to_string().contains("cancelled"));
    assert!(
        !std::process::Command::new("/bin/kill")
            .args(["-0", &child])
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap()
            .success()
    );
}

#[tokio::test]
#[ignore = "requires explicitly configured Java17+ test runtime"]
async fn explicit_startup_template_overrides_builtin_aspnet_compatibility_template() {
    let java = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_JAVA").expect("explicit Java executable"),
    );
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), Some(java)).unwrap();
    let startup =
        "// explicitly selected startup\nnamespace {{packageName}} { public class Startup { } }\n";
    let input = ProjectInput {
        specification: json!({"openapi":"3.0.3","info":{"title":"Startup override","version":"1"},"paths":{}}),
        target: "aspnetcore".into(),
        options: BTreeMap::new(),
        include_secrets: false,
        templates: Some(TemplateBundle {
            format: "moleapi-codegen-templates-v1".into(),
            files: vec![TemplateFile {
                path: "Startup.mustache".into(),
                content: startup.into(),
                encoding: None,
            }],
            outputs: BTreeMap::new(),
        }),
    };
    let artifact = runtime
        .generate(input, CancellationToken::new())
        .await
        .unwrap();
    let rendered = artifact
        .files
        .iter()
        .find(|file| file.path.ends_with("/Startup.cs"))
        .unwrap();
    assert!(rendered.content.contains("// explicitly selected startup"));
    assert!(rendered.content.contains("namespace Org.OpenAPITools"));
    assert!(!rendered.content.contains("UseAuthorization"));
    let source = artifact
        .files
        .iter()
        .find(|file| file.path == "moleapi-templates.json")
        .unwrap();
    assert!(source.content.contains("explicitly selected startup"));
}

#[tokio::test]
#[ignore = "requires explicitly configured Java17+ test runtime"]
async fn explicit_javascript_client_template_overrides_binary_compatibility_default() {
    let java = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_JAVA").expect("explicit Java executable"),
    );
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), Some(java)).unwrap();
    let input = ProjectInput {
        specification: json!({"openapi":"3.0.3","info":{"title":"Client override","version":"1"},"paths":{}}),
        target: "javascript".into(),
        options: BTreeMap::new(),
        include_secrets: false,
        templates: Some(TemplateBundle {
            format: "moleapi-codegen-templates-v1".into(),
            outputs: BTreeMap::new(),
            files: vec![TemplateFile {
                path: "ApiClient.mustache".into(),
                content: "// explicit client {{projectName}}\n".into(),
                encoding: None,
            }],
        }),
    };
    let result = runtime
        .generate(input, CancellationToken::new())
        .await
        .unwrap();
    let client = result
        .files
        .iter()
        .find(|file| file.path == "src/ApiClient.js")
        .unwrap();
    assert!(client.content.contains("// explicit client"));
    assert!(!client.content.contains("isBinaryType"));
    assert!(
        result
            .files
            .iter()
            .find(|file| file.path == "moleapi-templates.json")
            .unwrap()
            .content
            .contains("explicit client")
    );
}

#[tokio::test]
#[ignore = "requires explicitly configured Java17+ test runtime"]
async fn explicit_csharp_api_template_overrides_form_array_compatibility_default() {
    let java = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_JAVA").expect("explicit Java executable"),
    );
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), Some(java)).unwrap();
    let input = ProjectInput {
        target: "csharp".into(),
        options: BTreeMap::new(),
        include_secrets: false,
        specification: json!({"openapi":"3.0.3","info":{"title":"API override","version":"1"},"paths":{"/x":{"get":{"operationId":"x","responses":{"200":{"description":"OK"}}}}}}),
        templates: Some(TemplateBundle {
            format: "moleapi-codegen-templates-v1".into(),
            outputs: BTreeMap::new(),
            files: vec![TemplateFile {
                path: "api.mustache".into(),
                content: "// explicit CSharp API {{classname}}\n".into(),
                encoding: None,
            }],
        }),
    };
    let result = runtime
        .generate(input, CancellationToken::new())
        .await
        .unwrap();
    let api = result
        .files
        .iter()
        .find(|file| file.path.ends_with("/Api/DefaultApi.cs"))
        .unwrap();
    assert!(api.content.contains("// explicit CSharp API"));
    assert!(!api.content.contains("formParameterLocalVars"));
    assert!(
        result
            .files
            .iter()
            .find(|file| file.path == "moleapi-templates.json")
            .unwrap()
            .content
            .contains("explicit CSharp API")
    );
}

#[tokio::test]
#[ignore = "requires explicitly configured Java17+ test runtime"]
async fn explicit_javascript_union_partial_overrides_json_binding_compatibility_default() {
    let java = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_JAVA").expect("explicit Java executable"),
    );
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), Some(java)).unwrap();
    let input = ProjectInput {
        target: "javascript".into(),
        options: BTreeMap::new(),
        include_secrets: false,
        specification: json!({"openapi":"3.0.3","info":{"title":"Union override","version":"1"},"paths":{},"components":{"schemas":{"Cat":{"type":"object","properties":{"name":{"type":"string"}}},"Dog":{"type":"object","properties":{"barks":{"type":"boolean"}}},"Animal":{"oneOf":[{"$ref":"#/components/schemas/Cat"},{"$ref":"#/components/schemas/Dog"}]}}}}),
        templates: Some(TemplateBundle {
            format: "moleapi-codegen-templates-v1".into(),
            outputs: BTreeMap::new(),
            files: vec![TemplateFile {
                path: "partial_model_oneof.mustache".into(),
                content: "// explicit union {{classname}}\n".into(),
                encoding: None,
            }],
        }),
    };
    let result = runtime
        .generate(input, CancellationToken::new())
        .await
        .unwrap();
    let animal = result
        .files
        .iter()
        .find(|file| file.path == "src/model/Animal.js")
        .unwrap();
    assert!(animal.content.contains("// explicit union Animal"));
    assert!(!animal.content.contains("getActualInstance"));
    assert!(
        result
            .files
            .iter()
            .find(|file| file.path == "moleapi-templates.json")
            .unwrap()
            .content
            .contains("explicit union")
    );
}

#[tokio::test]
#[ignore = "requires explicitly configured Java17+ test runtime"]
async fn explicit_csharp_converter_template_overrides_discriminator_compatibility_default() {
    let java = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_JAVA").expect("explicit Java executable"),
    );
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), Some(java)).unwrap();
    let input = ProjectInput {
        target: "csharp".into(),
        options: BTreeMap::new(),
        include_secrets: false,
        specification: json!({"openapi":"3.0.3","info":{"title":"Converter override","version":"1"},"paths":{},"components":{"schemas":{"Pet":{"type":"object","properties":{"name":{"type":"string"}}}}}}),
        templates: Some(TemplateBundle {
            format: "moleapi-codegen-templates-v1".into(),
            outputs: BTreeMap::new(),
            files: vec![TemplateFile {
                path: "JsonConverter.mustache".into(),
                content: "// explicit converter {{classname}}\n".into(),
                encoding: None,
            }],
        }),
    };
    let result = runtime
        .generate(input, CancellationToken::new())
        .await
        .unwrap();
    let model = result
        .files
        .iter()
        .find(|file| file.path.ends_with("/Model/Pet.cs"))
        .unwrap();
    assert!(model.content.contains("// explicit converter Pet"));
    assert!(!model.content.contains("discriminatorValidatedLocalVar"));
    assert!(
        result
            .files
            .iter()
            .find(|file| file.path == "moleapi-templates.json")
            .unwrap()
            .content
            .contains("explicit converter")
    );
}

#[tokio::test]
#[ignore = "requires explicitly configured Java17+ test runtime"]
async fn explicit_java_templates_override_okhttp_form_compatibility_defaults() {
    let java = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_JAVA").expect("explicit Java executable"),
    );
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), Some(java)).unwrap();
    let input = ProjectInput {
        target: "java".into(),
        options: BTreeMap::new(),
        include_secrets: false,
        specification: json!({"openapi":"3.0.3","info":{"title":"Java overrides","version":"1"},"paths":{"/x":{"get":{"operationId":"x","responses":{"200":{"description":"OK"}}}}}}),
        templates: Some(TemplateBundle {
            format: "moleapi-codegen-templates-v1".into(),
            outputs: BTreeMap::new(),
            files: vec![
                TemplateFile {
                    path: "ApiClient.mustache".into(),
                    content: "// explicit Java client {{invokerPackage}}\n".into(),
                    encoding: None,
                },
                TemplateFile {
                    path: "api.mustache".into(),
                    content: "// explicit Java API {{classname}}\n".into(),
                    encoding: None,
                },
            ],
        }),
    };
    let result = runtime
        .generate(input, CancellationToken::new())
        .await
        .unwrap();
    let client = result
        .files
        .iter()
        .find(|file| file.path.ends_with("/client/ApiClient.java"))
        .unwrap();
    assert!(client.content.contains("// explicit Java client"));
    assert!(!client.content.contains("buildRequestBodyMultipart"));
    let api = result
        .files
        .iter()
        .find(|file| file.path.ends_with("/api/DefaultApi.java"))
        .unwrap();
    assert!(api.content.contains("// explicit Java API DefaultApi"));
    assert!(!api.content.contains("localVarFormParams"));
    assert!(
        result
            .files
            .iter()
            .find(|file| file.path == "moleapi-templates.json")
            .unwrap()
            .content
            .contains("explicit Java client")
    );
}

#[tokio::test]
#[ignore = "requires explicitly configured Java17+ test runtime"]
async fn explicit_kotlin_api_template_overrides_form_format_compatibility_default() {
    let java = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_JAVA").expect("explicit Java executable"),
    );
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), Some(java)).unwrap();
    let input = ProjectInput {
        target: "kotlin".into(),
        options: BTreeMap::new(),
        include_secrets: false,
        specification: json!({"openapi":"3.0.3","info":{"title":"Kotlin override","version":"1"},"paths":{"/x":{"get":{"operationId":"x","responses":{"200":{"description":"OK"}}}}}}),
        templates: Some(TemplateBundle {
            format: "moleapi-codegen-templates-v1".into(),
            outputs: BTreeMap::new(),
            files: vec![TemplateFile {
                path: "api.mustache".into(),
                content: "// explicit Kotlin API {{classname}}\n".into(),
                encoding: None,
            }],
        }),
    };
    let result = runtime
        .generate(input, CancellationToken::new())
        .await
        .unwrap();
    let api = result
        .files
        .iter()
        .find(|file| file.path.ends_with("/apis/DefaultApi.kt"))
        .unwrap();
    assert!(api.content.contains("// explicit Kotlin API DefaultApi"));
    assert!(!api.content.contains("PartConfig"));
    assert!(
        result
            .files
            .iter()
            .find(|file| file.path == "moleapi-templates.json")
            .unwrap()
            .content
            .contains("explicit Kotlin API")
    );
}

#[tokio::test]
#[ignore = "requires explicitly configured Java17+ test runtime"]
async fn explicit_kotlin_retrofit_template_overrides_path_order_compatibility_default() {
    let java = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_JAVA").expect("explicit Java executable"),
    );
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), Some(java)).unwrap();
    let input = ProjectInput {
        target: "kotlin".into(),
        options: BTreeMap::from([
            ("library".into(), "jvm-retrofit2".into()),
            ("serializationLibrary".into(), "gson".into()),
        ]),
        include_secrets: false,
        specification: json!({"openapi":"3.0.3","info":{"title":"Kotlin override","version":"1"},"paths":{"/x":{"get":{"operationId":"x","responses":{"200":{"description":"OK"}}}}}}),
        templates: Some(TemplateBundle {
            format: "moleapi-codegen-templates-v1".into(),
            outputs: BTreeMap::new(),
            files: vec![TemplateFile {
                path: "api.mustache".into(),
                content: "// explicit Kotlin API {{classname}}\n".into(),
                encoding: None,
            }],
        }),
    };
    let result = runtime
        .generate(input, CancellationToken::new())
        .await
        .unwrap();
    let api = result
        .files
        .iter()
        .find(|file| file.path.ends_with("/apis/DefaultApi.kt"))
        .unwrap();
    assert!(api.content.contains("// explicit Kotlin API DefaultApi"));
    assert!(!api.content.contains("PartConfig"));
    assert!(
        result
            .files
            .iter()
            .find(|file| file.path == "moleapi-templates.json")
            .unwrap()
            .content
            .contains("explicit Kotlin API")
    );
}

#[tokio::test]
#[ignore = "requires explicitly configured Java17+ test runtime"]
async fn explicit_php_api_template_overrides_form_collection_compatibility_default() {
    let java = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_JAVA").expect("explicit Java executable"),
    );
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), Some(java)).unwrap();
    let input = ProjectInput {
        target: "php".into(),
        options: BTreeMap::new(),
        include_secrets: false,
        specification: json!({"openapi":"3.0.3","info":{"title":"PHP override","version":"1"},"paths":{"/x":{"get":{"operationId":"x","responses":{"200":{"description":"OK"}}}}}}),
        templates: Some(TemplateBundle {
            format: "moleapi-codegen-templates-v1".into(),
            outputs: BTreeMap::new(),
            files: vec![TemplateFile {
                path: "api.mustache".into(),
                content: "// explicit PHP API {{classname}}\n".into(),
                encoding: None,
            }],
        }),
    };
    let result = runtime
        .generate(input, CancellationToken::new())
        .await
        .unwrap();
    let api = result
        .files
        .iter()
        .find(|file| file.path.ends_with("/Api/DefaultApi.php"))
        .unwrap();
    assert!(api.content.contains("// explicit PHP API DefaultApi"));
    assert!(!api.content.contains("collectionParams"));
    assert!(
        result
            .files
            .iter()
            .find(|file| file.path == "moleapi-templates.json")
            .unwrap()
            .content
            .contains("explicit PHP API")
    );
}

#[tokio::test]
#[ignore = "requires explicitly configured Java17+ test runtime"]
async fn explicit_php_model_and_serializer_templates_override_union_compatibility_defaults() {
    let java = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_JAVA").expect("explicit Java executable"),
    );
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), Some(java)).unwrap();
    let input = ProjectInput {
        target: "php".into(),
        options: BTreeMap::new(),
        include_secrets: false,
        specification: json!({"openapi":"3.0.3","info":{"title":"PHP union override","version":"1"},"paths":{},"components":{"schemas":{"Cat":{"type":"object","properties":{"kind":{"type":"string"}}},"Dog":{"type":"object","properties":{"kind":{"type":"string"}}},"Animal":{"oneOf":[{"$ref":"#/components/schemas/Cat"},{"$ref":"#/components/schemas/Dog"}],"discriminator":{"propertyName":"kind","mapping":{"cat":"#/components/schemas/Cat","dog":"#/components/schemas/Dog"}}}}}}),
        templates: Some(TemplateBundle {
            format: "moleapi-codegen-templates-v1".into(),
            outputs: BTreeMap::new(),
            files: vec![
                TemplateFile {
                    path: "model_generic.mustache".into(),
                    content: "// explicit PHP model {{classname}}\n".into(),
                    encoding: None,
                },
                TemplateFile {
                    path: "ObjectSerializer.mustache".into(),
                    content: "<?php // explicit PHP serializer\n".into(),
                    encoding: None,
                },
            ],
        }),
    };
    let result = runtime
        .generate(input, CancellationToken::new())
        .await
        .unwrap();
    let model = result
        .files
        .iter()
        .find(|file| file.path.ends_with("/Model/Animal.php"))
        .unwrap();
    assert!(model.content.contains("explicit PHP model Animal"));
    assert!(!model.content.contains("DISCRIMINATOR_MAPPING"));
    let serializer = result
        .files
        .iter()
        .find(|file| file.path.ends_with("ObjectSerializer.php"))
        .unwrap();
    assert!(serializer.content.contains("explicit PHP serializer"));
    assert!(!serializer.content.contains("mappedOneOf"));
    assert!(
        result
            .files
            .iter()
            .find(|file| file.path == "moleapi-templates.json")
            .unwrap()
            .content
            .contains("explicit PHP serializer")
    );
}

#[tokio::test]
#[ignore = "requires explicitly configured Java17+ test runtime"]
async fn explicit_php_model_header_overrides_static_type_alias_defaults() {
    let java = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_JAVA").expect("explicit Java executable"),
    );
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), Some(java)).unwrap();
    let input = ProjectInput {
        target: "php".into(),
        options: BTreeMap::new(),
        include_secrets: false,
        specification: json!({"openapi":"3.0.3","info":{"title":"Header override","version":"1"},"paths":{},"components":{"schemas":{"Pet":{"type":"object","properties":{"id":{"type":"integer"}}}}}}),
        templates: Some(TemplateBundle {
            format: "moleapi-codegen-templates-v1".into(),
            outputs: BTreeMap::new(),
            files: vec![TemplateFile {
                path: "model.mustache".into(),
                content:
                    "{{#models}}{{#model}}// explicit PHP header {{classname}}{{/model}}{{/models}}"
                        .into(),
                encoding: None,
            }],
        }),
    };
    let result = runtime
        .generate(input, CancellationToken::new())
        .await
        .unwrap();
    let model = result
        .files
        .iter()
        .find(|file| file.path.ends_with("/Model/Pet.php"))
        .unwrap();
    assert!(model.content.contains("explicit PHP header Pet"));
    assert!(!model.content.contains("phpstan-type"));
}

#[tokio::test]
#[ignore = "requires explicitly configured Java17+ test runtime"]
async fn explicit_php_supporting_output_retains_priority_over_static_analysis_defaults() {
    let java = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_JAVA").expect("explicit Java executable"),
    );
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), Some(java)).unwrap();
    for alternate in [false, true] {
        let path = if alternate {
            "custom-analysis.mustache"
        } else {
            "phpstan.neon.mustache"
        };
        let outputs = if alternate {
            BTreeMap::from([(
                path.into(),
                TemplateOutput {
                    destination_filename: Some("phpstan.neon".into()),
                    ..Default::default()
                },
            )])
        } else {
            BTreeMap::new()
        };
        let input = ProjectInput {
            target: "php".into(),
            options: BTreeMap::new(),
            include_secrets: false,
            specification: json!({"openapi":"3.0.3","info":{"title":"Analysis override","version":"1"},"paths":{}}),
            templates: Some(TemplateBundle {
                format: "moleapi-codegen-templates-v1".into(),
                outputs,
                files: vec![TemplateFile {
                    path: path.into(),
                    content: "# explicit user analysis config\n".into(),
                    encoding: None,
                }],
            }),
        };
        let result = runtime
            .generate(input, CancellationToken::new())
            .await
            .unwrap();
        let config = result
            .files
            .iter()
            .find(|file| file.path == "phpstan.neon")
            .unwrap();
        assert!(config.content.contains("explicit user analysis config"));
        assert!(!config.content.contains("stubFiles"));
        assert!(
            result
                .files
                .iter()
                .any(|file| file.path == "phpstan-guzzle.stub")
        );
    }
}

#[tokio::test]
#[ignore = "requires explicitly configured Java17+ test runtime"]
async fn explicit_php_error_partial_override_is_used_in_sync_and_async_handlers() {
    let java = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_JAVA").expect("explicit Java executable"),
    );
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), Some(java)).unwrap();
    let input = ProjectInput {
        target: "php".into(),
        options: BTreeMap::new(),
        include_secrets: false,
        specification: json!({"openapi":"3.0.3","info":{"title":"Error override","version":"1"},"paths":{"/x":{"get":{"operationId":"x","responses":{"400":{"description":"Error","content":{"application/json":{"schema":{"type":"object","properties":{"message":{"type":"string"}}}}}}}}}}}),
        templates: Some(TemplateBundle {
            format: "moleapi-codegen-templates-v1".into(),
            outputs: BTreeMap::new(),
            files: vec![TemplateFile {
                path: "deserialize_error.mustache".into(),
                content: "// explicit error decoder\n".into(),
                encoding: None,
            }],
        }),
    };
    let result = runtime
        .generate(input, CancellationToken::new())
        .await
        .unwrap();
    let api = result
        .files
        .iter()
        .find(|file| file.path.ends_with("/Api/DefaultApi.php"))
        .unwrap();
    assert_eq!(api.content.matches("explicit error decoder").count(), 2);
    assert!(!api.content.contains("$e->setResponseObject"));
}

#[tokio::test]
#[ignore = "requires explicitly configured Java17+ test runtime"]
async fn explicit_dart_client_template_overrides_response_await_default() {
    let java = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_JAVA").expect("explicit Java executable"),
    );
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), Some(java)).unwrap();
    let input = ProjectInput {
        target: "dart".into(),
        options: BTreeMap::new(),
        include_secrets: false,
        specification: json!({"openapi":"3.0.3","info":{"title":"Dart override","version":"1"},"paths":{}}),
        templates: Some(TemplateBundle {
            format: "moleapi-codegen-templates-v1".into(),
            outputs: BTreeMap::new(),
            files: vec![TemplateFile {
                path: "api_client.mustache".into(),
                content: "// explicit Dart client\n".into(),
                encoding: None,
            }],
        }),
    };
    let result = runtime
        .generate(input, CancellationToken::new())
        .await
        .unwrap();
    let client = result
        .files
        .iter()
        .find(|file| file.path == "lib/api_client.dart")
        .unwrap();
    assert!(client.content.contains("explicit Dart client"));
    assert!(!client.content.contains("Response.fromStream"));
}

#[tokio::test]
#[ignore = "requires explicitly configured Java17+ test runtime"]
async fn explicit_dart_api_template_overrides_binary_and_multipart_defaults() {
    let java = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_JAVA").expect("explicit Java executable"),
    );
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), Some(java)).unwrap();
    let input=ProjectInput {
        target:"dart".into(),options:BTreeMap::new(),include_secrets:false,
        specification: serde_json::from_str(r#"{"openapi":"3.0.3","info":{"title":"API override","version":"1"},"paths":{"/x":{"get":{"operationId":"x","responses":{"200":{"description":"OK"}}}}}}"#).unwrap(),
        templates:Some(TemplateBundle{format:"moleapi-codegen-templates-v1".into(),outputs:BTreeMap::new(),files:vec![TemplateFile{path:"api.mustache".into(),content:"// explicit Dart API\n".into(),encoding:None}]})
    };
    let result = runtime
        .generate(input, CancellationToken::new())
        .await
        .unwrap();
    let api = result
        .files
        .iter()
        .find(|file| file.path == "lib/api/default_api.dart")
        .unwrap();
    assert!(api.content.contains("explicit Dart API"));
    assert!(!api.content.contains("MultipartFile.fromBytes"));
}
