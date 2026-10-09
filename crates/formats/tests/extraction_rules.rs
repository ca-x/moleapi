use moleapi_core::Workspace;
use serde_json::json;
#[test]
fn extraction_selectors_with_private_copies_are_disabled_in_default_backup() {
    let workspace:Workspace=serde_json::from_value(json!({"id":"w","name":"Rules","revision":1,"updated_at":"now","data":{"schema_version":1,"global_variables":[{"id":"secret","key":"seed","value":"private-selector-token","secret":true,"enabled":true}],"collections":[{"id":"c","name":"Collection","description":"","requests":[{"id":"r","name":"Request","method":"GET","url":"https://example.test","description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":true,"verify_tls":true,"assertions":[],"examples":[],"extractions":[{"id":"extract","name":"Value","kind":"regex","target":"token=(private-selector-token)","scope":"temporary","key":"value","required":true}]}]}],"environments":[],"active_environment_id":null}})).unwrap();
    let safe = moleapi_formats::export(&workspace, "moleapi", false).unwrap();
    assert!(!safe.content.contains("private-selector-token"));
    let restored = moleapi_formats::import("moleapi", &safe.content).unwrap();
    assert!(!restored.data.collections[0].requests[0].extractions[0].enabled);
    let backup = moleapi_formats::export(&workspace, "moleapi", true).unwrap();
    let restored = moleapi_formats::import("moleapi", &backup.content).unwrap();
    assert_eq!(
        restored.data.collections[0].requests[0].extractions,
        workspace.data.collections[0].requests[0].extractions
    );
}
