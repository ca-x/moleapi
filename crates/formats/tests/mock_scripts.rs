#![recursion_limit = "256"]
use moleapi_core::{Workspace, validate_workspace};
use moleapi_formats::{export, import};
use serde_json::json;

#[test]
fn default_backup_withholds_mock_script_and_full_backup_restores_it() {
    let workspace: Workspace = serde_json::from_value(json!({"id":"w","name":"Mock","revision":1,"updated_at":"now","data":{"schema_version":1,"script_mode":"v2","collections":[{"id":"c","name":"C","description":"","requests":[{"id":"r","name":"R","method":"GET","url":"https://example.test/","description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":true,"verify_tls":true,"assertions":[],"examples":[{"id":"e","name":"Dynamic","status":200,"headers":[],"body":"static","mock_script":"pm.mock.setBody('private-mock-literal')"}]}]}],"environments":[],"active_environment_id":null}})).unwrap();
    validate_workspace(&workspace.data).unwrap();
    let safe = import(
        "moleapi",
        &export(&workspace, "moleapi", false).unwrap().content,
    )
    .unwrap();
    assert!(
        safe.data.collections[0].requests[0].examples[0]
            .mock_script
            .is_empty()
    );
    let full = import(
        "moleapi",
        &export(&workspace, "moleapi", true).unwrap().content,
    )
    .unwrap();
    assert_eq!(
        full.data.collections[0].requests[0].examples[0].mock_script,
        "pm.mock.setBody('private-mock-literal')"
    );
    assert!(export(&workspace, "postman", true).is_err());
}

#[test]
fn mock_match_private_criteria_are_withheld_and_external_exports_reject() {
    let workspace: Workspace = serde_json::from_value(json!({"id":"w","name":"Rules","revision":1,"updated_at":"now","data":{"schema_version":1,"collections":[{"id":"c","name":"C","description":"","requests":[{"id":"r","name":"R","method":"GET","url":"https://example.test/","description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":true,"verify_tls":true,"assertions":[],"examples":[{"id":"e","name":"Matched","status":200,"headers":[],"body":"ok","mock_match":{"priority":2,"headers":[{"id":"h","key":"X-Selector","value":"private-selector","enabled":true,"secret":true}]}}]}]}],"environments":[],"active_environment_id":null}})).unwrap();
    validate_workspace(&workspace.data).unwrap();
    let safe = export(&workspace, "moleapi", false).unwrap();
    assert!(!safe.content.contains("private-selector"));
    let safe_data = import("moleapi", &safe.content).unwrap();
    assert!(
        safe_data.data.collections[0].requests[0].examples[0]
            .mock_match
            .is_none()
    );
    let full = export(&workspace, "moleapi", true).unwrap();
    assert!(full.content.contains("private-selector"));
    assert!(export(&workspace, "postman", true).is_err());
    assert!(export(&workspace, "openapi", true).is_err());
}

#[test]
fn mock_delay_survives_native_backup_but_external_formats_reject_it() {
    let workspace: Workspace = serde_json::from_value(json!({"id":"w","name":"Delay","revision":1,"updated_at":"now","data":{"schema_version":1,"collections":[{"id":"c","name":"C","description":"","requests":[{"id":"r","name":"R","method":"GET","url":"https://example.test/","description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":true,"verify_tls":true,"assertions":[],"examples":[{"id":"e","name":"Delayed","status":200,"headers":[],"body":"ok","delay_ms":120}]}]}],"environments":[],"active_environment_id":null}})).unwrap();
    validate_workspace(&workspace.data).unwrap();
    for private in [false, true] {
        let exported = export(&workspace, "moleapi", private).unwrap();
        let imported = import("moleapi", &exported.content).unwrap();
        assert_eq!(
            imported.data.collections[0].requests[0].examples[0].delay_ms,
            120
        );
    }
    assert!(export(&workspace, "postman", true).is_err());
    assert!(export(&workspace, "openapi", true).is_err());
}
