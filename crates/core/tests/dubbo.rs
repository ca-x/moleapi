use moleapi_core::{DubboConfig, Environment, Protocol, RequestSpec, validate_request};
use serde_json::{Value, json};

fn request() -> RequestSpec {
    serde_json::from_value(json!({
        "id":"dubbo", "name":"Classic", "method":"GET", "url":"dubbo://example.com:20880",
        "description":"", "query":[], "headers":[], "body_kind":"none", "body":"",
        "auth":{"kind":"none","token":"","username":"","password":""},
        "timeout_ms":5000,"follow_redirects":false,"verify_tls":true,
        "assertions":[],"examples":[],
        "protocol":{"kind":"dubbo","source_version":1,"transport":"classic","serialization":"hessian2",
          "service":"fixture.EchoService","method":"echo","version":"1.0.0","group":"",
          "arguments":[{"id":"first","type_name":"java.lang.String","value_source":"\"鼹鼠 {{name}}\"","secret":true}],
          "attachments":[{"id":"trace","key":"trace-id","value":"{{trace}}","enabled":true,"secret":true}]}
    })).unwrap()
}

#[test]
fn classic_draft_roundtrips_order_and_original_json_without_becoming_http() {
    let r = request();
    validate_request(&r, true).unwrap();
    let original = serde_json::to_value(&r).unwrap();
    let restored: RequestSpec = serde_json::from_value(original.clone()).unwrap();
    assert_eq!(serde_json::to_value(restored).unwrap(), original);
    let Protocol::Dubbo { config } = r.protocol else {
        panic!("not Dubbo")
    };
    assert_eq!(config.arguments[0].value_source, "\"鼹鼠 {{name}}\"");
    assert_eq!(config.attachments[0].key, "trace-id");
    assert_eq!(DubboConfig::default().transport, "classic");
}

#[test]
fn classic_validates_endpoint_kind_type_limits_and_reserved_attachments() {
    let mut r = request();
    r.url = "https://example.com:20880".into();
    assert!(validate_request(&r, true).is_err());
    r.url = "dubbo://example.com:20880".into();
    let Protocol::Dubbo { config } = &mut r.protocol else {
        unreachable!()
    };
    config.transport = "triple".into();
    assert!(validate_request(&r, true).is_err());
    let Protocol::Dubbo { config } = &mut r.protocol else {
        unreachable!()
    };
    config.transport = "classic".into();
    config.arguments[0].type_name = "java.lang.String;Runtime.exec".into();
    assert!(validate_request(&r, true).is_err());
    let Protocol::Dubbo { config } = &mut r.protocol else {
        unreachable!()
    };
    config.arguments[0].type_name = "java.lang.String".into();
    config.attachments[0].key = "path".into();
    assert!(validate_request(&r, true).is_err());
    let Protocol::Dubbo { config } = &mut r.protocol else {
        unreachable!()
    };
    config.attachments[0].key = "trace-id".into();
    config.arguments[0].value_source = "x".repeat(64 * 1024 + 1);
    assert!(validate_request(&r, true).is_err());
    let Protocol::Dubbo { config } = &mut r.protocol else {
        unreachable!()
    };
    config.arguments[0].value_source = "\"ok\"".into();
    config.attachments[0].local_value = Some("x".repeat(4097));
    assert!(validate_request(&r, true).is_err());
}

#[test]
fn argument_interpolation_quotes_private_text_and_preserves_saved_source() {
    let r = request();
    let before = serde_json::to_value(&r).unwrap();
    let env: Environment = serde_json::from_value(json!({"id":"dev","name":"Dev","variables":[
      {"id":"name","key":"name","value":"\"\\\n","enabled":true,"secret":true},
      {"id":"trace","key":"trace","value":"trace-1","enabled":true}
    ]}))
    .unwrap();
    let resolved = moleapi_core::resolve_request(&r, Some(&env)).unwrap();
    let Protocol::Dubbo { config } = resolved.protocol else {
        unreachable!()
    };
    let value: Value = serde_json::from_str(&config.arguments[0].value_source).unwrap();
    assert_eq!(value, "鼹鼠 \"\\\n");
    assert_eq!(config.attachments[0].value, "trace-1");
    assert_eq!(serde_json::to_value(r).unwrap(), before);
}
