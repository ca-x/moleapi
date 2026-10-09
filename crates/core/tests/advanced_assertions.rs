use moleapi_core::*;
use serde_json::json;
fn response(body: &str) -> Response {
    serde_json::from_value(json!({"status":200,"status_text":"OK","headers":[{"id":"h","key":"X-Test","value":"yes","enabled":true}],"body":body,"elapsed_ms":10,"size_bytes":body.len(),"truncated":false,"url":"https://example.test","tests":[]})).unwrap()
}
fn check(kind: &str, target: &str, expected: &str) -> Assertion {
    Assertion {
        id: "a".into(),
        name: kind.into(),
        kind: kind.into(),
        target: target.into(),
        expected: expected.into(),
    }
}
#[test]
fn mature_queries_schema_and_regex_return_structured_checks() {
    let response = response(r#"{"items":[{"id":3},{"id":7}],"value":"id=42"}"#);
    let checks = [
        check("jsonpath", "$.items[*].id", "[3,7]"),
        check("jsonpath", "$.items[0].id", "3"),
        check("regex", "id=(\\d+)", "42"),
        check("header", "x-test", "yes"),
        check(
            "schema",
            "",
            r#"{"type":"object","required":["items"],"properties":{"items":{"type":"array","items":{"type":"object","required":["id"],"properties":{"id":{"type":"integer"}}}}}}"#,
        ),
    ];
    let result = assertions(&checks, &response);
    assert!(result.iter().all(|result| result.passed), "{result:?}");
}
#[test]
fn xpath_root_namespaces_and_scalar_types_use_mature_evaluator() {
    let response = response(r#"<s:Envelope xmlns:s="urn:soap"><s:id>42</s:id></s:Envelope>"#);
    let checks = [
        check("xpath", "string(/s:Envelope/s:id)", "42"),
        check("xpath", "count(//s:id)", "1"),
        check("xpath", "boolean(//s:id)", "true"),
    ];
    assert!(
        assertions(&checks, &response)
            .iter()
            .all(|check| check.passed)
    );
}
#[test]
fn invalid_queries_schema_fetches_and_xml_entities_fail_explicitly() {
    let json = response(r#"{"value":"abc"}"#);
    for check in [
        check("schema", "", r#"{"$ref":"file:///etc/passwd"}"#),
        check("regex", "[", ""),
        check("jsonpath", "$[", "null"),
    ] {
        assert!(!assertions(&[check], &json)[0].passed);
    }
    let xml = response("<!DOCTYPE a [<!ENTITY secret 'private'>]><a>&secret;</a>");
    assert!(!assertions(&[check("xpath", "string(/a)", "private")], &xml)[0].passed);
}
#[test]
fn advanced_regex_features_formats_and_missing_xpath_are_not_silently_downgraded() {
    let json = response(r#"{"value":"id=42","email":"not-an-email"}"#);
    let checks = [
        check("regex", "(?<=id=)\\d+", "42"),
        check(
            "schema",
            "/value",
            r#"{"type":"string","pattern":"(?<=id=)\\d+$"}"#,
        ),
        check("schema", "/email", r#"{"type":"string","format":"email"}"#),
    ];
    let result = assertions(&checks, &json);
    assert!(
        result[0].passed && result[1].passed && !result[2].passed,
        "{result:?}"
    );
    let xml = response("<root/>");
    assert!(!assertions(&[check("xpath", "/root/missing", "")], &xml)[0].passed);
}
#[test]
fn long_details_are_withheld_whole_instead_of_partial_private_fragments() {
    let value = "private-detail-".repeat(500);
    let body = json!({"value":value}).to_string();
    let result = assertions(
        &[check(
            "jsonpath",
            "$.value",
            &serde_json::to_string(&value).unwrap(),
        )],
        &response(&body),
    );
    assert!(result[0].passed);
    assert!(!result[0].actual.contains("private-detail"));
    assert!(!result[0].expected.contains("private-detail"));
    assert!(result[0].actual.contains("omitted"));
}
