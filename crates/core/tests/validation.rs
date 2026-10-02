use moleapi_core::*;
use serde_json::json;
fn request() -> RequestSpec {
    serde_json::from_value(json!({"id":"r","name":"request","method":"GET","url":"https://example.com","description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":true,"verify_tls":true,"assertions":[],"examples":[]})).unwrap()
}
fn variable(value: &str) -> Environment {
    Environment {
        id: "e".into(),
        name: "Env".into(),
        variables: vec![Pair {
            id: "v".into(),
            key: "method".into(),
            value: value.into(),
            enabled: true,
            secret: None,
        }],
    }
}
#[test]
fn method_templates_are_validated_after_resolution_and_nested_templates_fail() {
    let mut r = request();
    r.method = "{{method}}".into();
    assert!(validate_request(&r, true).is_ok());
    assert_eq!(
        resolve_request(&r, Some(&variable("POST"))).unwrap().method,
        "POST"
    );
    assert!(resolve_request(&r, Some(&variable("TRACE"))).is_err());
    assert!(resolve_request(&r, Some(&variable("{{unknown}}"))).is_err());
}
#[test]
fn rfc6901_pointer_escapes_are_validated() {
    let mut r = request();
    r.assertions.push(Assertion {
        id: "a".into(),
        name: "JSON".into(),
        kind: "json".into(),
        target: "/a~1b/~0".into(),
        expected: "null".into(),
    });
    assert!(validate_request(&r, false).is_ok());
    r.assertions[0].target = "/bad~2escape".into();
    assert!(validate_request(&r, false).is_err());
    r.assertions[0].target = "/trailing~".into();
    assert!(validate_request(&r, false).is_err());
}
#[test]
fn compatibility_defaults_preserve_old_workspace_and_request_documents() {
    let w: WorkspaceData = serde_json::from_value(
        json!({"schema_version":1,"collections":[],"environments":[],"active_environment_id":null}),
    )
    .unwrap();
    assert!(w.specifications.is_empty());
    let r = request();
    assert!(r.specification_id.is_none());
    assert!(r.operation_id.is_none());
    assert!(validate_workspace(&w).is_ok());
}
#[test]
fn canonical_specification_references_and_size_are_checked() {
    let mut r = request();
    r.specification_id = Some("spec".into());
    let mut w = WorkspaceData {
        specifications: vec![],
        schema_version: 1,
        collections: vec![Collection {
            id: "c".into(),
            name: "C".into(),
            description: "".into(),
            requests: vec![r],
        }],
        environments: vec![],
        active_environment_id: None,
    };
    assert!(validate_workspace(&w).is_err());
    w.specifications.push(Specification {
        id: "spec".into(),
        name: "Spec".into(),
        kind: "openapi".into(),
        source: "original schema".into(),
        dialect: "3.1".into(),
    });
    assert!(validate_workspace(&w).is_ok());
    w.specifications[0].source = "x".repeat(MAX_BODY + 1);
    assert!(validate_workspace(&w).is_err());
}

#[test]
fn aggregate_expansion_budget_includes_form_encoding_growth() {
    let huge = "x".repeat(4_500_000);
    let value = serde_json::json!({"id":"r","name":"budget","method":"POST","url":"https://example.com","description":"","query":[],"headers":[],"body_kind":"form","body":"field={{value}}","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":false,"verify_tls":true,"assertions":[],"examples":(0..4).map(|i|serde_json::json!({"id":format!("e{i}"),"name":"example","status":200,"headers":[],"body":huge})).collect::<Vec<_>>()});
    let request: moleapi_core::RequestSpec = serde_json::from_value(value).unwrap();
    let environment:moleapi_core::Environment=serde_json::from_value(serde_json::json!({"id":"env","name":"env","variables":[{"id":"value","key":"value","value":"&".repeat(1_000_000),"enabled":true}]})).unwrap();
    assert!(
        moleapi_core::resolve_request(&request, Some(&environment)).is_err(),
        "encoded form growth must count toward the aggregate budget"
    );
}
