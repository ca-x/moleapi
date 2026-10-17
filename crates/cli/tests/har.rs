#[allow(dead_code)]
mod common;
use common::{cli, result};

#[tokio::test]
async fn cli_imports_har_file_and_default_export_screens_recorded_credentials() {
    let temporary = tempfile::tempdir().unwrap();
    let input = temporary.path().join("capture.har");
    let database = temporary.path().join("cli.db");
    let safe = temporary.path().join("safe.json");
    std::fs::write(
        &input,
        include_str!("../../formats/tests/fixtures/capture.har"),
    )
    .unwrap();
    let saved = result(
        cli(&[
            "--database",
            database.to_str().unwrap(),
            "import",
            "--format",
            "har",
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
    assert_eq!(listed.as_array().unwrap().len(), 3);
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
    let source = std::fs::read_to_string(safe).unwrap();
    assert!(!source.contains("har-secret"));
    assert!(!source.contains("cookie-secret"));
    let har_output = temporary.path().join("safe.har");
    let exported = cli(&[
        "--database",
        database.to_str().unwrap(),
        "export",
        "--workspace",
        id,
        "--format",
        "har",
        "--output",
        har_output.to_str().unwrap(),
    ])
    .await;
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    let har_text = std::fs::read_to_string(har_output).unwrap();
    assert!(!har_text.contains("har-secret"));
    assert_eq!(
        moleapi_formats::import("har", &har_text)
            .unwrap()
            .data
            .collections[0]
            .requests
            .len(),
        3
    );
    let mut unsupported: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&input).unwrap()).unwrap();
    unsupported["log"]["entries"][1]["request"]["postData"]["mimeType"] =
        serde_json::json!("multipart/form-data");
    std::fs::write(&input, unsupported.to_string()).unwrap();
    let rejected = cli(&[
        "--database",
        database.to_str().unwrap(),
        "import",
        "--format",
        "har",
        "--input",
        input.to_str().unwrap(),
    ])
    .await;
    assert!(!rejected.status.success());
    assert!(!String::from_utf8_lossy(&rejected.stderr).contains("har-secret"));
}
