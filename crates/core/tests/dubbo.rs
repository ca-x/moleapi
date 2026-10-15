use moleapi_core::{
    DubboConfig, DubboRegistryAuth, Environment, Protocol, RequestSpec, dubbo_provider_url,
    validate_request,
};
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
fn zookeeper_source_preserves_direct_drafts_and_rejects_unsupported_endpoints() {
    let mut r = request();
    validate_request(&r, false).unwrap();
    r.url = "zookeeper://example.com:2181".into();
    let Protocol::Dubbo { config } = &mut r.protocol else {
        unreachable!()
    };
    config.source_version = 2;
    config.source = "zookeeper".into();
    validate_request(&r, false).unwrap();
    let restored: RequestSpec = serde_json::from_value(serde_json::to_value(&r).unwrap()).unwrap();
    assert_eq!(restored, r);
    for endpoint in [
        "zookeeper://user:secret@example.com:2181",
        "zookeeper://example.com:2181/a",
        "zookeeper://example.com:2181?x=y",
        "http://example.com:2181",
    ] {
        r.url = endpoint.into();
        assert!(validate_request(&r, false).is_err(), "{endpoint}");
    }
}

#[test]
fn zookeeper_digest_credentials_are_explicit_and_bounded() {
    let mut r = request();
    r.url = "zookeeper://example.com:2181".into();
    let Protocol::Dubbo { config } = &mut r.protocol else {
        unreachable!()
    };
    config.source_version = 2;
    config.source = "zookeeper".into();
    config.registry_auth = Some(DubboRegistryAuth {
        username: "reader".into(),
        password: "private".into(),
    });
    validate_request(&r, false).unwrap();
    let Protocol::Dubbo { config } = &mut r.protocol else {
        unreachable!()
    };
    config.registry_auth.as_mut().unwrap().username = "bad:name".into();
    assert!(validate_request(&r, false).is_err());
    let Protocol::Dubbo { config } = &mut r.protocol else {
        unreachable!()
    };
    config.registry_auth.as_mut().unwrap().username = "reader".into();
    config.source = "direct".into();
    config.source_version = 1;
    r.url = "dubbo://example.com:20880".into();
    assert!(validate_request(&r, false).is_err());
}

#[test]
fn nacos_source_requires_a_plain_registry_endpoint() {
    let mut r = request();
    r.url = "nacos://registry.example:8848".into();
    let Protocol::Dubbo { config } = &mut r.protocol else {
        unreachable!()
    };
    config.source_version = 2;
    config.source = "nacos".into();
    config.group = "qa".into();
    validate_request(&r, false).unwrap();
    let Protocol::Dubbo { config } = &mut r.protocol else {
        unreachable!()
    };
    config.group.clear();
    assert!(validate_request(&r, false).is_err());
    let Protocol::Dubbo { config } = &mut r.protocol else {
        unreachable!()
    };
    config.group = "qa".into();
    r.url = "nacos://registry.example:8848/other".into();
    assert!(validate_request(&r, false).is_err());
}

#[test]
fn classic_draft_roundtrips_order_and_original_json_without_becoming_http() {
    let r = request();
    validate_request(&r, true).unwrap();
    let original = serde_json::to_value(&r).unwrap();
    assert!(original["protocol"].get("source").is_none());
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

#[test]
fn registry_result_must_match_classic_service_version_group_and_serialization() {
    let mut config = DubboConfig {
        service: "fixture.EchoService".into(),
        version: "1.0.0".into(),
        group: "qa".into(),
        ..Default::default()
    };
    let valid = "dubbo://127.0.0.1:20880/fixture.EchoService?side=provider&version=1.0.0&group=qa&prefer.serialization=hessian2%2Cfastjson2";
    dubbo_provider_url(valid, &config).unwrap();
    for source in [
        "http://127.0.0.1:20880/fixture.EchoService?side=provider&version=1.0.0&group=qa",
        "dubbo://127.0.0.1:20880/fixture.Other?side=provider&version=1.0.0&group=qa",
        "dubbo://127.0.0.1:20880/fixture.EchoService?side=consumer&version=1.0.0&group=qa",
        "dubbo://127.0.0.1:20880/fixture.EchoService?side=provider&version=2.0&group=qa",
        "dubbo://127.0.0.1:20880/fixture.EchoService?side=provider&version=1.0.0&group=other",
        "dubbo://127.0.0.1:20880/fixture.EchoService?side=provider&version=1.0.0&group=qa&serialization=fastjson2",
        "dubbo://127.0.0.1:20880/fixture.EchoService?side=provider&side=consumer&version=1.0.0&group=qa",
        "dubbo://127.0.0.1:20880/fixture.EchoService?side=provider&version=1.0.0&group=qa&enabled=false",
        "dubbo://user:secret@127.0.0.1:20880/fixture.EchoService?side=provider&version=1.0.0&group=qa",
    ] {
        assert!(
            dubbo_provider_url(source, &config).is_err(),
            "accepted {source}"
        );
    }
    config.version.clear();
    assert!(dubbo_provider_url(valid, &config).is_err());
    config.version = "1.0.0".into();
    config.transport = "triple".into();
    assert!(dubbo_provider_url(valid, &config).is_err());
    config.transport = "classic".into();
    let mut excessive = String::from(
        "dubbo://127.0.0.1:20880/fixture.EchoService?side=provider&version=1.0.0&group=qa",
    );
    for index in 0..126 {
        excessive.push_str(&format!("&option{index}=x"));
    }
    assert!(dubbo_provider_url(&excessive, &config).is_err());
}
