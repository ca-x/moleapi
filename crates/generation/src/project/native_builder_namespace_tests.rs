use super::*;
use serde_json::json;

fn input() -> ProjectInput {
    let mut paths = serde_json::Map::new();
    for name in [
        "result",
        "option",
        "some",
        "none",
        "ok",
        "err",
        "error",
        "response_value",
        "byte_stream",
        "operation_info",
        "client_info",
        "client_hooks",
        "request_builder_ext",
    ] {
        paths.insert(format!("/{name}"),json!({"post":{"operationId":name,"parameters":[{"in":"query","name":"q","schema":{"type":"string"}}],"requestBody":{"required":false,"content":{"application/json":{"schema":{"type":"object","required":["id"],"properties":{"id":{"type":"integer","format":"int32"}}}}}},"responses":{"204":{"description":"Accepted"}}}}));
    }
    ProjectInput {
        target: "rust-progenitor-cli".into(),
        options: BTreeMap::new(),
        templates: None,
        include_secrets: false,
        specification: json!({"openapi":"3.0.3","info":{"title":"Builder namespaces","version":"1"},"paths":paths}),
    }
}

#[test]
fn standard_named_operation_builders_keep_names_with_qualified_support_paths() {
    let artifact = native_project(&input()).unwrap();
    let sdk = &artifact
        .files
        .iter()
        .find(|file| file.path == "src/lib.rs")
        .unwrap()
        .content;
    assert!(sdk.contains("pub struct Result<'a>"));
    assert!(sdk.contains("pub struct Option<'a>"));
    assert!(sdk.contains("::std::result::Result"));
    assert!(sdk.contains("super::Error"));
}

#[test]
#[ignore = "Compiles publicly named builders and calls standard/SDK-colliding operations"]
fn standard_named_builders_compile_and_preserve_real_optional_json_requests() {
    use std::{
        net::TcpListener,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let project = tempfile::tempdir().unwrap();
    for file in native_project(&input()).unwrap().files {
        let path = project.path().join(file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, file.content).unwrap();
    }
    let target = std::env::var_os("MOLEAPI_GENERATED_TEST_TARGET")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| project.path().join("target"));
    let build = Command::new("cargo")
        .args(["build", "--quiet"])
        .current_dir(project.path())
        .env("CARGO_TARGET_DIR", &target)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let names = [
        "result",
        "option",
        "some",
        "none",
        "ok",
        "err",
        "error",
        "response_value",
        "byte_stream",
        "operation_info",
        "client_info",
        "client_hooks",
        "request_builder_ext",
    ];
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        for (index, name) in names.iter().enumerate() {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut headers = Vec::new();
            while !headers.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                socket.read_exact(&mut byte).unwrap();
                headers.push(byte[0]);
                assert!(headers.len() < 8192);
            }
            let headers = String::from_utf8(headers).unwrap();
            assert!(headers.starts_with(&format!("POST /{name}?q=namespace ")));
            let header = |name: &str| {
                headers.lines().find_map(|line| {
                    let (k, v) = line.split_once(':')?;
                    k.eq_ignore_ascii_case(name).then_some(v.trim())
                })
            };
            assert_eq!(header("authorization"), Some("Bearer builder-fixture"));
            let length = header("content-length")
                .map(|v| v.parse::<usize>().unwrap())
                .unwrap_or(0);
            let mut body = vec![0; length];
            socket.read_exact(&mut body).unwrap();
            if index % 2 == 0 {
                assert_eq!(header("content-type"), Some("application/json"));
                assert_eq!(
                    serde_json::from_slice::<Value>(&body).unwrap(),
                    json!({"id":7})
                );
            } else {
                assert!(header("content-type").is_none());
                assert!(body.is_empty());
            }
            socket
                .write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n")
                .unwrap();
        }
    });
    let executable = target
        .join("debug")
        .join(format!("moleapi_sdk{}", std::env::consts::EXE_SUFFIX));
    for (index, name) in names.iter().enumerate() {
        let mut command = Command::new(&executable);
        command.args([
            "--base-url",
            &url,
            "--token",
            "builder-fixture",
            name,
            "--q",
            "namespace",
        ]);
        if index % 2 == 0 {
            command.args(["--id", "7"]);
        }
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        while child.try_wait().unwrap().is_none() {
            if Instant::now() >= deadline {
                child.kill().unwrap();
                panic!("Builder CLI timed out");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
    }
    server.join().unwrap();
    // Compile direct uses of the public operation type names too.
    std::fs::create_dir(project.path().join("examples")).unwrap();
    std::fs::write(project.path().join("examples/public.rs"),r#"fn main(){let client=moleapi_sdk::Client::new("http://127.0.0.1:1");let _:moleapi_sdk::builder::Result<'_>=client.result();let _:moleapi_sdk::builder::Option<'_>=client.option();let _:moleapi_sdk::builder::Error<'_>=client.error();let _:moleapi_sdk::builder::ResponseValue<'_>=client.response_value();let _:moleapi_sdk::builder::Some<'_>=client.some();}"#).unwrap();
    let check = Command::new("cargo")
        .args(["check", "--quiet", "--example", "public"])
        .current_dir(project.path())
        .env("CARGO_TARGET_DIR", &target)
        .output()
        .unwrap();
    assert!(
        check.status.success(),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );
}
