#[allow(dead_code)]
mod common;
use common::{cli, result};

#[tokio::test]
async fn cli_imports_postman20_object_auth_and_screens_default_backup() {
    let temporary = tempfile::tempdir().unwrap();
    let input = temporary.path().join("collection-v2.json");
    let database = temporary.path().join("cli.db");
    let safe = temporary.path().join("safe.json");
    let full = temporary.path().join("full.json");
    let source = include_str!("../../formats/tests/fixtures/postman20.json");
    std::fs::write(&input, source).unwrap();
    let saved = result(
        cli(&[
            "--database",
            database.to_str().unwrap(),
            "import",
            "--format",
            "postman",
            "--input",
            input.to_str().unwrap(),
        ])
        .await,
        0,
    );
    let id = saved["id"].as_str().unwrap();
    let requests = result(
        cli(&[
            "--database",
            database.to_str().unwrap(),
            "list",
            "requests",
            "--workspace",
            id,
        ])
        .await,
        0,
    );
    assert_eq!(requests.as_array().unwrap().len(), 2);
    for (output, secrets) in [(&safe, false), (&full, true)] {
        let mut args = vec![
            "--database",
            database.to_str().unwrap(),
            "export",
            "--workspace",
            id,
            "--output",
            output.to_str().unwrap(),
        ];
        if secrets {
            args.push("--include-secrets");
        }
        let response = cli(&args).await;
        assert!(
            response.status.success(),
            "{}",
            String::from_utf8_lossy(&response.stderr)
        );
    }
    let safe = std::fs::read_to_string(safe).unwrap();
    assert!(!safe.contains("collection-secret"));
    assert!(!safe.contains("bearer-secret"));
    assert!(!safe.contains("api-key-secret"));
    let full = std::fs::read_to_string(full).unwrap();
    assert!(full.contains("collection-secret"));
    assert!(full.contains("bearer-secret"));
    assert!(full.contains("api-key-secret"));
    assert!(full.contains("\"2.0.0\""));
}
