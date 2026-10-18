use moleapi_core::Workspace;
use moleapi_formats::{export, import};
use serde_json::Value;

const SOURCE: &str = include_str!("fixtures/insomnia_v4.json");

#[test]
fn insomnia_v4_imports_one_workspace_with_folders_requests_and_environments() {
    let imported = import("insomnia", SOURCE).unwrap();
    assert_eq!(imported.name, "Insomnia QA");
    assert_eq!(imported.data.collections.len(), 2);
    let catalog = imported
        .data
        .collections
        .iter()
        .find(|collection| collection.name == "Catalog")
        .unwrap();
    let nested = imported
        .data
        .collections
        .iter()
        .find(|collection| collection.name == "Nested")
        .unwrap();
    assert_eq!(nested.parent_id.as_deref(), Some(catalog.id.as_str()));
    let get = &nested.requests[0];
    assert_eq!(get.name, "List items");
    assert_eq!(get.url, "{{base_url}}/items?tag=a&tag=b");
    assert!(get.query.is_empty());
    assert_eq!(get.auth.kind, "basic");
    assert_eq!(get.auth.username, "ada");
    assert_eq!(get.auth.password, "basic-secret");
    let form = &catalog.requests[0];
    assert_eq!(form.method, "POST");
    assert_eq!(form.body_kind, "form");
    assert_eq!(form.body, "color=red&color=blue");
    assert_eq!(form.auth.kind, "bearer");
    assert_eq!(form.auth.token, "bearer-secret");
    assert_eq!(imported.data.environments.len(), 2);
    assert!(imported.data.environments.iter().any(|env| {
        env.name == "Staging"
            && env
                .variables
                .iter()
                .any(|v| v.key == "base_url" && v.value == "https://staging.example.test")
    }));
    assert!(
        imported
            .data
            .specifications
            .iter()
            .any(|spec| spec.kind == "insomnia" && spec.dialect == "v4" && spec.source == SOURCE)
    );
}

#[test]
fn insomnia_source_and_credentials_survive_only_explicit_full_backup() {
    let imported = import("insomnia", SOURCE).unwrap();
    let workspace: Workspace = serde_json::from_value(serde_json::json!({
        "id":"w","name":imported.name,"revision":1,"updated_at":"now","data":imported.data
    }))
    .unwrap();
    let safe = export(&workspace, "moleapi", false).unwrap();
    assert!(!safe.content.contains("basic-secret"));
    assert!(!safe.content.contains("bearer-secret"));
    assert!(!safe.content.contains("insomnia-secret"));
    let restored = import("moleapi", &safe.content).unwrap();
    assert!(restored.data.specifications[0].source.is_empty());
    let full = export(&workspace, "moleapi", true).unwrap();
    assert_eq!(
        import("moleapi", &full.content)
            .unwrap()
            .data
            .specifications[0]
            .source,
        SOURCE
    );
    for format in ["postman", "openapi", "har"] {
        assert!(export(&workspace, format, true).is_err(), "{format}");
    }
}

#[test]
fn insomnia_form_credentials_are_private_in_default_backup() {
    let mut source: Value = serde_json::from_str(SOURCE).unwrap();
    source["resources"][6]["body"]["params"] = serde_json::json!([
        {"name":"password","value":"form-only-secret"},
        {"name":"note","value":"form-only-secret"},
        {"name":"color","value":"red"}
    ]);
    source["resources"][5]["body"] = serde_json::json!({
        "mimeType":"text/plain","text":"copied form-only-secret"
    });
    source["resources"][5]["authentication"] = Value::Null;
    source["resources"][6]["authentication"] = Value::Null;
    let imported = import("insomnia", &source.to_string()).unwrap();
    let workspace: Workspace = serde_json::from_value(serde_json::json!({
        "id":"w","name":imported.name,"revision":1,"updated_at":"now","data":imported.data
    }))
    .unwrap();
    let safe = export(&workspace, "moleapi", false).unwrap();
    assert!(!safe.content.contains("form-only-secret"));
    let full = export(&workspace, "moleapi", true).unwrap();
    assert!(full.content.contains("form-only-secret"));
}

#[test]
fn insomnia_rejects_disabled_query_still_present_in_url() {
    let mut source: Value = serde_json::from_str(SOURCE).unwrap();
    source["resources"][5]["url"] = "https://example.test/items?token=secret".into();
    source["resources"][5]["parameters"] =
        serde_json::json!([{"name":"token","value":"secret","disabled":true}]);
    assert!(import("insomnia", &source.to_string()).is_err());
}

