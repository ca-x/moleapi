use serde_json::Value;
use std::process::{Command, Output};
fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_moleapi-cli"))
        .args(args)
        .env_remove("MOLEAPI_SKILL_ABSENT_CREDENTIAL")
        .output()
        .unwrap()
}
fn success(output: Output) -> Vec<u8> {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}
#[test]
fn embedded_skill_exports_valid_source_and_bundle_without_database_or_credentials() {
    let temporary = tempfile::tempdir().unwrap();
    let database = temporary.path().join("no-storage/db.sqlite");
    let exported = temporary.path().join("SKILL.md");
    let args = [
        "--database",
        database.to_str().unwrap(),
        "skill",
        "--output",
        exported.to_str().unwrap(),
    ];
    success(cli(&args));
    assert!(!database.exists());
    let expected = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/skills/moleapi/SKILL.md"),
    )
    .unwrap();
    assert_eq!(std::fs::read(&exported).unwrap(), expected);
    let repeated = cli(&args);
    assert_eq!(repeated.status.code(), Some(2));
    assert_eq!(std::fs::read(&exported).unwrap(), expected);
    let bundle: Value = serde_json::from_slice(&success(cli(&[
        "--server",
        "https://unreachable.invalid",
        "--token-env",
        "MOLEAPI_SKILL_ABSENT_CREDENTIAL",
        "skill",
        "--format",
        "json",
    ])))
    .unwrap();
    let files = bundle["files"].as_array().unwrap();
    assert_eq!(files.len(), 2);
    assert_eq!(files[0]["path"], "SKILL.md");
    assert_eq!(files[0]["content"].as_str().unwrap().as_bytes(), expected);
    assert_eq!(files[1]["path"], "agents/openai.yaml");
    assert!(files[1]["content"].as_str().unwrap().contains("$moleapi"));
    let metadata = success(cli(&["--database", database.to_str().unwrap(), "schema"]));
    let metadata: Value = serde_json::from_slice(&metadata).unwrap();
    assert!(!database.exists());
    let commands = metadata["subcommands"].as_array().unwrap();
    let run = commands
        .iter()
        .find(|command| command["name"] == "run")
        .unwrap();
    let args = run["arguments"].as_array().unwrap();
    let workspace = args.iter().find(|arg| arg["id"] == "workspace").unwrap();
    assert!(
        workspace["conflicts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id == "input")
    );
    assert_eq!(workspace["min_values"], 1);
    for arguments in [
        vec!["--database", database.to_str().unwrap(), "run"],
        vec![
            "--database",
            database.to_str().unwrap(),
            "snippet",
            "--workspace",
            "unselected",
        ],
        vec![
            "--database",
            database.to_str().unwrap(),
            "snippet",
            "--request",
            "unselected",
        ],
    ] {
        assert_eq!(cli(&arguments).status.code(), Some(2));
        assert!(!database.exists());
    }
}
#[test]
fn request_metadata_supports_exact_agent_selection_without_saved_urls_or_bodies() {
    let temporary = tempfile::tempdir().unwrap();
    let database = temporary.path().join("resources.db");
    let input = temporary.path().join("request.txt");
    std::fs::write(&input,"curl -X POST 'https://private.example.com/?token=secret-query' -H 'Authorization: Bearer secret-auth' -d 'secret-body'").unwrap();
    let workspace: Value = serde_json::from_slice(&success(cli(&[
        "--database",
        database.to_str().unwrap(),
        "import",
        "--input",
        input.to_str().unwrap(),
        "--format",
        "curl",
    ])))
    .unwrap();
    let output = success(cli(&[
        "--database",
        database.to_str().unwrap(),
        "list",
        "requests",
        "--workspace",
        workspace["id"].as_str().unwrap(),
    ]));
    let requests: Value = serde_json::from_slice(&output).unwrap();
    assert!(requests[0]["id"].is_string());
    assert!(requests[0]["collection_id"].is_string());
    assert_eq!(requests[0]["method"], "POST");
    let snippet = cli(&[
        "--database",
        database.to_str().unwrap(),
        "snippet",
        "--workspace",
        workspace["id"].as_str().unwrap(),
        "--request",
        requests[0]["id"].as_str().unwrap(),
        "--target",
        "go",
        "--client",
        "native",
    ]);
    assert!(snippet.status.success());
    let warnings = String::from_utf8(snippet.stderr).unwrap();
    assert!(warnings.contains("TLS"));
    let code = String::from_utf8(snippet.stdout).unwrap();
    assert!(!code.contains("Warning:"));
    assert!(!warnings.contains("secret-auth") && !warnings.contains("secret-body"));
    let text = String::from_utf8(output).unwrap();
    for secret in [
        "private.example.com",
        "secret-query",
        "secret-auth",
        "secret-body",
    ] {
        assert!(!text.contains(secret), "leaked {secret}");
    }
}
