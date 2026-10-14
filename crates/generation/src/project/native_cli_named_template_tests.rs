use super::*;
use serde_json::json;

fn input() -> ProjectInput {
    let mut paths = serde_json::Map::new();
    for (name, mime, schema, examples) in [
        (
            "boolean",
            "application/json",
            json!({"type":"boolean","nullable":true}),
            json!({"z_first":{"value":true},"a_second":{"$ref":"#/components/examples/False"},"null":{"value":null},"蓝 + /":{"value":false},"invalid":{"value":"wrong"},"external":{"externalValue":"http://127.0.0.1:1/do-not-fetch"}}),
        ),
        (
            "number",
            "application/json",
            json!({"type":"integer","minimum":0}),
            json!({"first":{"value":5},"zero":{"value":0}}),
        ),
        (
            "string",
            "application/json",
            json!({"type":"string","nullable":true}),
            json!({"first":{"value":"default"},"empty":{"value":""},"null":{"value":null}}),
        ),
        (
            "form",
            "application/x-www-form-urlencoded",
            json!({"type":"object","required":["id","note"],"properties":{"id":{"type":"integer","format":"int32"},"note":{"type":"string"}}}),
            json!({"first":{"value":{"id":1,"note":"first"}},"selected":{"$ref":"#/components/examples/Form"}}),
        ),
    ] {
        paths.insert(format!("/{name}"),json!({"post":{"operationId":name,"requestBody":{"required":true,"content":{mime:{"schema":schema,"examples":examples}}},"responses":{"204":{"description":"Accepted"}}}}));
    }
    paths.insert("/null_override".into(),json!({"post":{"operationId":"null_override","requestBody":{"required":true,"content":{"application/json":{"schema":{"type":"boolean","nullable":true,"default":false},"example":null}}},"responses":{"204":{"description":"Accepted"}}}}));
    paths.insert("/schema_null_override".into(),json!({"post":{"operationId":"schema_null_override","requestBody":{"required":true,"content":{"application/json":{"schema":{"type":"string","nullable":true,"example":null,"default":"fallback"}}}},"responses":{"204":{"description":"Accepted"}}}}));
    ProjectInput {
        target: "rust-progenitor-cli".into(),
        options: BTreeMap::new(),
        templates: None,
        include_secrets: false,
        specification: json!({"openapi":"3.0.3","info":{"title":"Named templates","version":"1"},"paths":paths,"components":{"examples":{"False":{"value":false},"Form":{"value":{"id":9,"note":"selected鼹鼠 + &"}}}}}),
    }
}

#[test]
fn named_template_artifacts_preserve_explicit_selection_and_falsy_wrappers() {
    let artifact = native_project(&input()).unwrap();
    let cli = &artifact
        .files
        .iter()
        .find(|file| file.path == "src/cli.rs")
        .unwrap()
        .content;
    assert!(cli.contains("pub fn body_template_named"));
    assert!(cli.contains(".require_equals(true)"));
    assert!(cli.contains("entry.get(\"value\")"));
}

#[test]
#[ignore = "Compiles generated CLI, selects named offline examples and calls saved templates"]
fn named_body_templates_preserve_order_selection_and_json_form_wire_values() {
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
                panic!("Named template CLI timed out");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        child.wait_with_output().unwrap()
    };
    for (op, option, expected) in [
        ("boolean", "--json-body-template", json!(true)),
        ("boolean", "--json-body-template=a_second", json!(false)),
        ("boolean", "--json-body-template=null", Value::Null),
        ("boolean", "--json-body-template=蓝 + /", json!(false)),
        ("number", "--json-body-template=zero", json!(0)),
        ("string", "--json-body-template=empty", json!("")),
        ("string", "--json-body-template=null", Value::Null),
        ("null_override", "--json-body-template", Value::Null),
        ("schema_null_override", "--json-body-template", Value::Null),
    ] {
        let output = run(&[op, option]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stdout).unwrap(),
            expected
        );
    }
    for name in ["unknown", "external", "invalid"] {
        let option = format!("--json-body-template={name}");
        let output = run(&["boolean", &option]);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
    }
    let conflict = run(&["form", "--json-body-template=selected", "--id", "9"]);
    assert!(!conflict.status.success());
    assert!(conflict.stdout.is_empty());
    let file = project.path().join("false.json");
    let output = run(&["boolean", "--json-body-template=a_second"]);
    assert!(output.status.success());
    std::fs::write(&file, output.stdout).unwrap();
    let form_file = project.path().join("form.json");
    let output = run(&["form", "--json-body-template=selected"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::write(&form_file, output.stdout).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        for index in 0..2 {
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
            let header = |name: &str| {
                headers.lines().find_map(|line| {
                    let (k, v) = line.split_once(':')?;
                    k.eq_ignore_ascii_case(name).then_some(v.trim())
                })
            };
            let mut body = vec![0; header("content-length").unwrap().parse::<usize>().unwrap()];
            socket.read_exact(&mut body).unwrap();
            if index == 0 {
                assert!(headers.starts_with("POST /boolean "));
                assert_eq!(header("content-type"), Some("application/json"));
                assert_eq!(body, b"false");
            } else {
                assert!(headers.starts_with("POST /form "));
                assert_eq!(
                    header("content-type"),
                    Some("application/x-www-form-urlencoded")
                );
                let values = url::form_urlencoded::parse(&body)
                    .into_owned()
                    .collect::<BTreeMap<_, _>>();
                assert_eq!(
                    values,
                    BTreeMap::from([
                        ("id".into(), "9".into()),
                        ("note".into(), "selected鼹鼠 + &".into())
                    ])
                );
            }
            socket
                .write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n")
                .unwrap();
        }
    });
    for (op, path) in [("boolean", &file), ("form", &form_file)] {
        let output = run(&[
            "--base-url",
            &url,
            op,
            "--json-body",
            path.to_str().unwrap(),
        ]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
    }
    server.join().unwrap();
    let help = run(&["boolean", "--help"]);
    assert!(help.status.success());
    assert!(String::from_utf8(help.stdout).unwrap().contains("=EXAMPLE"));
}
