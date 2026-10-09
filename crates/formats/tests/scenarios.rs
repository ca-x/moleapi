use serde_json::json;
#[test]
fn scenario_metadata_is_screened_but_explicit_backup_is_exact() {
    let mut workspace:moleapi_core::Workspace=serde_json::from_value(json!({"id":"w","name":"Flow","revision":1,"updated_at":"now","data":{"schema_version":1,"global_variables":[{"id":"private","key":"secret","value":"private-scenario-value","secret":true,"enabled":true}],"collections":[{"id":"c","name":"C","description":"","requests":[{"id":"r","name":"R","method":"GET","url":"https://example.test","description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":true,"verify_tls":true,"assertions":[],"examples":[]}]}],"environments":[],"active_environment_id":null,"scenarios":[{"id":"s","name":"private-scenario-value","description":"private-scenario-value","collection_id":"c","steps":[{"id":"a","request_id":"r","name":"private-scenario-value","group":"private-scenario-value"}]}]}})).unwrap();
    workspace.data.scenarios[0].steps[0].condition =
        Some("pm.variables.get('token') === 'private-scenario-value'".into());
    workspace.data.scenarios[0].steps[0].repeat = 3;
    workspace.data.scenarios[0]
        .steps
        .push(moleapi_core::ScenarioStep {
            id: "b".into(),
            request_id: "r".into(),
            name: "Safe".into(),
            group: String::new(),
            enabled: true,
            condition: None,
            repeat: 1,
            on_true: Some(moleapi_core::ScenarioTarget::Step {
                step_id: "a".into(),
            }),
            on_false: None,
        });
    let safe = moleapi_formats::export(&workspace, "moleapi", false).unwrap();
    assert!(!safe.content.contains("private-scenario-value"));
    let imported = moleapi_formats::import("moleapi", &safe.content).unwrap();
    moleapi_core::validate_workspace(&imported.data).unwrap();
    assert_eq!(imported.data.scenarios[0].steps[0].request_id, "r");
    assert!(!imported.data.scenarios[0].steps[0].enabled);
    assert!(imported.data.scenarios[0].steps[0].condition.is_none());
    assert_eq!(
        imported.data.scenarios[0].steps[1].on_true,
        Some(moleapi_core::ScenarioTarget::Stop)
    );
    let backup = moleapi_formats::export(&workspace, "moleapi", true).unwrap();
    let imported = moleapi_formats::import("moleapi", &backup.content).unwrap();
    assert_eq!(imported.data.scenarios, workspace.data.scenarios);
    let scenario = &mut workspace.data.scenarios[0];
    scenario.steps[1].on_true = None;
    let mut third = scenario.steps[1].clone();
    third.id = "c".into();
    scenario.steps.push(third);
    scenario.parallel = vec![moleapi_core::ScenarioParallel {
        id: "block".into(),
        name: "private-scenario-value".into(),
        step_ids: vec!["a".into(), "b".into(), "c".into()],
        concurrency: 2,
    }];
    let safe = moleapi_formats::export(&workspace, "moleapi", false).unwrap();
    assert!(!safe.content.contains("private-scenario-value"));
    let imported = moleapi_formats::import("moleapi", &safe.content).unwrap();
    moleapi_core::validate_workspace(&imported.data).unwrap();
    assert_eq!(
        imported.data.scenarios[0].parallel[0].step_ids,
        vec!["b", "c"]
    );
    let backup = moleapi_formats::export(&workspace, "moleapi", true).unwrap();
    let imported = moleapi_formats::import("moleapi", &backup.content).unwrap();
    assert_eq!(imported.data.scenarios, workspace.data.scenarios);
}
