#[allow(dead_code)]
mod common;
use common::{cli, result};
use serde_json::{Value, json};

#[tokio::test]
async fn cli_imports_and_exports_postman_variable_scopes_in_an_existing_workspace() {
    let temporary = tempfile::tempdir().unwrap();
    let database = temporary.path().join("cli.db");
    let workspace_file = temporary.path().join("workspace.json");
    let environment_file = temporary
        .path()
        .join("development.postman_environment.json");
    let globals_file = temporary.path().join("globals.postman_globals.json");
    let safe_file = temporary.path().join("safe.json");
    let full_file = temporary.path().join("full.json");
    std::fs::write(&workspace_file,json!({"name":"Variable workspace","data":{"schema_version":1,"collections":[],"environments":[],"active_environment_id":null}}).to_string()).unwrap();
    std::fs::write(
        &environment_file,
        include_str!("../../formats/tests/fixtures/postman_environment.json"),
    )
    .unwrap();
    std::fs::write(
        &globals_file,
        include_str!("../../formats/tests/fixtures/postman_globals.json"),
    )
    .unwrap();
    let saved = result(
        cli(&[
            "--database",
            database.to_str().unwrap(),
            "import",
            "--input",
            workspace_file.to_str().unwrap(),
        ])
        .await,
        0,
    );
    let id = saved["id"].as_str().unwrap();
    let imported = result(
        cli(&[
            "--database",
            database.to_str().unwrap(),
            "variables",
            "import",
            "--workspace",
            id,
            "--input",
            environment_file.to_str().unwrap(),
        ])
        .await,
        0,
    );
    assert_eq!(imported["scope"], "environment");
    let environments = result(
        cli(&[
            "--database",
            database.to_str().unwrap(),
            "list",
            "environments",
            "--workspace",
            id,
        ])
        .await,
        0,
    );
    assert_eq!(environments.as_array().unwrap().len(), 1);
    let environment_id = environments[0]["id"].as_str().unwrap();
    let output = cli(&[
        "--database",
        database.to_str().unwrap(),
        "variables",
        "export",
        "--workspace",
        id,
        "--scope",
        "environment",
        "--environment",
        environment_id,
        "--output",
        safe_file.to_str().unwrap(),
    ])
    .await;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let safe = std::fs::read_to_string(&safe_file).unwrap();
    assert!(!safe.contains("environment-secret"));
    let output = cli(&[
        "--database",
        database.to_str().unwrap(),
        "variables",
        "export",
        "--workspace",
        id,
        "--scope",
        "environment",
        "--environment",
        environment_id,
        "--output",
        full_file.to_str().unwrap(),
        "--include-secrets",
    ])
    .await;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let full = std::fs::read_to_string(&full_file).unwrap();
    assert!(full.contains("environment-secret"));
    let refused = cli(&[
        "--database",
        database.to_str().unwrap(),
        "variables",
        "import",
        "--workspace",
        id,
        "--input",
        globals_file.to_str().unwrap(),
    ])
    .await;
    assert!(!refused.status.success());
    let imported = result(
        cli(&[
            "--database",
            database.to_str().unwrap(),
            "variables",
            "import",
            "--workspace",
            id,
            "--input",
            globals_file.to_str().unwrap(),
            "--replace-globals",
        ])
        .await,
        0,
    );
    assert_eq!(imported["scope"], "globals");
    let globals_file_out = temporary.path().join("exported-globals.json");
    let output = cli(&[
        "--database",
        database.to_str().unwrap(),
        "variables",
        "export",
        "--workspace",
        id,
        "--scope",
        "globals",
        "--output",
        globals_file_out.to_str().unwrap(),
    ])
    .await;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let globals: Value =
        serde_json::from_str(&std::fs::read_to_string(globals_file_out).unwrap()).unwrap();
    assert_eq!(globals["_postman_variable_scope"], "globals");
    assert_eq!(globals["values"][0]["value"], "东京");
}
