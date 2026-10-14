use super::*;
use serde_json::json;

fn input() -> ProjectInput {
    ProjectInput {
        target: "rust-progenitor-cli".into(),
        options: BTreeMap::new(),
        templates: None,
        include_secrets: false,
        specification: json!({"openapi":"3.0.3","info":{"title":"Pagination fixture","version":"1"},"paths":{
            "/items":{"get":{"operationId":"items","x-dropshot-pagination":{"required":["sort"]},"parameters":[
                {"in":"query","name":"page_token","schema":{"type":"string"}},
                {"in":"query","name":"limit","schema":{"type":"integer","minimum":1,"maximum":4294967295u64}},
                {"in":"query","name":"sort","schema":{"type":"string"}},
                {"in":"header","name":"X-Test","required":true,"schema":{"type":"string"}}
            ],"responses":{
                "200":{"description":"Page","content":{"application/json":{"schema":{"$ref":"#/components/schemas/Page"}}}},
                "400":{"description":"Raw failure","content":{"application/octet-stream":{"schema":{"type":"string","format":"binary"}}}}
            }}}
        },"components":{"schemas":{
            "Page":{"type":"object","required":["items"],"properties":{"items":{"type":"array","items":{"$ref":"#/components/schemas/Item"}},"next_page":{"type":"string","nullable":true}}},
            "Item":{"type":"object","required":["id","note"],"properties":{"id":{"type":"integer","format":"int32"},"note":{"type":"string"}}}
        }}}),
    }
}

#[test]
fn paginated_cli_raw_errors_emit_original_sdk_error_without_placeholders() {
    let artifact = native_project(&input()).unwrap();
    let cli = &artifact
        .files
        .iter()
        .find(|file| file.path == "src/cli.rs")
        .unwrap()
        .content;
    assert!(cli.contains("request.stream()"));
    assert!(cli.contains("return Err(anyhow::Error::new(r))"));
    assert!(!cli.contains("todo!()"));
    let sdk = &artifact
        .files
        .iter()
        .find(|file| file.path == "src/lib.rs")
        .unwrap()
        .content;
    assert!(sdk.contains("page.next_page"));
}

#[test]
fn declared_unsupported_pagination_never_falls_back_to_a_single_cli_call() {
    let mut raw = input();
    raw.specification["paths"]["/items"]["get"]["responses"]["200"] = json!({"description":"Raw page","content":{"application/octet-stream":{"schema":{"type":"string","format":"binary"}}}});
    assert!(native_project(&raw).is_err());
    let mut missing_token = input();
    missing_token.specification["paths"]["/items"]["get"]["parameters"]
        .as_array_mut()
        .unwrap()
        .remove(0);
    assert!(native_project(&missing_token).is_err());
    let mut malformed_page = input();
    malformed_page.specification["components"]["schemas"]["Page"]["properties"]["next_page"] =
        json!({"type":"array","items":{"type":"string"}});
    assert!(native_project(&malformed_page).is_err());
}

#[test]
#[ignore = "Compiles generated standalone paginated CLI and calls multiple real pages"]
fn paginated_cli_traverses_limits_and_returns_raw_first_or_later_page_errors() {
    verify_pagination(input());
}

#[test]
#[ignore = "Compiles generated paginated CLI with explicit uint32 limit and real pages"]
fn paginated_cli_uint32_limits_preserve_the_same_wire_contract() {
    let mut input = input();
    input.specification["paths"]["/items"]["get"]["parameters"][1]["schema"]["format"] =
        "uint32".into();
    verify_pagination(input);
}

