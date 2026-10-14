use super::*;
use serde_json::json;

fn input() -> ProjectInput {
    let mut paths = serde_json::Map::new();
    let object = json!({"type":"object","required":["id","state","nested"],"properties":{"id":{"type":"integer","minimum":1},"state":{"type":"string","enum":["ready","done"]},"nested":{"type":"object","required":["label"],"properties":{"label":{"type":"string"}}}}});
    let example = json!({"id":7,"state":"ready","nested":{"label":"鼹鼠 \"quotes\" & +"}});
    for (name, schema, media_example, named) in [
        (
            "media",
            json!({"$ref":"#/components/schemas/Payload"}),
            Some(example.clone()),
            false,
        ),
        (
            "named",
            json!({"$ref":"#/components/schemas/Payload"}),
            None,
            true,
        ),
        (
            "schema",
            json!({"type":"integer","minimum":0,"example":0}),
            None,
            false,
        ),
        (
            "default",
            json!({"type":"boolean","default":false}),
            None,
            false,
        ),
        (
            "empty",
            json!({"type":"array","items":{"type":"string"}}),
            None,
            false,
        ),
        (
            "null",
            json!({"type":"string","nullable":true,"example":null}),
            None,
            false,
        ),
        (
            "invalid",
            json!({"type":"string","minLength":3,"example":"x"}),
            None,
            false,
        ),
        ("missing", object.clone(), None, false),
    ] {
        let mut media = json!({"schema":schema});
        if let Some(example) = media_example {
            media["example"] = example;
        }
        if named {
            media["examples"] = json!({"sample":{"$ref":"#/components/examples/Sample"}});
        }
        paths.insert(format!("/{name}/{{id}}"),json!({"post":{"operationId":name,"parameters":[{"in":"path","name":"id","required":true,"schema":{"type":"integer","format":"int32"}}],"requestBody":{"required":true,"content":{"application/json":media}},"responses":{"204":{"description":"Accepted"}}}}));
    }
    for (name, mime, schema, example) in [
        (
            "form",
            "application/x-www-form-urlencoded",
            json!({"type":"object","required":["note"],"properties":{"note":{"type":"string"},"state":{"type":"string","enum":["ready","done"]}}}),
            json!({"note":"鼹鼠 \"quotes\" & +","state":"ready"}),
        ),
        (
            "invalid_array",
            "application/json",
            json!({"type":"array","minItems":2,"items":{"type":"integer"}}),
            json!([1]),
        ),
        (
            "empty_string",
            "application/json",
            json!({"type":"string"}),
            json!(""),
        ),
    ] {
        paths.insert(format!("/{name}/{{id}}"), json!({"post":{"operationId":name,"parameters":[{"in":"path","name":"id","required":true,"schema":{"type":"integer","format":"int32"}}],"requestBody":{"required":true,"content":{mime:{"schema":schema,"example":example}}},"responses":{"204":{"description":"Accepted"}}}}));
    }
    ProjectInput {
        target: "rust-progenitor-cli".into(),
        options: BTreeMap::new(),
        templates: None,
        include_secrets: false,
        specification: json!({"openapi":"3.0.3","info":{"title":"Body templates","version":"1"},"paths":paths,"components":{"schemas":{"Payload":object},"examples":{"Sample":{"value":example}}}}),
    }
}

#[test]
fn body_template_artifacts_emit_validated_offline_execution_and_sources() {
    let artifact = native_project(&input()).unwrap();
    let cli = &artifact
        .files
        .iter()
        .find(|file| file.path == "src/cli.rs")
        .unwrap()
        .content;
    assert!(cli.contains(".exclusive(true)"));
    assert!(cli.contains("pub fn body_template"));
    assert!(cli.contains("jsonschema::options()"));
    assert!(!cli.contains(".help(\"XXX\")"));
}

