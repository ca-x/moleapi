use serde_json::Value;
use std::process::{Command, Output};

fn cli(database: &std::path::Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_moleapi-cli"))
        .arg("--database")
        .arg(database)
        .args(arguments)
        .output()
        .unwrap()
}
fn json(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn offline_cli_exposes_reference_catalog_and_generates_saved_request_code() {
    let temporary = tempfile::tempdir().unwrap();
    let database = temporary.path().join("cli.db");
    let catalog = json(cli(&database, &["snippet"]));
    let targets = catalog["targets"].as_array().unwrap();
    assert_eq!(targets.len(), 23);
    assert_eq!(
        targets
            .iter()
            .map(|target| target["clients"].as_array().unwrap().len())
            .sum::<usize>(),
        55
    );
    for language in [
        "c",
        "csharp",
        "clojure",
        "dart",
        "fsharp",
        "go",
        "http",
        "java",
        "js",
        "julia",
        "kotlin",
        "node",
        "objc",
        "ocaml",
        "php",
        "powershell",
        "python",
        "r",
        "ruby",
        "rust",
        "shell",
        "swift",
    ] {
        assert!(
            targets.iter().any(|target| target["target"] == language),
            "Missing {language}"
        );
    }
    let source = temporary.path().join("request.txt");
    std::fs::write(&source, "curl -X POST 'https://example.com/echo?a=1' -H 'Content-Type: application/json' -d '{\"message\":\"hello\"}'").unwrap();
    let workspace = json(cli(
        &database,
        &[
            "import",
            "--input",
            source.to_str().unwrap(),
            "--format",
            "curl",
            "--name",
            "Code examples",
        ],
    ));
    let workspace_id = workspace["id"].as_str().unwrap();
    let collections = json(cli(
        &database,
        &["list", "collections", "--workspace", workspace_id],
    ));
    assert_eq!(collections[0]["requests"], 1);
    // Request selection can use the original imported name; export gives its exact ID.
    let export = temporary.path().join("workspace.json");
    let exported = cli(
        &database,
        &[
            "export",
            "--workspace",
            workspace_id,
            "--output",
            export.to_str().unwrap(),
        ],
    );
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    let saved: Value = serde_json::from_slice(&std::fs::read(&export).unwrap()).unwrap();
    let request_id = saved["data"]["collections"][0]["requests"][0]["id"]
        .as_str()
        .unwrap();
    let output = temporary.path().join("request.py");
    let args = [
        "snippet",
        "--workspace",
        workspace_id,
        "--request",
        request_id,
        "--target",
        "python",
        "--client",
        "requests",
        "--output",
        output.to_str().unwrap(),
    ];
    let generated = cli(&database, &args);
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    let code = std::fs::read_to_string(&output).unwrap();
    assert!(
        code.contains("requests")
            && code.contains("example.com")
            && code.to_ascii_uppercase().contains("POST")
    );
    let duplicate = cli(&database, &args);
    assert_eq!(duplicate.status.code(), Some(2));
    assert_eq!(std::fs::read_to_string(&output).unwrap(), code);
}