fn verify_pagination(input: ProjectInput) {
    use std::{
        net::TcpListener,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let project = tempfile::tempdir().unwrap();
    for file in native_project(&input).unwrap().files {
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
    let note = "鼹鼠 \"quotes\" & +";
    let server = std::thread::spawn(move || {
        // (first-page sort, page token, limit, returned ids, next token, status)
        let cases = [
            (
                Some("all"),
                None,
                None,
                vec![1, 2],
                Some("all +鼹鼠/&"),
                200,
            ),
            (
                None,
                Some("all +鼹鼠/&"),
                None,
                vec![3, 4],
                Some("all second+?"),
                200,
            ),
            (None, Some("all second+?"), None, vec![5], None, 200),
            (
                Some("limited"),
                None,
                Some("3"),
                vec![1, 2],
                Some("limited +鼹鼠/&"),
                200,
            ),
            (
                None,
                Some("limited +鼹鼠/&"),
                Some("3"),
                vec![3, 4],
                Some("must not fetch"),
                200,
            ),
            (
                Some("single"),
                None,
                Some("1"),
                vec![1],
                Some("must not fetch"),
                200,
            ),
            (
                Some("later_error"),
                None,
                None,
                vec![1, 2],
                Some("later +鼹鼠/&"),
                200,
            ),
            (None, Some("later +鼹鼠/&"), None, vec![], None, 400),
            (Some("first_error"), None, None, vec![], None, 400),
        ];
        for (sort, token, limit, ids, next, status) in cases {
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
            let line = headers.lines().next().unwrap();
            let path = line
                .strip_prefix("GET ")
                .unwrap()
                .strip_suffix(" HTTP/1.1")
                .unwrap();
            let request_url = url::Url::parse(&format!("http://fixture{path}")).unwrap();
            assert_eq!(request_url.path(), "/items");
            let actual = request_url
                .query_pairs()
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect::<BTreeMap<_, _>>();
            let expected = [("sort", sort), ("page_token", token), ("limit", limit)]
                .into_iter()
                .filter_map(|(k, v)| v.map(|v| (k.to_owned(), v.to_owned())))
                .collect::<BTreeMap<_, _>>();
            assert_eq!(actual, expected);
            assert!(
                headers
                    .lines()
                    .any(|line| line
                        .eq_ignore_ascii_case("authorization: Bearer pagination-fixture"))
            );
            assert!(
                headers
                    .lines()
                    .any(|line| line.eq_ignore_ascii_case("x-test: pagination"))
            );
            let (mime, body) = if status == 200 {
                ("application/json",serde_json::to_vec(&json!({"items":ids.into_iter().map(|id|json!({"id":id,"note":note})).collect::<Vec<_>>(),"next_page":next})).unwrap())
            } else {
                ("application/octet-stream", vec![0, 255, 128, b'X'])
            };
            let reason = if status == 200 { "OK" } else { "Bad Request" };
            write!(socket,"HTTP/1.1 {status} {reason}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).unwrap();
            socket.write_all(&body).unwrap();
        }
    });
    let executable = target
        .join("debug")
        .join(format!("moleapi_sdk{}", std::env::consts::EXE_SUFFIX));
    let run = |args: &[&str]| {
        let mut child = Command::new(&executable)
            .args([
                "--base-url",
                &url,
                "--token",
                "pagination-fixture",
                "items",
                "--x-test",
                "pagination",
            ])
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
                panic!("Paginated CLI timed out");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        child.wait_with_output().unwrap()
    };
    let records = |bytes: &[u8]| {
        String::from_utf8(bytes.to_vec())
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect::<Vec<_>>()
    };
    for (args, expected) in [
        (vec!["--sort", "all"], vec![1, 2, 3, 4, 5]),
        (vec!["--sort", "limited", "--limit", "3"], vec![1, 2, 3]),
        (vec!["--sort", "single", "--limit", "1"], vec![1]),
    ] {
        let output = run(&args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let items = records(&output.stdout);
        assert_eq!(
            items
                .iter()
                .map(|item| item["id"].as_i64().unwrap())
                .collect::<Vec<_>>(),
            expected
        );
        assert!(items.iter().all(|item| item["note"] == note));
    }
    let later = run(&["--sort", "later_error"]);
    assert!(!later.status.success());
    assert_eq!(records(&later.stdout).len(), 2);
    assert!(String::from_utf8_lossy(&later.stderr).contains("400"));
    assert!(!String::from_utf8_lossy(&later.stderr).contains("panicked"));
    let first = run(&["--sort", "first_error"]);
    assert!(!first.status.success());
    assert!(first.stdout.is_empty());
    assert!(String::from_utf8_lossy(&first.stderr).contains("400"));
    for args in [
        vec![],
        vec!["--sort", "invalid", "--limit", "0"],
        vec!["--sort", "invalid", "--page-token", "manual"],
    ] {
        let invalid = run(&args);
        assert!(!invalid.status.success());
        assert!(invalid.stdout.is_empty());
    }
    server.join().unwrap();
}
