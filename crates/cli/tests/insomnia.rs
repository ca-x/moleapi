#[allow(dead_code)]
mod common;
use common::{cli, result};

#[tokio::test]
async fn cli_imports_insomnia_v4_and_screens_original_source_by_default() {
    let temporary = tempfile::tempdir().unwrap();
    let input = temporary.path().join("collection.insomnia.json");
    let database = temporary.path().join("cli.db");
    let safe = temporary.path().join("safe.json");
    std::fs::write(
        &input,
        include_str!("../../formats/tests/fixtures/insomnia_v4.json"),
    )
    .unwrap();
    let saved = result(
        cli(&[
            "--database",
            database.to_str().unwrap(),
            "import",
            "--format",
            "insomnia",
            "--input",
            input.to_str().unwrap(),
        ])
        .await,
        0,
    );
    let id = saved["id"].as_str().unwrap();
    let listed = result(
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
    assert_eq!(listed.as_array().unwrap().len(), 2);
    let output = cli(&[
        "--database",
        database.to_str().unwrap(),
        "export",
        "--workspace",
        id,
        "--output",
        safe.to_str().unwrap(),
    ])
    .await;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = std::fs::read_to_string(safe).unwrap();
    assert!(
        !text.contains("basic-secret")
            && !text.contains("bearer-secret")
            && !text.contains("insomnia-secret")
    );
    let backup = moleapi_formats::import("moleapi", &text).unwrap();
    assert!(backup.data.specifications[0].source.is_empty());
    let mut invalid: serde_json::Value = serde_json::from_str(include_str!(
        "../../formats/tests/fixtures/insomnia_v4.json"
    ))
    .unwrap();
    invalid["resources"][6]["body"]["mimeType"] = serde_json::json!("multipart/form-data");
    std::fs::write(&input, invalid.to_string()).unwrap();
    let rejected = cli(&[
        "--database",
        database.to_str().unwrap(),
        "import",
        "--format",
        "insomnia",
        "--input",
        input.to_str().unwrap(),
    ])
    .await;
    assert!(!rejected.status.success());
    assert!(!String::from_utf8_lossy(&rejected.stderr).contains("basic-secret"));
}
