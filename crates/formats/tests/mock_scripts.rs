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

#[test]
fn mock_simulation_settings_validate_and_roundtrip_only_in_native_backup() {
    let mut source = json!({"id":"w","name":"Fault","revision":1,"updated_at":"now","data":{"schema_version":1,"collections":[{"id":"c","name":"C","description":"","requests":[{"id":"r","name":"R","method":"GET","url":"https://example.test/","description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":true,"verify_tls":true,"assertions":[],"examples":[{"id":"e","name":"Fault","status":200,"headers":[],"body":"ok"}]}]}],"environments":[],"active_environment_id":null}});
    let old: Workspace = serde_json::from_value(source.clone()).unwrap();
    let old_example = &old.data.collections[0].requests[0].examples[0];
    assert_eq!(old_example.fault_rate_percent, 0);
    assert_eq!(old_example.fault_status, 503);
    assert_eq!(old_example.rate_limit_per_minute, 0);
    let serialized = serde_json::to_value(old_example).unwrap();
    assert!(serialized.get("fault_rate_percent").is_none());
    assert!(serialized.get("fault_status").is_none());
    assert!(serialized.get("rate_limit_per_minute").is_none());

    let example = &mut source["data"]["collections"][0]["requests"][0]["examples"][0];
    example["fault_rate_percent"] = json!(100);
    example["fault_status"] = json!(502);
    example["rate_limit_per_minute"] = json!(3);
    let workspace: Workspace = serde_json::from_value(source.clone()).unwrap();
    validate_workspace(&workspace.data).unwrap();
    for private in [false, true] {
        let exported = export(&workspace, "moleapi", private).unwrap();
        let imported = import("moleapi", &exported.content).unwrap();
        let restored = &imported.data.collections[0].requests[0].examples[0];
        assert_eq!(
            (restored.fault_rate_percent, restored.fault_status),
            (100, 502)
        );
        assert_eq!(restored.rate_limit_per_minute, 3);
    }
    for format in ["postman", "openapi", "har"] {
        assert!(export(&workspace, format, true).is_err(), "{format}");
    }
    for (rate, status) in [(101, 503), (1, 499), (1, 600), (0, 499)] {
        source["data"]["collections"][0]["requests"][0]["examples"][0]["fault_rate_percent"] =
            json!(rate);
        source["data"]["collections"][0]["requests"][0]["examples"][0]["fault_status"] =
            json!(status);
        let workspace: Workspace = serde_json::from_value(source.clone()).unwrap();
        assert!(validate_workspace(&workspace.data).is_err());
    }
    source["data"]["collections"][0]["requests"][0]["examples"][0]["fault_rate_percent"] = json!(0);
    source["data"]["collections"][0]["requests"][0]["examples"][0]["fault_status"] = json!(503);
    source["data"]["collections"][0]["requests"][0]["examples"][0]["rate_limit_per_minute"] =
        json!(1001);
    let workspace: Workspace = serde_json::from_value(source).unwrap();
    assert!(validate_workspace(&workspace.data).is_err());
}

#[test]
fn mock_schema_is_private_in_default_backup_and_exact_in_full_backup() {
    let schema = r#"{"type":"string","default":"private-schema-example"}"#;
    let mut source = json!({"id":"w","name":"Schema","revision":1,"updated_at":"now","data":{"schema_version":1,"collections":[{"id":"c","name":"C","description":"","requests":[{"id":"r","name":"R","method":"GET","url":"https://example.test/","description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":true,"verify_tls":true,"assertions":[],"examples":[{"id":"e","name":"Schema","status":200,"headers":[],"body":"static"}]}]}],"environments":[],"active_environment_id":null}});
    source["data"]["collections"][0]["requests"][0]["examples"][0]["mock_schema"] = json!(schema);
    source["data"]["collections"][0]["requests"][0]["examples"][0]["mock_name_inference"] =
        json!(true);
    let workspace: Workspace = serde_json::from_value(source).unwrap();
    validate_workspace(&workspace.data).unwrap();
    let mut missing_source = workspace.data.clone();
    missing_source.collections[0].requests[0].examples[0]
        .mock_schema
        .clear();
    assert!(validate_workspace(&missing_source).is_err());
    let safe = export(&workspace, "moleapi", false).unwrap();
    assert!(!safe.content.contains("private-schema-example"));
    let safe_example =
        &import("moleapi", &safe.content).unwrap().data.collections[0].requests[0].examples[0];
    assert!(safe_example.mock_schema.is_empty());
    assert!(!safe_example.mock_name_inference);
    let full = export(&workspace, "moleapi", true).unwrap();
    assert_eq!(
        import("moleapi", &full.content).unwrap().data.collections[0].requests[0].examples[0]
            .mock_schema,
        schema
    );
    assert!(
        import("moleapi", &full.content).unwrap().data.collections[0].requests[0].examples[0]
            .mock_name_inference
    );
    for format in ["postman", "openapi", "har"] {
        assert!(export(&workspace, format, true).is_err(), "{format}");
    }
}

#[test]
fn linked_mock_schema_and_private_spec_source_survive_only_full_backup() {
    let specification = json!({"openapi":"3.1.0","paths":{"/items":{"get":{"responses":{"200":{"content":{"application/json":{"schema":{"type":"string","const":"private-linked-example"}}}}}}}}}).to_string();
    let source = json!({"id":"w","name":"Linked","revision":1,"updated_at":"now","data":{"schema_version":1,"specifications":[{"id":"spec","name":"Spec","kind":"openapi","dialect":"3.1.0","source":specification}],"collections":[{"id":"c","name":"C","description":"","requests":[{"id":"r","name":"R","method":"GET","url":"https://example.test/items","specification_id":"spec","description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":true,"verify_tls":true,"assertions":[],"examples":[{"id":"e","name":"Linked","status":200,"headers":[],"body":"static","mock_schema_link":{"specification_id":"spec","pointer":"#/paths/~1items/get/responses/200/content/application~1json/schema"}}]}]}],"environments":[],"active_environment_id":null}});
    let workspace: Workspace = serde_json::from_value(source).unwrap();
    validate_workspace(&workspace.data).unwrap();
    let safe = export(&workspace, "moleapi", false).unwrap();
    assert!(!safe.content.contains("private-linked-example"));
    let restored = import("moleapi", &safe.content).unwrap();
    assert!(
        restored.data.collections[0].requests[0].examples[0]
            .mock_schema_link
            .is_none()
    );
    let full = export(&workspace, "moleapi", true).unwrap();
    let restored = import("moleapi", &full.content).unwrap();
    assert_eq!(
        restored.data.collections[0].requests[0].examples[0]
            .mock_schema_link
            .as_ref()
            .unwrap()
            .specification_id,
        "spec"
    );
    assert!(
        restored.data.specifications[0]
            .source
            .contains("private-linked-example")
    );
    for format in ["postman", "openapi", "har"] {
        assert!(export(&workspace, format, true).is_err(), "{format}");
    }
}
