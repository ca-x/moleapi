use super::*;
use serde_json::json;

fn input(target: &str) -> ProjectInput {
    let mut paths = serde_json::Map::new();
    for (name, required, mime, schema) in [
        (
            "optional_binary",
            false,
            "application/octet-stream",
            json!({"type":"string","format":"binary"}),
        ),
        (
            "required_binary",
            true,
            "application/octet-stream",
            json!({"type":"string","format":"binary"}),
        ),
        (
            "optional_text",
            false,
            "text/plain",
            json!({"type":"string"}),
        ),
        (
            "required_text",
            true,
            "text/plain",
            json!({"type":"string"}),
        ),
    ] {
        paths.insert(format!("/{name}"),json!({"post":{"operationId":name,"requestBody":{"required":required,"content":{mime:{"schema":schema}}},"responses":{"204":{"description":"Accepted"}}}}));
    }
    ProjectInput {
        target: target.into(),
        options: BTreeMap::new(),
        templates: None,
        include_secrets: false,
        specification: json!({"openapi":"3.0.3","info":{"title":"Body fixture","version":"1"},"paths":paths}),
    }
}

#[test]
fn optional_raw_bodies_preserve_omission_and_cli_file_types_in_artifacts() {
    for target in ["rust-progenitor", "rust-progenitor-cli"] {
        let artifact = native_project(&input(target)).unwrap();
        let sdk = &artifact
            .files
            .iter()
            .find(|file| file.path == "src/lib.rs")
            .unwrap()
            .content;
        assert!(
            sdk.contains("if let Some(body) = body")
                || sdk.contains("if let ::std::option::Option::Some(body) = body")
        );
        if target == "rust-progenitor-cli" {
            let cli = &artifact
                .files
                .iter()
                .find(|file| file.path == "src/cli.rs")
                .unwrap()
                .content;
            assert!(cli.contains(".required(false)"));
            assert!(cli.contains(".required(true)"));
            assert!(cli.contains("tokio::fs::read_to_string(path)"));
            assert!(cli.contains("stream_file(path)"));
        } else {
            assert!(sdk.contains("body: Option<B>"));
            assert!(sdk.contains("body: Option<::std::string::String>"));
        }
    }
}

fn write_project(input: &ProjectInput, directory: &std::path::Path) {
    for file in native_project(input).unwrap().files {
        let path = directory.join(file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, file.content).unwrap();
    }
}

#[test]
#[ignore = "Compiles generated standalone CLI/positional SDK and verifies real request bodies"]
fn optional_binary_and_text_cli_and_sdk_preserve_wire_semantics() {
    use std::{
        net::TcpListener,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let project = tempfile::tempdir().unwrap();
    write_project(&input("rust-progenitor-cli"), project.path());
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
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let binary = vec![0, 255, 128, b'\n', b'X'];
    let text = "鼹鼠 \"quotes\" & +";
    let payload = binary.clone();
    let server = std::thread::spawn(move || {
        let cases = [
            ("optional_binary", None, Vec::new()),
            (
                "optional_binary",
                Some("application/octet-stream"),
                Vec::new(),
            ),
            ("optional_binary", Some("application/octet-stream"), payload),
            ("optional_text", None, Vec::new()),
            ("optional_text", Some("text/plain"), Vec::new()),
            (
                "optional_text",
                Some("text/plain"),
                text.as_bytes().to_vec(),
            ),
            (
                "required_text",
                Some("text/plain"),
                text.as_bytes().to_vec(),
            ),
            ("optional_binary", None, Vec::new()),
            (
                "optional_text",
                Some("text/plain"),
                text.as_bytes().to_vec(),
            ),
        ];
        for (name, mime, payload) in cases {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                socket.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
                assert!(request.len() < 8192);
            }
            let headers = String::from_utf8_lossy(&request);
            assert!(headers.starts_with(&format!("POST /{name} ")));
            let header = |name: &str| {
                headers.lines().find_map(|line| {
                    let (key, value) = line.split_once(':')?;
                    key.eq_ignore_ascii_case(name).then_some(value.trim())
                })
            };
            assert_eq!(header("content-type"), mime);
            assert!(header("transfer-encoding").is_none());
            let length = header("content-length")
                .map(|length| length.parse::<usize>().unwrap())
                .unwrap_or(0);
            assert_eq!(length, payload.len());
            let mut body = vec![0; length];
            socket.read_exact(&mut body).unwrap();
            assert_eq!(body, payload);
            socket
                .write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n")
                .unwrap();
        }
    });
    let executable = target
        .join("debug")
        .join(format!("moleapi_sdk{}", std::env::consts::EXE_SUFFIX));
    let run = |args: &[&str]| {
        let mut child = Command::new(&executable)
            .args(["--base-url", &url])
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        while child.try_wait().unwrap().is_none() {
            if Instant::now() >= deadline {
                child.kill().unwrap();
                panic!("Body CLI timed out");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        child.wait_with_output().unwrap()
    };
    let binary_file = project.path().join("input.bin");
    std::fs::write(&binary_file, &binary).unwrap();
    let empty_file = project.path().join("empty.bin");
    std::fs::write(&empty_file, []).unwrap();
    let text_file = project.path().join("text.txt");
    std::fs::write(&text_file, text).unwrap();
    for args in [
        vec!["optional_binary"],
        vec![
            "optional_binary",
            "--body-file",
            empty_file.to_str().unwrap(),
        ],
        vec![
            "optional_binary",
            "--body-file",
            binary_file.to_str().unwrap(),
        ],
        vec!["optional_text"],
        vec!["optional_text", "--body-file", empty_file.to_str().unwrap()],
        vec!["optional_text", "--body-file", text_file.to_str().unwrap()],
        vec!["required_text", "--body-file", text_file.to_str().unwrap()],
    ] {
        let output = run(&args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
    }
    for operation in ["required_binary", "required_text"] {
        let output = run(&[operation]);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("--body-file"));
    }
    let invalid = run(&[
        "optional_text",
        "--body-file",
        binary_file.to_str().unwrap(),
    ]);
    assert!(!invalid.status.success());
    let missing = run(&[
        "optional_binary",
        "--body-file",
        project.path().join("missing").to_str().unwrap(),
    ]);
    assert!(!missing.status.success());
    let sdk = tempfile::tempdir().unwrap();
    write_project(&input("rust-progenitor"), sdk.path());
    let manifest = sdk.path().join("Cargo.toml");
    let mut cargo = std::fs::read_to_string(&manifest).unwrap();
    cargo.push_str("tokio = {version=\"1\",features=[\"macros\",\"rt-multi-thread\"]}\n");
    std::fs::write(manifest, cargo).unwrap();
    std::fs::create_dir(sdk.path().join("examples")).unwrap();
    std::fs::write(sdk.path().join("examples/body.rs"),r#"#[tokio::main] async fn main() { let client=moleapi_sdk::Client::new(&std::env::var("FIXTURE_URL").unwrap()); client.optional_binary::<reqwest::Body>(None).await.unwrap(); client.optional_text(Some("鼹鼠 \"quotes\" & +".into())).await.unwrap(); }"#).unwrap();
    let output = Command::new("cargo")
        .args(["run", "--quiet", "--example", "body"])
        .current_dir(sdk.path())
        .env("CARGO_TARGET_DIR", &target)
        .env("FIXTURE_URL", &url)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    server.join().unwrap();
}
