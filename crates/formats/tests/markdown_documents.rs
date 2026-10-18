use moleapi_core::{Workspace, validate_workspace};
use moleapi_formats::{export, import};
use serde_json::json;

fn workspace() -> Workspace {
    serde_json::from_value(json!({
        "id":"w","name":"Docs","revision":1,"updated_at":"now",
        "data":{
            "schema_version":1,"collections":[],"environments":[],"active_environment_id":null,
            "global_variables":[{"id":"v","key":"api_key","value":"copied-doc-secret","enabled":true,"secret":true}],
            "documents":[
                {"id":"private","name":"Private","source":"# Private\ncopied-doc-secret"},
                {"id":"shared","name":"Guide copied-doc-secret","source":"# Guide\ncopied-doc-secret and ordinary text","secret":false}
            ]
        }
    })).unwrap()
}

#[test]
fn markdown_documents_keep_exact_full_backup_and_screen_default_backup() {
    let source = workspace();
    validate_workspace(&source.data).unwrap();
    let safe = export(&source, "moleapi", false).unwrap();
    assert!(!safe.content.contains("copied-doc-secret"));
    let restored = import("moleapi", &safe.content).unwrap();
    assert!(restored.data.documents[0].source.is_empty());
    assert!(restored.data.documents[0].secret);
    assert!(restored.data.documents[1].source.contains("ordinary text"));
    assert!(
        !restored.data.documents[1]
            .source
            .contains("copied-doc-secret")
    );
    let full = export(&source, "moleapi", true).unwrap();
    let exact = import("moleapi", &full.content).unwrap();
    assert_eq!(exact.data.documents, source.data.documents);
}

#[test]
fn markdown_documents_reject_external_formats_until_they_can_preserve_them() {
    let mut source = workspace();
    source.data.global_variables.clear();
    for format in ["postman", "openapi", "har"] {
        let error = export(&source, format, true).err().unwrap().to_string();
        assert!(error.contains("Markdown"), "{format}: {error}");
    }
}

#[test]
fn public_markdown_source_is_withheld_when_privacy_patterns_exceed_budget() {
    let mut source = workspace();
    source.data.global_variables[0].value = "x".repeat(7_000);
    source.data.documents[1].source = "# Ordinary guide".into();
    validate_workspace(&source.data).unwrap();
    let safe = export(&source, "moleapi", false).unwrap();
    let restored = import("moleapi", &safe.content).unwrap();
    assert!(restored.data.documents[1].source.is_empty());
    assert_eq!(restored.data.documents[1].name, "[REDACTED]");
}
