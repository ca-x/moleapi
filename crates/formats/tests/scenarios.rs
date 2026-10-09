use serde_json::json;
#[test]
fn scenario_metadata_is_screened_but_explicit_backup_is_exact() {
    let workspace:moleapi_core::Workspace=serde_json::from_value(json!({"id":"w","name":"Flow","revision":1,"updated_at":"now","data":{"schema_version":1,"global_variables":[{"id":"private","key":"secret","value":"private-scenario-value","secret":true,"enabled":true}],"collections":[{"id":"c","name":"C","description":"","requests":[{"id":"r","name":"R","method":"GET","url":"https://example.test","description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":true,"verify_tls":true,"assertions":[],"examples":[]}]}],"environments":[],"active_environment_id":null,"scenarios":[{"id":"s","name":"private-scenario-value","description":"private-scenario-value","collection_id":"c","steps":[{"id":"a","request_id":"r","name":"private-scenario-value","group":"private-scenario-value"}]}]}})).unwrap();
    let safe = moleapi_formats::export(&workspace, "moleapi", false).unwrap();
    assert!(!safe.content.contains("private-scenario-value"));
    let imported = moleapi_formats::import("moleapi", &safe.content).unwrap();
    moleapi_core::validate_workspace(&imported.data).unwrap();
    assert_eq!(imported.data.scenarios[0].steps[0].request_id, "r");
    let backup = moleapi_formats::export(&workspace, "moleapi", true).unwrap();
    let imported = moleapi_formats::import("moleapi", &backup.content).unwrap();
    assert_eq!(imported.data.scenarios, workspace.data.scenarios);
}
