use moleapi_generation::project::*;
use serde_json::json;
use std::{collections::BTreeMap, path::PathBuf};
use tokio_util::sync::CancellationToken;
#[cfg(unix)]
#[tokio::test]
#[ignore = "requires explicitly configured Java17+ test runtime"]
async fn native_java_selection_generates_immediately_keeps_running_jobs_and_survives_restart() {
    use std::os::unix::fs::PermissionsExt;
    let java = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_JAVA").expect("explicit Java executable"),
    );
    let directory = tempfile::tempdir().unwrap();
    let preference = directory.path().join("java-runtime.json");
    let entered = directory.path().join("entered");
    let release = directory.path().join("release");
    let quote =
        |path: &std::path::Path| format!("'{}'", path.to_str().unwrap().replace('\'', "'\\''"));
    let wrapper = directory.path().join("selected java");
    std::fs::write(&wrapper, format!("#!/bin/sh\nif [ \"$1\" != -version ]; then\nprintf started > {}\nwhile [ ! -f {} ]; do /bin/sleep 0.01; done\nfi\nexec {} \"$@\"\n", quote(&entered), quote(&release), quote(&java))).unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700)).unwrap();
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), None).unwrap();
    let shared = runtime.clone();
    runtime
        .select_native_java(wrapper, &preference)
        .await
        .unwrap();
    let input = ProjectInput {
        specification: json!({"openapi":"3.1.0","info":{"title":"Native runtime selection","version":"1"},"paths":{},"components":{"schemas":{"Pet":{"type":"object","properties":{"id":{"type":"integer"}}}}}}),
        target: "go".into(),
        options: BTreeMap::new(),
        include_secrets: false,
        templates: None,
    };
    let first_input = input.clone();
    let running =
        tokio::spawn(async move { shared.generate(first_input, CancellationToken::new()).await });
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while !entered.is_file() {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    runtime
        .select_native_java(java.clone(), &preference)
        .await
        .unwrap();
    std::fs::write(release, "continue").unwrap();
    let first = running.await.unwrap().unwrap();
    assert_eq!(runtime.java_path(), Some(java));
    let restarted = ProjectRuntime::new(std::env::current_exe().unwrap(), None).unwrap();
    restarted.restore_native_java(&preference).unwrap();
    let second = restarted
        .generate(input, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(first.engine, "openapi-generator@7.26.0");
    assert_eq!(first.source_sha256, second.source_sha256);
    let model = |artifact: &ProjectArtifact| {
        artifact
            .files
            .iter()
            .find(|file| file.path == "model_pet.go")
            .unwrap()
            .content
            .clone()
    };
    assert_eq!(model(&first), model(&second));
}
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
    let spec: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/projects/fixtures/protobuf-imports.json"
    ))
    .unwrap();
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
        let imported = match language {
            "cpp" => "models/types.pb.h",
            "csharp" => "Types.cs",
            "java" => "fixture/Types.java",
            "kotlin" => "fixture/EchoKt.kt",
            "objc" => "models/Types.pbobjc.h",
            "php" => "Fixture/PBEcho.php",
            "python" => "models/types_pb2.py",
            "ruby" => "models/types_pb.rb",
            _ => unreachable!(),
        };
        assert!(
            artifact.files.iter().any(|file| file.path == imported),
            "{language}: missing imported model {imported}; paths: {:?}",
            artifact.files.iter().map(|f| &f.path).collect::<Vec<_>>()
        );
        assert!(
            !artifact
                .files
                .iter()
                .any(|file| file.path.starts_with("google/protobuf/")
                    || file.path.to_lowercase().contains("unused")
                        && file.path != "protobuf-source.json")
        );
        let descriptor = artifact
            .files
            .iter()
            .find(|file| file.path == "descriptor-set.bin")
            .unwrap();
        let regenerated = runtime.generate(ProjectInput {
            target: format!("protobuf-{language}"),
            specification: json!({"kind":"descriptor","descriptor_set_base64":descriptor.content}),
            options: BTreeMap::new(), include_secrets: false, templates: None,
        }, CancellationToken::new()).await.unwrap();
        let generated_files = |artifact: &moleapi_generation::project::ProjectArtifact| {
            artifact
                .files
                .iter()
                .filter(|file| {
                    ![
                        "README.md",
                        "protobuf-source.json",
                        "moleapi-generation.json",
                        "descriptor-set.bin",
                    ]
                    .contains(&file.path.as_str())
                })
                .map(|file| {
                    (
                        file.path.clone(),
                        file.content.clone(),
                        file.encoding.clone(),
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            generated_files(&artifact),
            generated_files(&regenerated),
            "{language}: source/descriptor emitted code differs"
        );
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
    // A namespace prefix is not evidence that a supplied file is a well-known type.
    let custom = json!({"kind":"proto","files":[
        {"path":"fixture.proto","content":"syntax=\"proto3\"; package fixture; import \"google/protobuf/custom.proto\"; message Envelope {Custom value=1;}"},
        {"path":"google/protobuf/custom.proto","content":"syntax=\"proto3\"; package fixture; message Custom {string value=1;}"}
    ],"entry_files":["fixture.proto"]});
    let custom_artifact = runtime
        .generate(
            ProjectInput {
                target: "protobuf-cpp".into(),
                specification: custom,
                options: BTreeMap::new(),
                include_secrets: false,
                templates: None,
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(
        custom_artifact
            .files
            .iter()
            .any(|file| file.path == "google/protobuf/custom.pb.h")
    );
    let unsafe_import = json!({"kind":"proto","files":[
        {"path":"fixture.proto","content":"syntax=\"proto3\"; package fixture; import \"-model.proto\"; message Envelope {Model value=1;}"},
        {"path":"-model.proto","content":"syntax=\"proto3\"; package fixture; message Model {string value=1;}"}
    ],"entry_files":["fixture.proto"]});
    let error = runtime
        .generate(
            ProjectInput {
                target: "protobuf-python".into(),
                specification: unsafe_import,
                options: BTreeMap::new(),
                include_secrets: false,
                templates: None,
            },
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("cannot be command flags"),
        "{error}"
    );
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
    let spec: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/projects/fixtures/protobuf-imports.json"
    ))
    .unwrap();
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
            .any(|f| f.path == "models/types_pb2.py")
    );
    assert!(
        artifact
            .files
            .iter()
            .any(|f| f.path == "models/common_pb2.py")
    );
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

#[tokio::test]
#[ignore = "requires explicitly configured Java17+ test runtime"]
async fn explicit_dart_mapped_wrapper_partial_overrides_union_default() {
    let java = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_JAVA").expect("explicit Java executable"),
    );
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), Some(java)).unwrap();
    let specification = json!({"openapi":"3.0.3","info":{"title":"Wrapper override","version":"1"},"paths":{},"components":{"schemas":{"Cat":{"type":"object","required":["kind"],"properties":{"kind":{"type":"string","enum":["cat"]}}},"Dog":{"type":"object","required":["kind"],"properties":{"kind":{"type":"string","enum":["dog"]}}},"Animal":{"oneOf":[{"$ref":"#/components/schemas/Cat"},{"$ref":"#/components/schemas/Dog"}],"discriminator":{"propertyName":"kind","mapping":{"cat":"#/components/schemas/Cat","dog":"#/components/schemas/Dog"}}}}}});
    for path in ["model.mustache", "mapped_oneof.mustache"] {
        let content = if path == "model.mustache" {
            "{{#models}}{{#model}}// explicit Dart model {{classname}}{{/model}}{{/models}}"
        } else {
            "// explicit Dart wrapper {{classname}}"
        };
        let input = ProjectInput {
            target: "dart".into(),
            options: BTreeMap::new(),
            include_secrets: false,
            specification: specification.clone(),
            templates: Some(TemplateBundle {
                format: "moleapi-codegen-templates-v1".into(),
                outputs: BTreeMap::new(),
                files: vec![TemplateFile {
                    path: path.into(),
                    content: content.into(),
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
            .find(|file| file.path == "lib/model/animal.dart")
            .unwrap();
        assert!(model.content.contains("explicit Dart"));
        assert!(!model.content.contains("actualInstance"));
    }
}

#[tokio::test]
#[ignore = "requires explicitly configured Java17+ test runtime"]
async fn explicit_swift_api_template_retains_priority_over_query_encoding_default() {
    let java = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_JAVA").expect("explicit Java executable"),
    );
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), Some(java)).unwrap();
    let specification=serde_json::from_str(r#"{"openapi":"3.0.3","info":{"title":"Swift override","version":"1"},"paths":{"/x":{"get":{"operationId":"x","responses":{"200":{"description":"OK"}}}}}}"#).unwrap();
    let input = ProjectInput {
        target: "swift6".into(),
        options: BTreeMap::new(),
        include_secrets: false,
        specification,
        templates: Some(TemplateBundle {
            format: "moleapi-codegen-templates-v1".into(),
            outputs: BTreeMap::new(),
            files: vec![TemplateFile {
                path: "api.mustache".into(),
                content: "// explicit Swift API\n".into(),
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
        .find(|file| file.path.ends_with("/APIs/DefaultAPI.swift"))
        .unwrap();
    assert!(api.content.contains("explicit Swift API"));
    assert!(!api.content.contains("percentEncodedQuery"));
}

#[tokio::test]
#[ignore = "requires explicitly configured Java17+ test runtime"]
async fn explicit_swift_parameter_partial_retains_priority_over_form_array_default() {
    let java = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_JAVA").expect("explicit Java executable"),
    );
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), Some(java)).unwrap();
    let specification = json!({"openapi":"3.0.3","info":{"title":"Swift form override","version":"1"},"paths":{"/form":{"post":{"operationId":"uploadForm","requestBody":{"content":{"multipart/form-data":{"schema":{"type":"object","properties":{"tags":{"type":"array","items":{"type":"string"}}}},"encoding":{"tags":{"style":"form","explode":false}}}}},"responses":{"204":{"description":"OK"}}}}}});
    let input = ProjectInput {
        target: "swift6".into(),
        options: BTreeMap::new(),
        include_secrets: false,
        specification,
        templates: Some(TemplateBundle {
            format: "moleapi-codegen-templates-v1".into(),
            outputs: BTreeMap::new(),
            files: vec![TemplateFile {
                path: "_param.mustache".into(),
                content: "\"{{baseName}}\": \"explicit Swift parameter\"".into(),
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
        .find(|file| file.path.ends_with("/APIs/DefaultAPI.swift"))
        .unwrap();
    assert!(api.content.contains("explicit Swift parameter"));
    assert!(!api.content.contains("compactMap"));
}

#[tokio::test]
#[ignore = "requires explicitly configured protoc and official Java gRPC plugin"]
async fn official_java_plugin_emits_imported_models_and_four_rpc_service_interfaces() {
    let compiler =
        PathBuf::from(std::env::var_os("MOLEAPI_CODEGEN_TEST_PROTOC").expect("explicit protoc"));
    let plugin = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_GRPC_JAVA").expect("explicit Java plugin"),
    );
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), None)
        .unwrap()
        .with_protoc(Some(compiler), BTreeMap::from([("java".into(), plugin)]))
        .unwrap();
    let specification = serde_json::from_str(include_str!(
        "../../../tools/projects/fixtures/protobuf-imports.json"
    ))
    .unwrap();
    let artifact = runtime
        .generate(
            ProjectInput {
                target: "protobuf-java".into(),
                specification,
                options: BTreeMap::new(),
                include_secrets: false,
                templates: None,
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(
        artifact
            .files
            .iter()
            .any(|file| file.path == "fixture/Types.java")
    );
    assert!(
        artifact
            .files
            .iter()
            .any(|file| file.path == "fixture/Common.java")
    );
    let service = artifact
        .files
        .iter()
        .find(|file| file.path == "fixture/EchoServiceGrpc.java")
        .unwrap();
    for method in [
        "getCallMethod",
        "getWatchMethod",
        "getCollectMethod",
        "getChatMethod",
    ] {
        assert!(
            service.content.contains(method),
            "Missing generated Java {method}"
        );
    }
    assert!(
        service.content.contains("EchoServiceStub")
            && service.content.contains("EchoServiceImplBase")
    );
    assert!(artifact.engine.contains("grpc-plugin=true"));
}

#[tokio::test]
#[ignore = "requires explicitly configured protoc and official C# gRPC plugin"]
async fn official_csharp_plugin_emits_imported_models_and_four_rpc_service_interfaces() {
    let compiler =
        PathBuf::from(std::env::var_os("MOLEAPI_CODEGEN_TEST_PROTOC").expect("explicit protoc"));
    let plugin = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_GRPC_CSHARP").expect("explicit C# plugin"),
    );
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), None)
        .unwrap()
        .with_protoc(Some(compiler), BTreeMap::from([("csharp".into(), plugin)]))
        .unwrap();
    let specification = serde_json::from_str(include_str!(
        "../../../tools/projects/fixtures/protobuf-imports.json"
    ))
    .unwrap();
    let artifact = runtime
        .generate(
            ProjectInput {
                target: "protobuf-csharp".into(),
                specification,
                options: BTreeMap::new(),
                include_secrets: false,
                templates: None,
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(artifact.files.iter().any(|file| file.path == "Types.cs"));
    assert!(artifact.files.iter().any(|file| file.path == "Common.cs"));
    let service = artifact
        .files
        .iter()
        .find(|file| file.path == "FixtureGrpc.cs")
        .unwrap();
    for method in ["CallAsync", "Watch", "Collect", "Chat"] {
        assert!(
            service.content.contains(method),
            "Missing generated C# {method}"
        );
    }
    assert!(
        service.content.contains("EchoServiceClient")
            && service.content.contains("EchoServiceBase")
    );
    assert!(artifact.engine.contains("grpc-plugin=true"));
}

#[tokio::test]
async fn kotlin_grpc_requires_java_service_plugin_before_tool_execution() {
    let executable = std::env::current_exe().unwrap();
    let runtime = ProjectRuntime::new(executable.clone(), None)
        .unwrap()
        .with_protoc(
            Some(executable.clone()),
            BTreeMap::from([("kotlin".into(), executable)]),
        )
        .unwrap();
    let specification = serde_json::from_str(include_str!(
        "../../../tools/projects/fixtures/protobuf-imports.json"
    ))
    .unwrap();
    let error = runtime
        .generate(
            ProjectInput {
                target: "protobuf-kotlin".into(),
                specification,
                options: BTreeMap::new(),
                include_secrets: false,
                templates: None,
            },
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("explicitly configured Java gRPC plugin"),
        "{error}"
    );
}

#[tokio::test]
#[ignore = "requires explicitly configured protoc and official Kotlin/Java gRPC plugins"]
async fn official_kotlin_plugin_emits_coroutine_and_java_service_interfaces() {
    let compiler =
        PathBuf::from(std::env::var_os("MOLEAPI_CODEGEN_TEST_PROTOC").expect("explicit protoc"));
    let kotlin = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_GRPC_KOTLIN").expect("explicit Kotlin plugin"),
    );
    let java = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_GRPC_JAVA").expect("explicit Java plugin"),
    );
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), None)
        .unwrap()
        .with_protoc(
            Some(compiler),
            BTreeMap::from([("kotlin".into(), kotlin), ("java".into(), java)]),
        )
        .unwrap();
    let specification = serde_json::from_str(include_str!(
        "../../../tools/projects/fixtures/protobuf-imports.json"
    ))
    .unwrap();
    let artifact = runtime
        .generate(
            ProjectInput {
                target: "protobuf-kotlin".into(),
                specification,
                options: BTreeMap::new(),
                include_secrets: false,
                templates: None,
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(
        artifact
            .files
            .iter()
            .any(|file| file.path == "fixture/Types.java")
    );
    assert!(
        artifact
            .files
            .iter()
            .any(|file| file.path == "fixture/EchoKt.kt")
    );
    assert!(
        artifact
            .files
            .iter()
            .any(|file| file.path == "fixture/EchoServiceGrpc.java")
    );
    let service = artifact
        .files
        .iter()
        .find(|file| file.path == "fixture/FixtureGrpcKt.kt")
        .unwrap();
    assert!(
        service.content.contains("EchoServiceCoroutineStub")
            && service.content.contains("EchoServiceCoroutineImplBase")
    );
    for method in ["call", "watch", "collect", "chat"] {
        assert!(service.content.contains(method), "Missing Kotlin {method}");
    }
}

#[tokio::test]
#[ignore = "requires explicitly configured protoc and official Ruby gRPC plugin"]
async fn official_ruby_plugin_emits_imported_models_and_four_rpc_service_interfaces() {
    let compiler =
        PathBuf::from(std::env::var_os("MOLEAPI_CODEGEN_TEST_PROTOC").expect("explicit protoc"));
    let plugin = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_GRPC_RUBY").expect("explicit Ruby plugin"),
    );
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), None)
        .unwrap()
        .with_protoc(Some(compiler), BTreeMap::from([("ruby".into(), plugin)]))
        .unwrap();
    let specification = serde_json::from_str(include_str!(
        "../../../tools/projects/fixtures/protobuf-imports.json"
    ))
    .unwrap();
    let artifact = runtime
        .generate(
            ProjectInput {
                target: "protobuf-ruby".into(),
                specification,
                options: BTreeMap::new(),
                include_secrets: false,
                templates: None,
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(
        artifact
            .files
            .iter()
            .any(|file| file.path == "models/types_pb.rb")
    );
    assert!(
        artifact
            .files
            .iter()
            .any(|file| file.path == "models/common_pb.rb")
    );
    let service = artifact
        .files
        .iter()
        .find(|file| file.path == "fixture_services_pb.rb")
        .unwrap();
    assert!(service.content.contains("GRPC::GenericService") && service.content.contains("Stub"));
    for method in ["Call", "Watch", "Collect", "Chat"] {
        assert!(service.content.contains(method), "Missing Ruby {method}");
    }
}

#[tokio::test]
#[ignore = "requires explicitly configured protoc and official C++ gRPC plugin"]
async fn official_cpp_plugin_emits_imported_models_and_four_rpc_service_interfaces() {
    let compiler =
        PathBuf::from(std::env::var_os("MOLEAPI_CODEGEN_TEST_PROTOC").expect("explicit protoc"));
    let plugin = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_GRPC_CPP").expect("explicit C++ plugin"),
    );
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), None)
        .unwrap()
        .with_protoc(Some(compiler), BTreeMap::from([("cpp".into(), plugin)]))
        .unwrap();
    let specification = serde_json::from_str(include_str!(
        "../../../tools/projects/fixtures/protobuf-imports.json"
    ))
    .unwrap();
    let artifact = runtime
        .generate(
            ProjectInput {
                target: "protobuf-cpp".into(),
                specification,
                options: BTreeMap::new(),
                include_secrets: false,
                templates: None,
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(
        artifact
            .files
            .iter()
            .any(|file| file.path == "models/types.pb.h")
    );
    assert!(
        artifact
            .files
            .iter()
            .any(|file| file.path == "models/common.pb.cc")
    );
    let service = artifact
        .files
        .iter()
        .find(|file| file.path == "fixture.grpc.pb.h")
        .unwrap();
    assert!(service.content.contains("EchoService") && service.content.contains("Service"));
    for method in ["Call", "Watch", "Collect", "Chat"] {
        assert!(service.content.contains(method), "Missing C++ {method}");
    }
}

#[tokio::test]
#[ignore = "requires explicitly configured protoc and official PHP gRPC plugin"]
async fn official_php_plugin_emits_imported_models_and_four_rpc_client_interfaces() {
    let compiler =
        PathBuf::from(std::env::var_os("MOLEAPI_CODEGEN_TEST_PROTOC").expect("explicit protoc"));
    let plugin = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_GRPC_PHP").expect("explicit PHP plugin"),
    );
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), None)
        .unwrap()
        .with_protoc(Some(compiler), BTreeMap::from([("php".into(), plugin)]))
        .unwrap();
    let specification = serde_json::from_str(include_str!(
        "../../../tools/projects/fixtures/protobuf-imports.json"
    ))
    .unwrap();
    let artifact = runtime
        .generate(
            ProjectInput {
                target: "protobuf-php".into(),
                specification,
                options: BTreeMap::new(),
                include_secrets: false,
                templates: None,
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(
        artifact
            .files
            .iter()
            .any(|file| file.path == "Fixture/PBEcho.php")
    );
    assert!(
        artifact
            .files
            .iter()
            .any(|file| file.path == "Fixture/Shade.php")
    );
    let service = artifact
        .files
        .iter()
        .find(|file| file.path == "Fixture/EchoServiceClient.php")
        .unwrap();
    assert!(service.content.contains("Grpc\\BaseStub"));
    for method in ["Call", "Watch", "Collect", "Chat"] {
        assert!(service.content.contains(method), "Missing PHP {method}");
    }
}

#[tokio::test]
async fn go_message_plugin_configuration_is_explicit_and_required_before_tools() {
    let executable = std::env::current_exe().unwrap();
    let runtime = ProjectRuntime::new(executable.clone(), None)
        .unwrap()
        .with_protoc(Some(executable.clone()), BTreeMap::new())
        .unwrap();
    let specification = serde_json::from_str(include_str!(
        "../../../tools/projects/fixtures/protobuf-go-imports.json"
    ))
    .unwrap();
    let error = runtime
        .clone()
        .generate(
            ProjectInput {
                target: "protobuf-go".into(),
                specification,
                options: BTreeMap::new(),
                include_secrets: false,
                templates: None,
            },
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("explicitly configured Go message plugin"),
        "{error}"
    );
    assert!(
        runtime
            .clone()
            .with_protobuf_plugins(BTreeMap::from([(
                "go".into(),
                PathBuf::from("relative-plugin")
            )]))
            .is_err()
    );
    assert!(
        runtime
            .with_protobuf_plugins(BTreeMap::from([("unsupported".into(), executable)]))
            .is_err()
    );
}

#[tokio::test]
#[ignore = "requires explicitly configured protoc and official Go message/gRPC plugins"]
async fn official_go_plugins_emit_authored_import_path_models_and_optional_services() {
    let compiler =
        PathBuf::from(std::env::var_os("MOLEAPI_CODEGEN_TEST_PROTOC").expect("explicit protoc"));
    let message = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_PROTO_GO").expect("explicit Go message plugin"),
    );
    let grpc = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_GRPC_GO").expect("explicit Go gRPC plugin"),
    );
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), None)
        .unwrap()
        .with_protoc(
            Some(compiler.clone()),
            BTreeMap::from([("go".into(), grpc)]),
        )
        .unwrap()
        .with_protobuf_plugins(BTreeMap::from([("go".into(), message.clone())]))
        .unwrap();
    let specification: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/projects/fixtures/protobuf-go-imports.json"
    ))
    .unwrap();
    let input = ProjectInput {
        target: "protobuf-go".into(),
        specification: specification.clone(),
        options: BTreeMap::new(),
        include_secrets: false,
        templates: None,
    };
    let artifact = runtime
        .generate(input.clone(), CancellationToken::new())
        .await
        .unwrap();
    assert!(
        artifact
            .files
            .iter()
            .any(|file| file.path == "example.invalid/moleapi/fixture/types.pb.go")
    );
    assert!(
        artifact
            .files
            .iter()
            .any(|file| file.path == "example.invalid/moleapi/fixture/common.pb.go")
    );
    let service = artifact
        .files
        .iter()
        .find(|file| file.path == "example.invalid/moleapi/fixture/fixture_grpc.pb.go")
        .unwrap();
    for method in ["Call", "Watch", "Collect", "Chat"] {
        assert!(service.content.contains(method), "Missing Go {method}");
    }
    assert!(
        artifact
            .files
            .iter()
            .any(|file| file.path == "protobuf-source.json"
                && serde_json::from_str::<serde_json::Value>(&file.content).unwrap()
                    == specification)
    );
    let messages = ProjectRuntime::new(std::env::current_exe().unwrap(), None)
        .unwrap()
        .with_protoc(Some(compiler), BTreeMap::new())
        .unwrap()
        .with_protobuf_plugins(BTreeMap::from([("go".into(), message)]))
        .unwrap()
        .generate(input.clone(), CancellationToken::new())
        .await
        .unwrap();
    assert!(
        messages
            .files
            .iter()
            .any(|file| file.path.ends_with("/types.pb.go"))
    );
    assert!(
        !messages
            .files
            .iter()
            .any(|file| file.path.ends_with("_grpc.pb.go"))
    );
    let mut invalid = input;
    invalid.specification = serde_json::from_str(include_str!(
        "../../../tools/projects/fixtures/protobuf-imports.json"
    ))
    .unwrap();
    assert!(
        runtime
            .generate(invalid, CancellationToken::new())
            .await
            .is_err(),
        "Missing authored go_package must not be guessed"
    );
}

