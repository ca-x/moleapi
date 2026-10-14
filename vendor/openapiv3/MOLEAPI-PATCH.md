# OpenAPIV3 explicit null preservation

Source: crates.io openapiv3 2.2.0, https://github.com/glademiller/openapiv3.
Upstream MIT/Apache-2.0 notices, source, fixtures and tests are retained.
Original archive SHA256: `5c8d427828b22ae1fff2833a03d8486c2c881367f1c336349f307f321e7f4d05`.

Serde Option<Value> normally collapses explicit JSON null into None. Example.value,
MediaType.example and SchemaData.example/default now use a small standard Serde value
deserializer: present null becomes Some(Value::Null), while absent fields default None.
Serialization retains present null and omits None. This fixes actual named-template null
selection and preserves default/example precedence without a parallel OpenAPI parser.
The generated projects do not depend on this parser patch.
