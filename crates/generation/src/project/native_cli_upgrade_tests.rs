use super::*;
use serde_json::json;

fn input() -> ProjectInput {
    ProjectInput {
        target: "rust-progenitor-cli".into(),
        options: BTreeMap::new(),
        include_secrets: false,
        templates: None,
        specification: json!({"openapi":"3.0.3","info":{"title":"Upgrade fixture","version":"1"},"paths":{
            "/stream":{"get":{"operationId":"stream","parameters":[
                {"in":"header","name":"Connection","required":true,"schema":{"type":"string"}},
                {"in":"header","name":"Upgrade","required":true,"schema":{"type":"string"}}
            ],"responses":{"101":{"description":"Upgraded"},"400":{"description":"Rejected","content":{"application/json":{"schema":{"type":"object","required":["message"],"properties":{"message":{"type":"string"}}}}}}}}},
            "/websocket":{"get":{"operationId":"websocket","x-dropshot-websocket":true,"responses":{"101":{"description":"WebSocket"},"400":{"description":"Rejected","content":{"application/json":{"schema":{"type":"object","required":["message"],"properties":{"message":{"type":"string"}}}}}}}}},
            "/failure":{"get":{"operationId":"failure","responses":{"101":{"description":"Upgraded"},"400":{"description":"Rejected","content":{"application/json":{"schema":{"type":"object","required":["message"],"properties":{"message":{"type":"string"}}}}}}}}},
            "/invalid":{"get":{"operationId":"invalid","responses":{"101":{"description":"Upgraded"}}}}
        }}),
    }
}

#[test]
fn upgraded_cli_artifacts_dispatch_mature_transports_without_placeholders() {
    let artifact = native_project(&input()).unwrap();
    let cli = &artifact
        .files
        .iter()
        .find(|f| f.path == "src/cli.rs")
        .unwrap()
        .content;
    assert!(cli.contains("self.config.success_upgrade(r, false).await"));
    assert!(cli.contains("self.config.success_upgrade(r, true).await"));
    assert!(!cli.contains("todo!()"));
    let main = &artifact
        .files
        .iter()
        .find(|f| f.path == "src/main.rs")
        .unwrap()
        .content;
    assert!(main.contains("create_new(true)"));
    assert!(main.contains("WebSocketStream::from_raw_socket"));
    assert!(main.contains("upgrade-input"));
}

#[test]
#[ignore = "Compiles and executes a generated standalone CLI with real upgraded sockets"]
fn upgraded_cli_preserves_duplex_bytes_websocket_frames_and_errors() {
    use std::{
        net::TcpListener,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    use tungstenite::{Message, WebSocket, protocol::Role};
    let artifact = native_project(&input()).unwrap();
    let project = tempfile::tempdir().unwrap();
    for file in artifact.files {
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
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let bytes = vec![0, 255, 128, b'\n', b'X'];
    let payload = bytes.clone();
    let server = std::thread::spawn(move || {
        for index in 0..6 {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            socket
                .set_write_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                socket.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
                assert!(request.len() < 8192);
            }
            let headers = String::from_utf8_lossy(&request);
            assert!(
                headers
                    .lines()
                    .any(|line| line.eq_ignore_ascii_case("authorization: Bearer upgrade-fixture"))
            );
            if index == 3 {
                assert!(request.starts_with(b"GET /websocket "));
                assert!(
                    headers
                        .lines()
                        .any(|line| line.eq_ignore_ascii_case("upgrade: websocket"))
                );
                let key = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("sec-websocket-key")
                            .then_some(value.trim())
                    })
                    .unwrap();
                let accept = tungstenite::handshake::derive_accept_key(key.as_bytes());
                write!(socket, "HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Accept: {accept}\r\n\r\n").unwrap();
                let mut websocket = WebSocket::from_raw_socket(socket, Role::Server, None);
                websocket.send(Message::Ping(vec![b'p'].into())).unwrap();
                let mut received = false;
                let mut pong = false;
                let mut sent = false;
                for _ in 0..2 {
                    match websocket.read().unwrap() {
                        Message::Binary(value) => {
                            assert_eq!(value.as_ref(), payload);
                            received = true;
                        }
                        Message::Pong(value) => {
                            assert_eq!(value.as_ref(), b"p");
                            pong = true;
                        }
                        other => panic!("Unexpected frame {other:?}"),
                    }
                    if received && !sent {
                        sent = true;
                        websocket.send(Message::Text("鼹鼠".into())).unwrap();
                        websocket
                            .send(Message::Binary(payload.clone().into()))
                            .unwrap();
                    }
                }
                assert!(received && pong);
                websocket.close(None).unwrap();
                assert!(matches!(websocket.read().unwrap(), Message::Close(_)));
                continue;
            }
            if index == 4 {
                assert!(request.starts_with(b"GET /failure "));
                let body = br#"{"message":"upgrade rejected"}"#;
                write!(socket,"HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).unwrap();
                socket.write_all(body).unwrap();
                continue;
            }
            if index == 5 {
                assert!(request.starts_with(b"GET /invalid "));
                socket
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .unwrap();
                continue;
            }
            assert!(request.starts_with(b"GET /stream "));
            assert!(
                headers
                    .lines()
                    .any(|line| line.eq_ignore_ascii_case("upgrade: moleapi-duplex"))
            );
            socket.write_all(b"HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: moleapi-duplex\r\n\r\n").unwrap();
            if index == 2 {
                continue;
            }
            let mut uploaded = Vec::new();
            socket.read_to_end(&mut uploaded).unwrap();
            assert_eq!(uploaded, payload);
            socket.write_all(&payload).unwrap();
        }
    });
    let executable = target
        .join("debug")
        .join(format!("moleapi_sdk{}", std::env::consts::EXE_SUFFIX));
    let input_file = project.path().join("input.bin");
    std::fs::write(&input_file, &bytes).unwrap();
    let run = |args: &[&str]| {
        let mut child = Command::new(&executable)
            .args(["--base-url", &url, "--token", "upgrade-fixture"])
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
                panic!("Generated upgraded CLI timed out");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        child.wait_with_output().unwrap()
    };
    let input_path = input_file.to_str().unwrap();
    let output = run(&[
        "--upgrade-input",
        input_path,
        "stream",
        "--connection",
        "Upgrade",
        "--upgrade",
        "moleapi-duplex",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, bytes);
    let saved = project.path().join("download.bin");
    let path = saved.to_str().unwrap();
    let output = run(&[
        "--upgrade-input",
        input_path,
        "--output-file",
        path,
        "stream",
        "--connection",
        "Upgrade",
        "--upgrade",
        "moleapi-duplex",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert_eq!(std::fs::read(&saved).unwrap(), bytes);
    let output = run(&[
        "--output-file",
        path,
        "stream",
        "--connection",
        "Upgrade",
        "--upgrade",
        "moleapi-duplex",
    ]);
    assert!(!output.status.success());
    assert_eq!(std::fs::read(&saved).unwrap(), bytes);
    let output = run(&["--upgrade-input", input_path, "websocket"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut expected = "鼹鼠".as_bytes().to_vec();
    expected.extend_from_slice(&bytes);
    assert_eq!(output.stdout, expected);
    for operation in ["failure", "invalid"] {
        let output = run(&[operation]);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
    }
    server.join().unwrap();
}
