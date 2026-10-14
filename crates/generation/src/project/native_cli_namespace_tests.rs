use super::*;
use serde_json::json;

fn input() -> ProjectInput {
    ProjectInput {
        target: "rust-progenitor-cli".into(),
        options: BTreeMap::new(),
        templates: None,
        include_secrets: false,
        specification: json!({"openapi":"3.0.3","info":{"title":"Control namespace","version":"1"},"paths":{
            "/json/{help}":{"post":{"operationId":"json","parameters":[
                {"in":"path","name":"help","required":true,"schema":{"type":"string"}},
                {"in":"query","name":"json_body","required":true,"schema":{"type":"string"}},
                {"in":"query","name":"api_json_body","required":true,"schema":{"type":"string"}},
                {"in":"query","name":"json_body_template","required":true,"schema":{"type":"string"}},
                {"in":"header","name":"body-file","required":true,"schema":{"type":"string"}}
            ],"requestBody":{"required":true,"content":{"application/json":{"schema":{"type":"object","required":["id"],"properties":{"id":{"type":"integer","format":"int32"},"help":{"type":"string"}}}}}},"responses":{"204":{"description":"Accepted"}}}},
            "/binary":{"post":{"operationId":"binary","parameters":[
                {"in":"query","name":"body_file","required":true,"schema":{"type":"string"}},
                {"in":"query","name":"api_body_file","required":true,"schema":{"type":"string"}},
                {"in":"query","name":"help","schema":{"type":"string"}}
            ],"requestBody":{"required":true,"content":{"application/octet-stream":{"schema":{"type":"string","format":"binary"}}}},"responses":{"204":{"description":"Accepted"}}}},
            "/operation":{"get":{"operationId":"help","responses":{"204":{"description":"Accepted"}}}}
        }}),
    }
}

#[test]
fn native_cli_reserved_api_names_have_distinct_help_documented_aliases() {
    let artifact = native_project(&input()).unwrap();
    let cli = &artifact
        .files
        .iter()
        .find(|file| file.path == "src/cli.rs")
        .unwrap()
        .content;
    assert!(cli.contains("api-api-json-body"));
    assert!(cli.contains("API parameter `json_body`"));
    assert!(cli.contains("api-help"));
}

#[test]
#[ignore = "Compiles generated CLI and checks control/API aliases against real HTTP"]
fn native_cli_controls_and_reserved_api_names_keep_distinct_wire_values() {
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
    let json_file = project.path().join("body.json");
    std::fs::write(
        &json_file,
        serde_json::to_vec(&json!({"id":7,"help":"body-help"})).unwrap(),
    )
    .unwrap();
    let binary_file = project.path().join("body.bin");
    std::fs::write(&binary_file, [0, 255, 128, 10, 65]).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        for index in 0..3 {
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
            let line = headers.lines().next().unwrap();
            let path = line.split_whitespace().nth(1).unwrap();
            let parsed = url::Url::parse(&format!("http://fixture{path}")).unwrap();
            let values = parsed
                .query_pairs()
                .into_owned()
                .collect::<BTreeMap<_, _>>();
            let header = |name: &str| {
                headers.lines().find_map(|line| {
                    let (k, v) = line.split_once(':')?;
                    k.eq_ignore_ascii_case(name).then_some(v.trim())
                })
            };
            assert_eq!(header("authorization"), Some("Bearer namespace-fixture"));
            let length = header("content-length")
                .map(|v| v.parse::<usize>().unwrap())
                .unwrap_or(0);
            let mut body = vec![0; length];
            socket.read_exact(&mut body).unwrap();
            if index == 0 {
                assert!(line.starts_with("POST "));
                assert_eq!(parsed.path(), "/json/path-help");
                assert_eq!(
                    values,
                    BTreeMap::from([
                        ("json_body".into(), "wire-query".into()),
                        ("api_json_body".into(), "original-alias".into()),
                        ("json_body_template".into(), "wire-template".into())
                    ])
                );
                assert_eq!(header("body-file"), Some("wire-header"));
                assert_eq!(header("content-type"), Some("application/json"));
                assert_eq!(
                    serde_json::from_slice::<Value>(&body).unwrap(),
                    json!({"id":7,"help":"body-help"})
                );
            } else if index == 1 {
                assert!(line.starts_with("POST "));
                assert_eq!(parsed.path(), "/binary");
                assert_eq!(
                    values,
                    BTreeMap::from([
                        ("body_file".into(), "wire-file".into()),
                        ("api_body_file".into(), "original-file-alias".into()),
                        ("help".into(), "wire-help".into())
                    ])
                );
                assert_eq!(header("content-type"), Some("application/octet-stream"));
                assert_eq!(body, [0, 255, 128, 10, 65]);
            } else {
                assert!(line.starts_with("GET /operation "));
                assert!(values.is_empty());
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
    let run = |args: &[&str]| {
        let mut child = Command::new(&executable)
            .args(["--base-url", &url, "--token", "namespace-fixture"])
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
                panic!("Namespace CLI timed out");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        child.wait_with_output().unwrap()
    };
    let json_args = [
        "json",
        "--api-help",
        "path-help",
        "--api-api-json-body",
        "wire-query",
        "--api-json-body",
        "original-alias",
        "--api-json-body-template",
        "wire-template",
        "--api-body-file",
        "wire-header",
        "--json-body",
        json_file.to_str().unwrap(),
    ];
    let output = run(&json_args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    let binary_args = [
        "binary",
        "--api-api-body-file",
        "wire-file",
        "--api-body-file",
        "original-file-alias",
        "--api-help",
        "wire-help",
        "--body-file",
        binary_file.to_str().unwrap(),
    ];
    let output = run(&binary_args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    let output = run(&["help"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let help = run(&["json", "--help"]);
    assert!(help.status.success());
    let text = String::from_utf8(help.stdout).unwrap();
    assert!(text.contains("--api-api-json-body"));
    assert!(text.contains("API parameter `json_body`"));
    assert!(text.contains("--json-body"));
    assert!(!text.contains("namespace-fixture"));
    let missing = run(&json_args[..json_args.len() - 2]);
    assert!(!missing.status.success());
    assert!(missing.stdout.is_empty());
    server.join().unwrap();
}
