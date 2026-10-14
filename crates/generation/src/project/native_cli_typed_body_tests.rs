use super::*;
use serde_json::json;

fn input(target: &str) -> ProjectInput {
    let mut paths = serde_json::Map::new();
    for (name, required, mime, schema) in [
        (
            "object",
            false,
            "application/json",
            json!({"$ref":"#/components/schemas/Body"}),
        ),
        (
            "required",
            true,
            "application/json",
            json!({"$ref":"#/components/schemas/Body"}),
        ),
        (
            "empty",
            false,
            "application/json",
            json!({"type":"object","properties":{"note":{"type":"string"}}}),
        ),
        (
            "nullable",
            false,
            "application/json",
            json!({"type":"string","nullable":true}),
        ),
        (
            "array",
            false,
            "application/json",
            json!({"type":"array","items":{"type":"string"}}),
        ),
        (
            "form",
            false,
            "application/x-www-form-urlencoded",
            json!({"$ref":"#/components/schemas/Body"}),
        ),
    ] {
        paths.insert(format!("/{name}"),json!({"post":{"operationId":name,"requestBody":{"required":required,"content":{mime:{"schema":schema}}},"responses":{"204":{"description":"Accepted"}}}}));
    }
    ProjectInput {
        target: target.into(),
        options: BTreeMap::new(),
        templates: None,
        include_secrets: false,
        specification: json!({"openapi":"3.0.3","info":{"title":"Optional typed bodies","version":"1"},"paths":paths,"components":{"schemas":{"Body":{"type":"object","required":["id"],"properties":{"id":{"type":"integer","format":"int32"},"note":{"type":"string"}}}}}}),
    }
}

#[test]
fn optional_typed_body_artifacts_preserve_presence_and_builder_validation() {
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
            assert!(sdk.contains("value.unwrap_or_default()"));
        } else {
            assert!(sdk.contains("Option<Option<&"));
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
#[ignore = "Compiles generated CLI/positional SDK and verifies optional JSON/form bodies"]
fn typed_optional_cli_and_sdk_preserve_omission_null_empty_and_required_fields() {
    use std::{
        net::TcpListener,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let cli = tempfile::tempdir().unwrap();
    write_project(&input("rust-progenitor-cli"), cli.path());
    let target = std::env::var_os("MOLEAPI_GENERATED_TEST_TARGET")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| cli.path().join("target"));
    let build = Command::new("cargo")
        .args(["build", "--quiet"])
        .current_dir(cli.path())
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
        let item = json!({"id":7,"note":note});
        let list = json!([note, "x"]);
        let cases = [
            ("object", None, None),
            ("object", Some("application/json"), Some(item.clone())),
            ("empty", Some("application/json"), Some(json!({}))),
            ("nullable", None, None),
            ("nullable", Some("application/json"), Some(Value::Null)),
            ("array", None, None),
            ("array", Some("application/json"), Some(list.clone())),
            ("form", None, None),
            (
                "form",
                Some("application/x-www-form-urlencoded"),
                Some(item.clone()),
            ),
            ("required", Some("application/json"), Some(item.clone())),
            ("object", None, None),
            ("object", Some("application/json"), Some(item.clone())),
            ("nullable", Some("application/json"), Some(Value::Null)),
            ("array", Some("application/json"), Some(list)),
            (
                "form",
                Some("application/x-www-form-urlencoded"),
                Some(item.clone()),
            ),
            ("required", Some("application/json"), Some(item)),
        ];
        for (name, mime, expected) in cases {
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
            assert!(headers.starts_with(&format!("POST /{name} ")));
            let header = |name: &str| {
                headers.lines().find_map(|line| {
                    let (k, v) = line.split_once(':')?;
                    k.eq_ignore_ascii_case(name).then_some(v.trim())
                })
            };
            assert_eq!(header("content-type"), mime);
            assert!(header("transfer-encoding").is_none());
            let length = header("content-length")
                .map(|v| v.parse::<usize>().unwrap())
                .unwrap_or(0);
            let mut body = vec![0; length];
            socket.read_exact(&mut body).unwrap();
            match expected {
                None => assert!(body.is_empty()),
                Some(expected) => {
                    let actual = if mime == Some("application/json") {
                        serde_json::from_slice::<Value>(&body).unwrap()
                    } else {
                        let values = url::form_urlencoded::parse(&body)
                            .into_owned()
                            .collect::<BTreeMap<_, _>>();
                        assert_eq!(values.len(), 2);
                        json!({"id":values["id"].parse::<i32>().unwrap(),"note":values["note"]})
                    };
                    assert_eq!(actual, expected);
                }
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
                panic!("Typed-body CLI timed out");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        child.wait_with_output().unwrap()
    };
    let empty = cli.path().join("empty.json");
    std::fs::write(&empty, "{}").unwrap();
    let null = cli.path().join("null.json");
    std::fs::write(&null, "null").unwrap();
    let array = cli.path().join("array.json");
    std::fs::write(&array, serde_json::to_vec(&json!([note, "x"])).unwrap()).unwrap();
    for args in [
        vec!["object"],
        vec!["object", "--id", "7", "--note", note],
        vec!["empty", "--json-body", empty.to_str().unwrap()],
        vec!["nullable"],
        vec!["nullable", "--json-body", null.to_str().unwrap()],
        vec!["array"],
        vec!["array", "--json-body", array.to_str().unwrap()],
        vec!["form"],
        vec!["form", "--id", "7", "--note", note],
        vec!["required", "--id", "7", "--note", note],
    ] {
        let output = run(&args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
    }
    for args in [
        vec!["required"],
        vec!["object", "--note", note],
        vec!["object", "--json-body", empty.to_str().unwrap()],
        vec!["form", "--note", note],
    ] {
        let output = run(&args);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
    let sdk = tempfile::tempdir().unwrap();
    write_project(&input("rust-progenitor"), sdk.path());
    let manifest = sdk.path().join("Cargo.toml");
    let mut cargo = std::fs::read_to_string(&manifest).unwrap();
    cargo.push_str("tokio = {version=\"1\",features=[\"macros\",\"rt-multi-thread\"]}\n");
    std::fs::write(manifest, cargo).unwrap();
    std::fs::create_dir(sdk.path().join("examples")).unwrap();
    std::fs::write(sdk.path().join("examples/typed.rs"),r#"#[tokio::main] async fn main(){let client=moleapi_sdk::Client::new(&std::env::var("FIXTURE_URL").unwrap());let body=moleapi_sdk::types::Body{id:7,note:Some("鼹鼠 \"quotes\" & +".into())};client.object(None).await.unwrap();client.object(Some(&body)).await.unwrap();client.nullable(Some(None)).await.unwrap();client.array(Some(&vec!["鼹鼠 \"quotes\" & +".into(),"x".into()])).await.unwrap();client.form(Some(&body)).await.unwrap();client.required(&body).await.unwrap();}"#).unwrap();
    let output = Command::new("cargo")
        .args(["run", "--quiet", "--example", "typed"])
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
