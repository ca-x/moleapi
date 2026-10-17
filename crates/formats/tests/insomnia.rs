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
    source["resources"][5]["authentication"]["disabled"] = Value::Bool(true);
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
