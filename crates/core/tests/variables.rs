use moleapi_core::*;
use serde_json::json;
fn pair(key: &str, value: &str) -> Pair {
    serde_json::from_value(json!({"id":key,"key":key,"value":value,"enabled":true})).unwrap()
}
#[test]
fn every_scope_has_correct_precedence_and_native_override_is_not_cloud_value() {
    let workspace: WorkspaceData = serde_json::from_value(
        json!({"schema_version":1,"collections":[],"environments":[],"active_environment_id":null}),
    )
    .unwrap();
    let mut workspace = workspace;
    workspace.global_variables = vec![pair("key", "project")];
    let collection: Collection = serde_json::from_value(json!({"id":"c","name":"C","description":"","requests":[],"variables":[pair("key","collection")]})).unwrap();
    let mut environment = Environment {
        id: "e".into(),
        name: "E".into(),
        variables: vec![pair("key", "environment")],
    };
    let data = [pair("key", "data")];
    let temporary = [pair("key", "temporary")];
    for (collection, environment, data, temporary, expected) in [
        (None, None, &[][..], &[][..], "project"),
        (Some(&collection), None, &[][..], &[][..], "collection"),
        (
            Some(&collection),
            Some(&environment),
            &[][..],
            &[][..],
            "environment",
        ),
        (
            Some(&collection),
            Some(&environment),
            &data[..],
            &[][..],
            "data",
        ),
        (
            Some(&collection),
            Some(&environment),
            &data[..],
            &temporary[..],
            "temporary",
        ),
    ] {
        let scopes =
            VariableScopes::new(&workspace, collection, environment, data, temporary, true)
                .unwrap();
        assert_eq!(scopes.effective().variables[0].value, expected);
    }
    environment.variables[0].local_value = Some("native".into());
    assert_eq!(
        VariableScopes::new(&workspace, None, Some(&environment), &[], &[], true)
            .unwrap()
            .effective()
            .variables[0]
            .value,
        "native"
    );
    assert_eq!(
        VariableScopes::new(&workspace, None, Some(&environment), &[], &[], false)
            .unwrap()
            .effective()
            .variables[0]
            .value,
        "environment"
    );
}
#[test]
fn variable_validation_bounds_and_atomic_updates() {
    let mut scopes = VariableScopes::default();
    let updates = [VariableUpdate {
        scope: "temporary".into(),
        key: "key".into(),
        value: Some("x".repeat(MAX_VARIABLE_BYTES + 1)),
    }];
    assert!(scopes.apply(&updates).is_err());
    assert!(scopes.temporary.is_empty());
    assert!(
        scopes
            .apply_locals(&[VariableUpdate {
                scope: "environment".into(),
                key: String::new(),
                value: None
            }])
            .is_err()
    );
    assert!(validate_variables(&[pair("dup", "a"), pair("dup", "b")]).is_err());
    assert!(
        scopes
            .apply(&[VariableUpdate {
                scope: "data".into(),
                key: "key".into(),
                value: Some("x".into())
            }])
            .is_err()
    );
}