#[test]
#[ignore = "Compiles generated CLI, exercises offline body templates and real file roundtrip"]
fn body_templates_are_offline_validated_exclusive_and_reusable_request_files() {
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
    let executable = target
        .join("debug")
        .join(format!("moleapi_sdk{}", std::env::consts::EXE_SUFFIX));
    let run = |args: &[&str]| {
        let mut child = Command::new(&executable)
            .env_remove("MOLEAPI_API_URL")
            .env_remove("MOLEAPI_API_TOKEN")
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
                panic!("Template CLI timed out");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        child.wait_with_output().unwrap()
    };
    let expected = json!({"id":7,"state":"ready","nested":{"label":"鼹鼠 \"quotes\" & +"}});
    let mut saved = Vec::new();
    for (operation, value) in [
        ("media", expected.clone()),
        ("named", expected.clone()),
        ("schema", json!(0)),
        ("default", json!(false)),
        ("empty", json!([])),
        ("null", Value::Null),
        ("empty_string", json!("")),
        (
            "form",
            json!({"note":"鼹鼠 \"quotes\" & +","state":"ready"}),
        ),
    ] {
        let output = run(&[operation, "--json-body-template"]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stdout).unwrap(),
            value
        );
        if operation == "media" {
            saved = output.stdout;
        }
    }
    let bypass = run(&[
        "--base-url",
        "invalid URL",
        "--token",
        "invalid\nsecret",
        "media",
        "--json-body-template",
    ]);
    assert!(
        bypass.status.success(),
        "{}",
        String::from_utf8_lossy(&bypass.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&bypass.stdout).unwrap(),
        expected
    );
    for operation in ["invalid", "missing"] {
        let output = run(&[operation, "--json-body-template"]);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
    }
    let invalid_array = run(&["invalid_array", "--json-body-template"]);
    assert!(!invalid_array.status.success());
    assert!(invalid_array.stdout.is_empty());
    assert!(String::from_utf8_lossy(&invalid_array.stderr).contains("violates its schema"));
    let body = project.path().join("body.json");
    std::fs::write(&body, saved).unwrap();
    for args in [
        vec![
            "media",
            "--json-body-template",
            "--json-body",
            body.to_str().unwrap(),
        ],
        vec!["media", "--json-body-template", "--id", "7"],
    ] {
        let output = run(&args);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
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
        assert!(headers.starts_with("POST /media/9 "));
        let length = headers
            .lines()
            .find_map(|line| {
                let (k, v) = line.split_once(':')?;
                k.eq_ignore_ascii_case("content-length")
                    .then(|| v.trim().parse::<usize>().unwrap())
            })
            .unwrap();
        let mut body = vec![0; length];
        socket.read_exact(&mut body).unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&body).unwrap(), expected);
        socket
            .write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n")
            .unwrap();
    });
    let execute = run(&[
        "--base-url",
        &url,
        "media",
        "--id",
        "9",
        "--json-body",
        body.to_str().unwrap(),
    ]);
    assert!(
        execute.status.success(),
        "{}",
        String::from_utf8_lossy(&execute.stderr)
    );
    assert!(execute.stdout.is_empty());
    server.join().unwrap();
    let form_template = run(&["form", "--json-body-template"]);
    assert!(form_template.status.success());
    let form_file = project.path().join("form.json");
    std::fs::write(&form_file, form_template.stdout).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let form_url = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
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
        assert!(headers.starts_with("POST /form/9 "));
        let header = |name: &str| {
            headers.lines().find_map(|line| {
                let (key, value) = line.split_once(':')?;
                key.eq_ignore_ascii_case(name).then_some(value.trim())
            })
        };
        assert_eq!(
            header("content-type"),
            Some("application/x-www-form-urlencoded")
        );
        let mut body = vec![0; header("content-length").unwrap().parse::<usize>().unwrap()];
        socket.read_exact(&mut body).unwrap();
        let values = url::form_urlencoded::parse(&body)
            .into_owned()
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            values,
            BTreeMap::from([
                ("note".into(), "鼹鼠 \"quotes\" & +".into()),
                ("state".into(), "ready".into())
            ])
        );
        socket
            .write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n")
            .unwrap();
    });
    let form_call = run(&[
        "--base-url",
        &form_url,
        "form",
        "--id",
        "9",
        "--json-body",
        form_file.to_str().unwrap(),
    ]);
    assert!(
        form_call.status.success(),
        "{}",
        String::from_utf8_lossy(&form_call.stderr)
    );
    server.join().unwrap();
    let missing_url = run(&["media", "--id", "9", "--json-body", body.to_str().unwrap()]);
    assert!(!missing_url.status.success());
    assert!(missing_url.stdout.is_empty());
}
