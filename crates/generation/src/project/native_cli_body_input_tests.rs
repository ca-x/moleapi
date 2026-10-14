use super::*;
use serde_json::json;

fn input() -> ProjectInput {
    let mut paths = serde_json::Map::new();
    for (name, mime, required) in [
        ("optional", "application/json", false),
        ("required", "application/json", true),
        ("form", "application/x-www-form-urlencoded", false),
    ] {
        paths.insert(format!("/{name}"),json!({"post":{"operationId":name,"requestBody":{"required":required,"content":{mime:{"schema":{"$ref":"#/components/schemas/Body"}}}},"responses":{"204":{"description":"Accepted"}}}}));
    }
    paths.insert("/reserved".into(),json!({"post":{"operationId":"reserved","requestBody":{"required":true,"content":{"application/json":{"schema":{"type":"object","required":["json_body"],"properties":{"json_body":{"type":"string"},"json_body_template":{"type":"string"}}}}}},"responses":{"204":{"description":"Accepted"}}}}));
    paths.insert("/collision/{id}".into(),json!({"post":{"operationId":"collision","parameters":[
        {"in":"path","name":"id","required":true,"schema":{"type":"integer","format":"int32"}},
        {"in":"header","name":"note","required":true,"schema":{"type":"string"}},
        {"in":"query","name":"mode","schema":{"type":"string"}}
    ],"requestBody":{"required":true,"content":{"application/json":{"schema":{"$ref":"#/components/schemas/Body"}}}},"responses":{"204":{"description":"Accepted"}}}}));
    ProjectInput {
        target: "rust-progenitor-cli".into(),
        options: BTreeMap::new(),
        templates: None,
        include_secrets: false,
        specification: json!({"openapi":"3.0.3","info":{"title":"Body input fixture","version":"1"},"paths":paths,"components":{"schemas":{"Body":{"type":"object","required":["id"],"properties":{"id":{"type":"integer","format":"int32"},"note":{"type":"string"}}}}}}),
    }
}

#[test]
fn body_property_flags_explicitly_conflict_with_whole_body_files() {
    let artifact = native_project(&input()).unwrap();
    let cli = &artifact
        .files
        .iter()
        .find(|file| file.path == "src/cli.rs")
        .unwrap()
        .content;
    assert!(cli.contains(".conflicts_with(\"json-body\")"));
}

#[test]
#[ignore = "Compiles generated CLI and verifies file/field conflicts and parameter collisions"]
fn typed_body_input_conflicts_preserve_file_field_and_non_body_arguments() {
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
    let note = "鼹鼠 \"quotes\" & +";
    let file_body = json!({"id":7,"note":note});
    let fields = json!({"id":8,"note":"fields"});
    let file = project.path().join("body.json");
    std::fs::write(&file, serde_json::to_vec(&file_body).unwrap()).unwrap();
    let file_path = file.to_str().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        for (path, mime, expected) in [
            ("/optional", "application/json", file_body.clone()),
            ("/optional", "application/json", fields.clone()),
            ("/required", "application/json", file_body.clone()),
            ("/required", "application/json", fields.clone()),
            (
                "/form",
                "application/x-www-form-urlencoded",
                file_body.clone(),
            ),
            ("/form", "application/x-www-form-urlencoded", fields),
            ("/collision/99?mode=wire", "application/json", file_body),
            (
                "/reserved",
                "application/json",
                json!({"json_body":"preserved","json_body_template":"also preserved"}),
            ),
        ] {
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
            assert!(headers.starts_with(&format!("POST {path} ")));
            let header = |name: &str| {
                headers.lines().find_map(|line| {
                    let (k, v) = line.split_once(':')?;
                    k.eq_ignore_ascii_case(name).then_some(v.trim())
                })
            };
            assert_eq!(header("content-type"), Some(mime));
            assert_eq!(header("authorization"), Some("Bearer input-fixture"));
            if path.starts_with("/collision") {
                assert_eq!(header("note"), Some("header-note"));
            }
            let length = header("content-length").unwrap().parse::<usize>().unwrap();
            let mut body = vec![0; length];
            socket.read_exact(&mut body).unwrap();
            let actual = if mime == "application/json" {
                serde_json::from_slice::<Value>(&body).unwrap()
            } else {
                let values = url::form_urlencoded::parse(&body)
                    .into_owned()
                    .collect::<BTreeMap<_, _>>();
                assert_eq!(values.len(), 2);
                json!({"id":values["id"].parse::<i32>().unwrap(),"note":values["note"]})
            };
            assert_eq!(actual, expected);
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
            .args(["--base-url", &url, "--token", "input-fixture"])
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
                panic!("Body-input CLI timed out");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        child.wait_with_output().unwrap()
    };
    for operation in ["optional", "required", "form"] {
        for args in [
            vec![operation, "--json-body", file_path],
            vec![operation, "--id", "8", "--note", "fields"],
        ] {
            let output = run(&args);
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stdout.is_empty());
        }
    }
    let collision = run(&[
        "collision",
        "--id",
        "99",
        "--note",
        "header-note",
        "--mode",
        "wire",
        "--json-body",
        file_path,
    ]);
    assert!(
        collision.status.success(),
        "{}",
        String::from_utf8_lossy(&collision.stderr)
    );
    let reserved_file = project.path().join("reserved.json");
    std::fs::write(
        &reserved_file,
        serde_json::to_vec(&json!({"json_body":"preserved","json_body_template":"also preserved"}))
            .unwrap(),
    )
    .unwrap();
    let reserved = run(&["reserved", "--json-body", reserved_file.to_str().unwrap()]);
    assert!(
        reserved.status.success(),
        "{}",
        String::from_utf8_lossy(&reserved.stderr)
    );
    let missing = project.path().join("unread-missing.json");
    let missing_path = missing.to_str().unwrap();
    for operation in ["optional", "required", "form"] {
        for flag in ["--id", "--note"] {
            let value = if flag == "--id" { "8" } else { "fields" };
            let conflict = run(&[operation, "--json-body", missing_path, flag, value]);
            assert!(!conflict.status.success());
            assert!(conflict.stdout.is_empty());
            let error = String::from_utf8_lossy(&conflict.stderr);
            assert!(error.contains("cannot be used with"), "{error}");
            assert!(!error.contains("failed to read"));
        }
    }
    let required = run(&["required"]);
    assert!(!required.status.success());
    assert!(required.stdout.is_empty());
    server.join().unwrap();
}
