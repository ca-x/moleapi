use moleapi_generation::project::*;
use serde_json::json;
use std::{collections::BTreeMap, path::PathBuf};
use tokio_util::sync::CancellationToken;
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
