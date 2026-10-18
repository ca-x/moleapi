#[allow(dead_code)]
mod common;
use common::{cli, result};

#[tokio::test]
async fn cli_roundtrips_markdown_documents_with_explicit_full_backup() {
    let temporary = tempfile::tempdir().unwrap();
    let database = temporary.path().join("cli.db");
    let input = temporary.path().join("documents.json");
    let safe = temporary.path().join("safe.json");
    let full = temporary.path().join("full.json");
    let source = serde_json::json!({"name":"Docs","data":{"schema_version":1,"collections":[],"environments":[],"active_environment_id":null,"documents":[{"id":"private","name":"Guide","source":"# private-cli-source"}]}});
    std::fs::write(&input, source.to_string()).unwrap();
    let imported = result(
        cli(&[
            "--database",
            database.to_str().unwrap(),
            "import",
            "--format",
            "moleapi",
            "--input",
            input.to_str().unwrap(),
        ])
        .await,
        0,
    );
    let workspace = imported["id"].as_str().unwrap();
    for (output, private) in [(&safe, false), (&full, true)] {
        let mut args = vec![
            "--database",
            database.to_str().unwrap(),
            "export",
            "--workspace",
            workspace,
            "--output",
            output.to_str().unwrap(),
        ];
        if private {
            args.push("--include-secrets");
        }
        let result = cli(&args).await;
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let safe_text = std::fs::read_to_string(&safe).unwrap();
    assert!(!safe_text.contains("private-cli-source"));
    let safe_import = moleapi_formats::import("moleapi", &safe_text).unwrap();
    assert!(safe_import.data.documents[0].source.is_empty());
    let full_text = std::fs::read_to_string(&full).unwrap();
    let exact = moleapi_formats::import("moleapi", &full_text).unwrap();
    assert_eq!(exact.data.documents[0].source, "# private-cli-source");
}