#[tokio::test]
#[ignore = "requires explicitly configured protoc and official Objective-C gRPC plugin"]
async fn official_objc_plugin_emits_original_apple_models_and_four_rpc_interfaces() {
    let compiler =
        PathBuf::from(std::env::var_os("MOLEAPI_CODEGEN_TEST_PROTOC").expect("explicit protoc"));
    let plugin = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_GRPC_OBJC").expect("explicit Objective-C plugin"),
    );
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), None)
        .unwrap()
        .with_protoc(Some(compiler), BTreeMap::from([("objc".into(), plugin)]))
        .unwrap();
    let specification: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/projects/fixtures/protobuf-imports.json"
    ))
    .unwrap();
    let input = ProjectInput {
        target: "protobuf-objc".into(),
        specification,
        options: BTreeMap::new(),
        include_secrets: false,
        templates: None,
    };
    let artifact = runtime
        .generate(input.clone(), CancellationToken::new())
        .await
        .unwrap();
    assert!(
        artifact
            .files
            .iter()
            .any(|file| file.path == "models/Types.pbobjc.h")
    );
    let service = artifact
        .files
        .iter()
        .find(|file| file.path.ends_with(".pbrpc.h"))
        .unwrap();
    for method in ["call", "watch", "collect", "chat"] {
        assert!(
            service.content.contains(method),
            "Missing Objective-C {method}"
        );
    }
    let descriptor = artifact
        .files
        .iter()
        .find(|file| file.path == "descriptor-set.bin")
        .unwrap();
    let mut replay = input;
    replay.specification = json!({"kind":"descriptor","descriptor_set_base64":descriptor.content});
    let repeated = runtime
        .generate(replay, CancellationToken::new())
        .await
        .unwrap();
    for file in artifact
        .files
        .iter()
        .filter(|file| file.path.ends_with(".h") || file.path.ends_with(".m"))
    {
        assert_eq!(
            file.content,
            repeated
                .files
                .iter()
                .find(|other| other.path == file.path)
                .unwrap()
                .content
        );
    }
}
