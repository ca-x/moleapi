use moleapi_core::*;
use prost::Message;
use serde_json::json;
fn specification(files: serde_json::Value) -> Specification {
    Specification {
        id: "proto".into(),
        name: "Test".into(),
        kind: "protobuf".into(),
        source: json!({"kind":"proto","files":files,"entry_files":["service.proto"]}).to_string(),
        dialect: "proto3".into(),
    }
}
fn bundle() -> Specification {
    specification(json!([
        {"path":"service.proto","content":include_str!("../../protocols/tests/fixtures/service.proto")},
        {"path":"types.proto","content":include_str!("../../protocols/tests/fixtures/types.proto")}
    ]))
}
#[test]
fn virtual_multifile_metadata_protobuf_json_and_descriptors_roundtrip() {
    let pool = protobuf_pool(&bundle()).unwrap();
    let schema = grpc_schema(&pool).unwrap();
    assert_eq!(schema.services.len(), 1);
    assert_eq!(schema.services[0].methods.len(), 4);
    let descriptor = pool.get_message_by_name("moleapi.fixture.Echo").unwrap();
    let message = grpc_message(descriptor.clone(), r#"{"text":"hello","count":"9223372036854775807","blob":"AP8=","mode":"SPECIAL","number":42,"at":"2026-10-03T00:00:00Z","nested":{"text":"nested"}}"#).unwrap();
    let json = grpc_json(&message).unwrap();
    assert_eq!(json["count"], "9223372036854775807");
    assert_eq!(json["mode"], "SPECIAL");
    assert_eq!(json["blob"], "AP8=");
    assert_eq!(json["at"], "2026-10-03T00:00:00Z");
    assert!(grpc_message(descriptor.clone(), r#"{"label":"a","number":1}"#).is_err());
    assert!(grpc_message(descriptor.clone(), r#"{"unknown":1}"#).is_err());
    assert!(grpc_message(descriptor, "{} {}").is_err());
    for method in &schema.services[0].methods {
        grpc_message(
            pool.get_message_by_name(&method.input_type).unwrap(),
            &method.input_template.to_string(),
        )
        .unwrap();
    }
    let reopened = protobuf_pool(
        &serde_json::from_str::<Specification>(&serde_json::to_string(&bundle()).unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(reopened.services().count(), 1);
    assert!(message.encoded_len() > 0);
}
#[test]
fn bounded_sources_paths_imports_and_recursive_untrusted_proto_reject_cleanly() {
    for path in [
        "../escape.proto",
        "/etc/passwd.proto",
        "C:\\test.proto",
        "a//b.proto",
        "a/./b.proto",
        "a/../b.proto",
    ] {
        assert!(validate_proto_path(path).is_err(), "{path}");
    }
    assert!(protobuf_pool(&specification(json!([{"path":"service.proto","content":"syntax = \"proto3\"; import \"missing.proto\";"}]))).is_err());
    assert!(protobuf_pool(&specification(json!([{"path":"service.proto","content":"syntax = \"proto3\"; import \"../../etc/passwd.proto\";"}]))).is_err());
    let deep = format!(
        "syntax=\"proto3\"; {}{}",
        "message M {".repeat(1000),
        "}".repeat(1000)
    );
    let error = protobuf_pool(&specification(
        json!([{"path":"service.proto","content":deep}]),
    ))
    .unwrap_err();
    assert!(error.to_string().contains("nesting"), "{error}");
    let quoted = "syntax=\"proto3\"; message M { string text=1 [json_name=\"{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{\"]; }";
    assert!(
        protobuf_pool(&specification(
            json!([{"path":"service.proto","content":quoted}])
        ))
        .is_ok()
    );
    assert!(
        protobuf_pool(&specification(
            json!([{"path":"service.proto","content":"x".repeat(256*1024+1)}])
        ))
        .is_err()
    );
    let depth = format!("{}{}", "{\"nested\":".repeat(150), "}".repeat(150));
    let pool = protobuf_pool(&bundle()).unwrap();
    assert!(
        grpc_message(
            pool.get_message_by_name("moleapi.fixture.Echo").unwrap(),
            &depth
        )
        .is_err()
    );
}
#[test]
fn grpc_message_variables_resolve_inside_json_without_brace_confusion() {
    let environment = Environment {
        id: "e".into(),
        name: "e".into(),
        variables: vec![Pair {
            id: "p".into(),
            key: "value".into(),
            value: "secret".into(),
            enabled: true,
            secret: Some(true),
            local_value: None,
        }],
    };
    let resolved = resolve_grpc_source(r#"{"nested":{"text":"{{value}}"}}"#, &environment).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&resolved).unwrap()["nested"]["text"],
        "secret"
    );
    assert!(resolve_grpc_source(r#"{"text":"{{missing}}"}"#, &environment).is_err());
}
#[test]
fn deep_descriptors_and_branching_recursive_templates_are_bounded() {
    use base64::Engine;
    let mut message = prost_types::DescriptorProto {
        name: Some("Leaf".into()),
        ..Default::default()
    };
    for n in 0..150 {
        message = prost_types::DescriptorProto {
            name: Some(format!("N{n}")),
            nested_type: vec![message],
            ..Default::default()
        };
    }
    let descriptors = prost_types::FileDescriptorSet {
        file: vec![prost_types::FileDescriptorProto {
            name: Some("service.proto".into()),
            message_type: vec![message],
            syntax: Some("proto3".into()),
            ..Default::default()
        }],
    }
    .encode_to_vec();
    let mut specification = bundle();
    specification.source=json!({"kind":"descriptor","descriptor_set_base64":base64::engine::general_purpose::STANDARD.encode(descriptors)}).to_string();
    assert!(protobuf_pool(&specification).is_err());
    let fields = (1..=200)
        .map(|n| format!("M m{n}={n};"))
        .collect::<String>();
    let source = format!(
        "syntax=\"proto3\"; message M {{{fields}}} service S {{ rpc Call(M) returns (M); }}"
    );
    let pool = protobuf_pool(&super_specification(&source)).unwrap();
    let schema = grpc_schema(&pool).unwrap();
    let template = &schema.services[0].methods[0].input_template;
    assert!(template.to_string().len() < 100_000);
    grpc_message(
        pool.get_message_by_name("M").unwrap(),
        &template.to_string(),
    )
    .unwrap();
}
fn super_specification(source: &str) -> Specification {
    specification(json!([{"path":"service.proto","content":source}]))
}
