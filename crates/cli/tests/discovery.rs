use serde_json::Value;
use std::process::Command;
#[test]
fn command_schema_exposes_options_and_invalid_modes_do_not_create_files() {
    let binary = env!("CARGO_BIN_EXE_moleapi-cli");
    let output = Command::new(binary).arg("schema").output().unwrap();
    assert!(output.status.success());
    let schema: Value = serde_json::from_slice(&output.stdout).unwrap();
    let commands = schema["subcommands"].as_array().unwrap();
    for name in [
        "login", "list", "import", "export", "run", "report", "snippet",
    ] {
        assert!(commands.iter().any(|command| command["name"] == name));
    }
    let run = commands
        .iter()
        .find(|command| command["name"] == "run")
        .unwrap();
    let args = run["arguments"].as_array().unwrap();
    assert!(args.iter().any(|arg| arg["long"] == "request"));
    assert!(
        args.iter()
            .any(|arg| arg["long"] == "server" && arg["global"] == true)
    );
    let temporary = tempfile::tempdir().unwrap();
    let database = temporary.path().join("must-not-exist.db");
    for arguments in [
        vec![
            "--database",
            database.to_str().unwrap(),
            "--server",
            "https://example.com",
            "list",
            "workspaces",
        ],
        vec![
            "run",
            "--workspace",
            "w",
            "--scenario",
            "s",
            "--request",
            "r",
        ],
        vec![
            "--server",
            "https://private:secret@example.com",
            "list",
            "workspaces",
        ],
    ] {
        let output = Command::new(binary).args(arguments).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("private:secret"));
    }
    assert!(!database.exists());
}
