use moleapi_core::{Environment, Pair, Workspace, WorkspaceData};
use moleapi_formats::{export_postman_variables, import_postman_variables};
use serde_json::{Value, json};

fn workspace(data: WorkspaceData) -> Workspace {
    Workspace {
        id: "owned".into(),
        name: "API".into(),
        revision: 1,
        updated_at: "2026-10-17T00:00:00Z".into(),
        data,
    }
}

#[test]
fn postman_environment_and_globals_keep_order_flags_secret_markers_and_exact_text() {
    let environment =
        import_postman_variables(include_str!("fixtures/postman_environment.json")).unwrap();
    assert_eq!(environment.scope, "environment");
    assert_eq!(environment.name, "Development");
    assert_eq!(
        environment
            .variables
            .iter()
            .map(|item| item.key.as_str())
            .collect::<Vec<_>>(),
        ["base_url", "api_token", "disabled"]
    );
    assert_eq!(environment.variables[1].secret, Some(true));
    assert!(!environment.variables[2].enabled);
    assert!(
        environment
            .variables
            .iter()
            .all(|item| item.local_value.is_none())
    );
    let globals = import_postman_variables(include_str!("fixtures/postman_globals.json")).unwrap();
    assert_eq!(globals.scope, "globals");
    assert_eq!(globals.variables[0].value, "东京");
    assert_eq!(globals.variables[1].secret, Some(true));
}

#[test]
fn postman_variable_export_defaults_private_and_full_export_restores_exact_values() {
    let environment =
        import_postman_variables(include_str!("fixtures/postman_environment.json")).unwrap();
    let mut data: WorkspaceData = serde_json::from_value(
        json!({"schema_version":1,"collections":[],"environments":[],"active_environment_id":null}),
    )
    .unwrap();
    data.environments.push(Environment {
        id: "dev".into(),
        name: environment.name,
        variables: environment.variables,
    });
    data.active_environment_id = Some("dev".into());
    data.global_variables = import_postman_variables(include_str!("fixtures/postman_globals.json"))
        .unwrap()
        .variables;
    data.global_variables.push(Pair {
        id: "copied".into(),
        key: "copied_value".into(),
        value: "environment-secret".into(),
        enabled: true,
        secret: Some(false),
        local_value: None,
    });
    let workspace = workspace(data);
    let safe = export_postman_variables(&workspace, "environment", Some("dev"), false).unwrap();
    assert!(!safe.content.contains("environment-secret"));
    let safe_source: Value = serde_json::from_str(&safe.content).unwrap();
    assert_eq!(safe_source["_postman_variable_scope"], "environment");
    assert_eq!(safe_source["values"][1]["type"], "secret");
    assert_eq!(safe_source["values"][1]["value"], "");
    assert_eq!(safe_source["values"][2]["enabled"], false);
    let full = export_postman_variables(&workspace, "environment", Some("dev"), true).unwrap();
    assert!(full.content.contains("environment-secret"));
    let imported = import_postman_variables(&full.content).unwrap();
    assert_eq!(
        imported
            .variables
            .iter()
            .map(|item| (&item.key, &item.value, item.enabled, item.secret))
            .collect::<Vec<_>>(),
        workspace.data.environments[0]
            .variables
            .iter()
            .map(|item| (&item.key, &item.value, item.enabled, item.secret))
            .collect::<Vec<_>>()
    );
    let globals = export_postman_variables(&workspace, "globals", None, false).unwrap();
    let globals_source: Value = serde_json::from_str(&globals.content).unwrap();
    assert_eq!(globals_source["_postman_variable_scope"], "globals");
    assert_ne!(globals_source["values"][2]["value"], "environment-secret");
    assert!(!globals.content.contains("environment-secret"));
    assert!(export_postman_variables(&workspace, "environment", Some("missing"), false).is_err());
}

#[test]
fn postman_variable_files_reject_ambiguous_or_oversized_input() {
    let mut source: Value =
        serde_json::from_str(include_str!("fixtures/postman_environment.json")).unwrap();
    source["_postman_variable_scope"] = json!("team");
    assert!(import_postman_variables(&source.to_string()).is_err());
    source = serde_json::from_str(include_str!("fixtures/postman_environment.json")).unwrap();
    source["values"][1]["key"] = json!("base_url");
    assert!(import_postman_variables(&source.to_string()).is_err());
    source = serde_json::from_str(include_str!("fixtures/postman_environment.json")).unwrap();
    source["values"][1]["value"] = json!({"nested":"not a string"});
    assert!(import_postman_variables(&source.to_string()).is_err());
    source = serde_json::from_str(include_str!("fixtures/postman_environment.json")).unwrap();
    source["values"] = Value::Array(vec![json!({"key":"row","value":"x"}); 1001]);
    assert!(import_postman_variables(&source.to_string()).is_err());
    source = serde_json::from_str(include_str!("fixtures/postman_environment.json")).unwrap();
    source["values"][0]["description"] = json!("Postman-only metadata");
    let imported = import_postman_variables(&source.to_string()).unwrap();
    assert_eq!(imported.warnings, ["postman_variable_row_metadata_omitted"]);
    assert!(import_postman_variables(&"x".repeat(1024 * 1024 + 1)).is_err());
}