#[test]
fn insomnia_rejects_unmapped_folder_environment() {
    let mut source: Value = serde_json::from_str(SOURCE).unwrap();
    source["resources"][3]["environment"] =
        serde_json::json!({"base_url":"https://other.example.test"});
    assert!(import("insomnia", &source.to_string()).is_err());
}

#[test]
fn insomnia_private_environment_variables_stay_private() {
    let mut source: Value = serde_json::from_str(SOURCE).unwrap();
    source["resources"][1]["isPrivate"] = true.into();
    source["resources"][1]["data"]["session"] = "private-session-value".into();
    let imported = import("insomnia", &source.to_string()).unwrap();
    let workspace: Workspace = serde_json::from_value(serde_json::json!({
        "id":"w","name":imported.name,"revision":1,"updated_at":"now","data":imported.data
    }))
    .unwrap();
    assert!(
        !export(&workspace, "moleapi", false)
            .unwrap()
            .content
            .contains("private-session-value")
    );
    assert!(
        export(&workspace, "moleapi", true)
            .unwrap()
            .content
            .contains("private-session-value")
    );
}

#[test]
fn insomnia_rejects_invalid_child_environment_url() {
    let mut source: Value = serde_json::from_str(SOURCE).unwrap();
    source["resources"][2]["data"]["base_url"] = "file:///tmp/private".into();
    assert!(import("insomnia", &source.to_string()).is_err());
}

#[test]
fn insomnia_request_settings_preserve_supported_wire_controls() {
    let mut source: Value = serde_json::from_str(SOURCE).unwrap();
    source["resources"][5]["settingFollowRedirects"] = false.into();
    source["resources"][5]["settingSendCookies"] = false.into();
    source["resources"][5]["settingStoreCookies"] = false.into();
    let imported = import("insomnia", &source.to_string()).unwrap();
    let request = &imported
        .data
        .collections
        .iter()
        .flat_map(|c| &c.requests)
        .find(|r| r.name == "List items")
        .unwrap();
    assert!(!request.follow_redirects);
    assert!(!request.use_cookie_jar);

    source["resources"][5]["settingStoreCookies"] = true.into();
    assert!(import("insomnia", &source.to_string()).is_err());
    source["resources"][5]["settingSendCookies"] = true.into();
    source["resources"][5]["settingEncodeUrl"] = false.into();
    assert!(import("insomnia", &source.to_string()).is_err());
    source["resources"][5]["settingEncodeUrl"] = true.into();
    source["resources"][5]["settingRebuildPath"] = false.into();
    assert!(import("insomnia", &source.to_string()).is_err());
}

#[test]
fn insomnia_rejects_conflicting_and_unrepresentable_resources_atomically() {
    let base: Value = serde_json::from_str(SOURCE).unwrap();
    let mut source = base.clone();
    let duplicate = source["resources"][5].clone();
    source["resources"].as_array_mut().unwrap().push(duplicate);
    assert!(import("insomnia", &source.to_string()).is_err());

    source = base.clone();
    source["resources"][3]["parentId"] = Value::String("__folder_2__".into());
    assert!(import("insomnia", &source.to_string()).is_err());

    source = base.clone();
    source["resources"][5]["parentId"] = Value::String("missing".into());
    assert!(import("insomnia", &source.to_string()).is_err());

    source = base.clone();
    source["resources"][5]["parameters"][0]["value"] = Value::String("different".into());
    assert!(import("insomnia", &source.to_string()).is_err());

    source = base.clone();
    source["resources"][5]["authentication"]["type"] = Value::String("oauth2".into());
    assert!(import("insomnia", &source.to_string()).is_err());

    source = base.clone();
    source["resources"][5]["headers"][0]["disabled"] = Value::String("false".into());
    assert!(import("insomnia", &source.to_string()).is_err());

    source = base.clone();
    source["resources"][5]["authentication"]["disabled"] = Value::String("true".into());
    assert!(import("insomnia", &source.to_string()).is_err());

    source = base.clone();
    source["resources"][5]["url"] = Value::String("{{ unknown.token }}/items".into());
    assert!(import("insomnia", &source.to_string()).is_err());

    source = base.clone();
    source["resources"][5]["preRequestScript"] = Value::String("console.log('not-mapped')".into());
    assert!(import("insomnia", &source.to_string()).is_err());

    source = base.clone();
    source["resources"][6]["body"]["fileName"] = Value::String("/private/file".into());
    assert!(import("insomnia", &source.to_string()).is_err());

    source = base.clone();
    source["resources"][7]["cookies"] = serde_json::json!([{"name":"sid","value":"private"}]);
    assert!(import("insomnia", &source.to_string()).is_err());

    source = base.clone();
    source["resources"][8]["contents"] = Value::String("openapi: 3.1.0".into());
    assert!(import("insomnia", &source.to_string()).is_err());

    source = base;
    source["resources"].as_array_mut().unwrap().push(
        serde_json::json!({"_id":"__response_1__","_type":"response","parentId":"__request_1__"}),
    );
    assert!(import("insomnia", &source.to_string()).is_err());
}

