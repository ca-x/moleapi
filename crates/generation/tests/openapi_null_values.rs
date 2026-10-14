use serde_json::{Value, json};
#[test]
fn openapi_literal_defaults_and_examples_preserve_null_presence_and_roundtrip() {
    let missing: openapiv3::Example = serde_json::from_value(json!({})).unwrap();
    assert!(missing.value.is_none());
    let supplied: openapiv3::Example = serde_json::from_value(json!({"value":null})).unwrap();
    assert_eq!(supplied.value, Some(Value::Null));
    assert_eq!(
        serde_json::to_value(&supplied).unwrap(),
        json!({"value":null})
    );
    let media: openapiv3::MediaType = serde_json::from_value(json!({"example":null})).unwrap();
    assert_eq!(media.example, Some(Value::Null));
    assert_eq!(
        serde_json::to_value(&media).unwrap(),
        json!({"example":null})
    );
    let schema: openapiv3::SchemaData =
        serde_json::from_value(json!({"example":null,"default":null})).unwrap();
    assert_eq!(schema.example, Some(Value::Null));
    assert_eq!(schema.default, Some(Value::Null));
    assert_eq!(
        serde_json::to_value(&schema).unwrap(),
        json!({"example":null,"default":null})
    );
    for value in [json!(false), json!(0), json!(""), json!([]), json!({})] {
        let parsed: openapiv3::Example = serde_json::from_value(json!({"value":value})).unwrap();
        assert_eq!(parsed.value, Some(value));
    }
}
