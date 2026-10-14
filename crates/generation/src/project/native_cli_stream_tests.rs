use super::*;
use serde_json::json;

fn input() -> ProjectInput {
    ProjectInput {
        target: "rust-progenitor-cli".into(),
        options: BTreeMap::new(),
        templates: None,
        include_secrets: false,
        specification: json!({"openapi":"3.0.3","info":{"title":"Streaming file","version":"1"},"paths":{
            "/upload":{"post":{"operationId":"upload","requestBody":{"required":true,"content":{"application/octet-stream":{"schema":{"type":"string","format":"binary"}}}},"responses":{"204":{"description":"Uploaded"}}}}
        }}),
    }
}

#[test]
fn binary_cli_file_input_uses_sized_streams_instead_of_whole_file_buffers() {
    let artifact = native_project(&input()).unwrap();
    let cli = &artifact
        .files
        .iter()
        .find(|file| file.path == "src/cli.rs")
        .unwrap()
        .content;
    assert!(cli.contains("ReaderStream::with_capacity"));
    assert!(cli.contains("stream_file(path)"));
    assert!(cli.contains("SizeHint::with_exact"));
    assert!(!cli.contains("tokio::fs::read(path)"));
}

const HELPER_TEST: &str = r#"
#[cfg(test)] mod streaming_fixture {
    use super::*;
    use http_body::Body;
    use futures::future::poll_fn;
    use std::pin::Pin;
    #[tokio::test]
    async fn captured_file_length_preserves_empty_growth_shrink_and_regular_file_contracts() {
        let root=std::path::PathBuf::from(std::env::var("STREAM_FIXTURE_DIR").unwrap());
        let empty=root.join("empty.bin");std::fs::write(&empty,[]).unwrap();
        let mut body=stream_file(&empty).await.unwrap();assert_eq!(body.size_hint().exact(),Some(0));
        assert!(poll_fn(|cx|Pin::new(&mut body).poll_frame(cx)).await.is_none());
        let growing=root.join("growing.bin");std::fs::write(&growing,[0,255,128]).unwrap();
        let mut body=stream_file(&growing).await.unwrap();assert_eq!(body.size_hint().exact(),Some(3));
        std::fs::write(&growing,[0,255,128,7,9]).unwrap();
        let mut bytes=Vec::new();while let Some(frame)=poll_fn(|cx|Pin::new(&mut body).poll_frame(cx)).await {bytes.extend_from_slice(&frame.unwrap().into_data().unwrap());}
        assert_eq!(bytes,[0,255,128]);
        let shrinking=root.join("shrinking.bin");std::fs::write(&shrinking,[1,2,3]).unwrap();
        let mut body=stream_file(&shrinking).await.unwrap();std::fs::write(&shrinking,[1]).unwrap();
        let frame=poll_fn(|cx|Pin::new(&mut body).poll_frame(cx)).await.unwrap().unwrap();assert_eq!(frame.into_data().unwrap().as_ref(),[1]);
        assert!(poll_fn(|cx|Pin::new(&mut body).poll_frame(cx)).await.unwrap().is_err());
        assert!(stream_file(&root).await.is_err());
    }
}
"#;

#[test]
#[ignore = "Compiles generated streamed CLI and verifies a slow 96MiB upload and memory"]
fn streamed_cli_large_file_retains_length_digest_and_bounded_memory() {
    use sha2::{Digest, Sha256};
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
    let cli = project.path().join("src/cli.rs");
    let mut code = std::fs::read_to_string(&cli).unwrap();
    code.push_str(HELPER_TEST);
    std::fs::write(cli, code).unwrap();
    let unit = project.path().join("unit");
    std::fs::create_dir(&unit).unwrap();
    let target = std::env::var_os("MOLEAPI_GENERATED_TEST_TARGET")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| project.path().join("target"));
    let helper = Command::new("cargo")
        .args(["test", "--quiet", "--lib"])
        .current_dir(project.path())
        .env("CARGO_TARGET_DIR", &target)
        .env("STREAM_FIXTURE_DIR", &unit)
        .output()
        .unwrap();
    assert!(
        helper.status.success(),
        "{}",
        String::from_utf8_lossy(&helper.stderr)
    );
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
    const LENGTH: usize = 96 * 1024 * 1024;
    let path = project.path().join("large.bin");
    let mut file = std::fs::File::create(&path).unwrap();
    let chunk = (0..64 * 1024).map(|i| (i % 256) as u8).collect::<Vec<_>>();
    let mut hash = Sha256::new();
    for _ in 0..LENGTH / chunk.len() {
        file.write_all(&chunk).unwrap();
        hash.update(&chunk);
    }
    drop(file);
    let expected = hash.finalize();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(15)))
            .unwrap();
        let mut headers = Vec::new();
        while !headers.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            socket.read_exact(&mut byte).unwrap();
            headers.push(byte[0]);
            assert!(headers.len() < 8192);
        }
        let headers = String::from_utf8(headers).unwrap();
        assert!(headers.starts_with("POST /upload "));
        let header = |name: &str| {
            headers.lines().find_map(|line| {
                let (k, v) = line.split_once(':')?;
                k.eq_ignore_ascii_case(name).then_some(v.trim())
            })
        };
        assert_eq!(
            header("content-length").unwrap().parse::<usize>().unwrap(),
            LENGTH
        );
        assert_eq!(header("content-type"), Some("application/octet-stream"));
        assert!(header("transfer-encoding").is_none());
        assert_eq!(header("authorization"), Some("Bearer streaming-fixture"));
        let mut buffer = vec![0; 64 * 1024];
        let mut actual = Sha256::new();
        for _ in 0..LENGTH / buffer.len() {
            socket.read_exact(&mut buffer).unwrap();
            actual.update(&buffer);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(actual.finalize(), expected);
        socket
            .write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n")
            .unwrap();
    });
    let executable = target
        .join("debug")
        .join(format!("moleapi_sdk{}", std::env::consts::EXE_SUFFIX));
    let mut child = Command::new(executable)
        .args([
            "--base-url",
            &url,
            "--token",
            "streaming-fixture",
            "upload",
            "--body-file",
            path.to_str().unwrap(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    #[cfg(target_os = "linux")]
    let mut peak_rss = 0usize;
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            child.kill().unwrap();
            panic!("Streamed upload timed out");
        }
        #[cfg(target_os = "linux")]
        {
            if let Ok(status) = std::fs::read_to_string(format!("/proc/{}/status", child.id()))
                && let Some(rss) = status.lines().find_map(|line| {
                    line.strip_prefix("VmRSS:")
                        .and_then(|value| value.split_whitespace().next())
                        .and_then(|value| value.parse::<usize>().ok())
                })
            {
                peak_rss = peak_rss.max(rss);
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    server.join().unwrap();
    #[cfg(target_os = "linux")]
    {
        assert!(peak_rss > 0, "No Linux process RSS evidence captured");
        eprintln!("96MiB streamed upload peak observed RSS: {peak_rss} KiB");
        assert!(
            peak_rss < 64 * 1024,
            "File upload buffered too much memory: {peak_rss} KiB"
        );
    }
}