#[test]
fn insomnia_drops_connection_nominated_headers_without_changing_other_headers() {
    let mut source: Value = serde_json::from_str(SOURCE).unwrap();
    source["resources"][5]["headers"] = serde_json::json!([
        {"name":"Connection","value":"X-Hop"},
        {"name":"X-Hop","value":"private-hop"},
        {"name":"X-Trace","value":"fixture"}
    ]);
    let imported = import("insomnia", &source.to_string()).unwrap();
    let request = &imported
        .data
        .collections
        .iter()
        .find(|c| c.name == "Nested")
        .unwrap()
        .requests[0];
    assert_eq!(request.headers.len(), 1);
    assert_eq!(request.headers[0].key, "X-Trace");
    assert!(
        imported
            .warnings
            .iter()
            .any(|warning| warning.contains("X-Hop"))
    );
}

#[test]
fn insomnia_caps_sources_and_rejects_unrepresentable_environment_values() {
    assert!(
        import(
            "insomnia",
            &format!("{SOURCE}{}", " ".repeat(5 * 1024 * 1024))
        )
        .is_err()
    );
    let mut source: Value = serde_json::from_str(SOURCE).unwrap();
    source["resources"][1]["data"]["nested"] = serde_json::json!({"key":"value"});
    assert!(import("insomnia", &source.to_string()).is_err());

    source = serde_json::from_str(SOURCE).unwrap();
    let template = source["resources"][5].clone();
    for index in 0..1_000 {
        let mut extra = template.clone();
        extra["_id"] = Value::String(format!("extra-{index}"));
        source["resources"].as_array_mut().unwrap().push(extra);
    }
    assert!(import("insomnia", &source.to_string()).is_err());
}

#[test]
fn insomnia_v4_accepts_an_optional_export_type_marker() {
    let mut source: Value = serde_json::from_str(SOURCE).unwrap();
    source.as_object_mut().unwrap().remove("_type");
    assert_eq!(
        import("insomnia", &source.to_string()).unwrap().name,
        "Insomnia QA"
    );
}

#[test]
fn insomnia_v4_accepts_a_request_only_export_with_an_implicit_workspace() {
    let mut source: Value = serde_json::from_str(SOURCE).unwrap();
    source["resources"].as_array_mut().unwrap().remove(0);
    let imported = import("insomnia", &source.to_string()).unwrap();
    assert_eq!(imported.name, "Insomnia import");
    assert_eq!(
        imported
            .data
            .collections
            .iter()
            .map(|c| c.requests.len())
            .sum::<usize>(),
        2
    );
}

#[test]
fn insomnia_auth_disabled_flag_preserves_on_and_off_wire_semantics() {
    let mut source: Value = serde_json::from_str(SOURCE).unwrap();
    source["resources"][5]["authentication"]["disabled"] = Value::Bool(false);
    let imported = import("insomnia", &source.to_string()).unwrap();
    let nested = imported
        .data
        .collections
        .iter()
        .find(|c| c.name == "Nested")
        .unwrap();
    assert_eq!(nested.requests[0].auth.kind, "basic");
    source["resources"][5]["authentication"]["disabled"] = Value::Bool(true);
    let imported = import("insomnia", &source.to_string()).unwrap();
    let nested = imported
        .data
        .collections
        .iter()
        .find(|c| c.name == "Nested")
        .unwrap();
    assert_eq!(nested.requests[0].auth.kind, "none");
}
