use moleapi_core::Workspace;
use moleapi_formats::{export, import};
use serde_json::{Value, json};
fn workspace(data: moleapi_core::WorkspaceData) -> Workspace {
    Workspace {
        id: "test".into(),
        name: "API".into(),
        revision: 1,
        updated_at: "2026-10-02T00:00:00Z".into(),
        data,
    }
}

#[test]
fn openapi31_keeps_references_extensions_and_raw_source() {
    let source = r#"openapi: 3.1.0
info:
  title: Pet API
  version: '1.0'
x-company: retained
servers:
  - url: https://example.com
paths:
  /pets/{petId}:
    get:
      operationId: getPet
      parameters:
        - $ref: '#/components/parameters/Id'
      responses:
        '200':
          description: A pet
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Pet'
              example:
                name: Mole
components:
  parameters:
    Id:
      name: petId
      in: path
      required: true
      schema:
        type: integer
        example: 7
  schemas:
    Pet:
      type: object
      properties:
        name:
          type: string
"#;
    let imported = import("openapi", source).unwrap();
    assert_eq!(imported.data.specifications[0].source, source);
    let request = &imported.data.collections[0].requests[0];
    assert_eq!(request.url, "{{base_url}}/pets/{{petId}}");
    assert_eq!(request.operation_id.as_deref(), Some("getPet"));
    assert_eq!(request.examples[0].status, 200);
    let output = export(&workspace(imported.data), "openapi", true).unwrap();
    let json: Value = serde_json::from_str(&output.content).unwrap();
    assert_eq!(json["x-company"], "retained");
    assert_eq!(
        json["paths"]["/pets/{petId}"]["get"]["responses"]["200"]["content"]["application/json"]["schema"]
            ["$ref"],
        "#/components/schemas/Pet"
    );
}
#[test]
fn openapi30_uses_compatible_typed_parser_and_rejects_wrong_shapes() {
    let good = json!({"openapi":"3.0.3","info":{"title":"v3","version":"1"},"paths":{"/get":{"get":{"responses":{"200":{"description":"ok"}}}}}});
    assert!(import("openapi", &good.to_string()).is_ok());
    let bad = json!({"openapi":"3.0.3","info":{"title":32,"version":"1"},"paths":{}});
    assert!(import("openapi", &bad.to_string()).is_err());
    assert!(import("openapi", "openapi: 9.0.0").is_err());
}
#[test]
fn curl_quoted_json_headers_and_redirect_semantics_are_preserved() {
    let result=import("curl",r#"curl 'https://example.com/api?q=one' -X POST -H 'Content-Type: application/json' -H 'X-Test: two words' --data-raw '{"name":"Mole"}' -m 2"#).unwrap();
    let request = &result.data.collections[0].requests[0];
    assert_eq!(request.method, "POST");
    assert_eq!(request.body_kind, "json");
    assert_eq!(request.timeout_ms, 2000);
    assert!(!request.follow_redirects);
    assert_eq!(
        request
            .headers
            .iter()
            .find(|h| h.key == "X-Test")
            .unwrap()
            .value,
        "two words"
    );
    assert_eq!(
        serde_json::from_str::<Value>(&request.body).unwrap()["name"],
        "Mole"
    );
    assert!(import("curl", "curl https://example.com --data @/etc/passwd").is_err());
    assert!(import("curl", "curl https://example.com --unknown value").is_err());
    assert!(import("curl", "curl https://a.example https://b.example").is_err());
}
fn postman() -> Value {
    json!({"info":{"name":"Team API","schema":"https://schema.getpostman.com/json/collection/v2.1.0/collection.json"},"auth":{"type":"bearer","bearer":[{"key":"token","value":"top-secret-token","type":"string"}]},"variable":[{"key":"password","value":"collection-secret"}],"item":[{"name":"Folder","item":[{"name":"Read","request":{"method":"GET","url":"https://example.com/get","header":[{"key":"Authorization","value":"Bearer header-secret"}]},"event":[{"listen":"test","script":{"type":"text/javascript","exec":["pm.test('ok', () => pm.response.to.have.status(200));"]}}]}]}]})
}
#[test]
fn postman_official_schema_and_inherited_auth() {
    let source = postman();
    let result = import("postman", &source.to_string()).unwrap();
    assert_eq!(
        result.data.collections[0].requests[0].auth.token,
        "top-secret-token"
    );
    assert_eq!(result.data.specifications[0].source, source.to_string());
    assert!(
        result.data.collections[0].requests[0]
            .post_response_script
            .contains("pm.test")
    );
    let bad = json!({"info":{"name":7,"schema":"https://schema.getpostman.com/json/collection/v2.1.0/collection.json"},"item":[]});
    assert!(import("postman", &bad.to_string()).is_err());
}
#[test]
fn default_export_redacts_credentials_in_canonical_source_and_request() {
    let result = import("postman", &postman().to_string()).unwrap();
    let workspace = workspace(result.data);
    for format in ["moleapi", "postman"] {
        let output = export(&workspace, format, false).unwrap();
        for secret in ["top-secret-token", "header-secret", "collection-secret"] {
            assert!(
                !output.content.contains(secret),
                "secret leaked from {format}"
            );
        }
    }
    assert!(
        export(&workspace, "moleapi", true)
            .unwrap()
            .content
            .contains("top-secret-token")
    );
}
#[test]
fn generated_openapi_and_postman_exports_are_importable() {
    let imported=import("curl",r#"curl -X POST 'https://example.com/users' -H 'Content-Type: application/json' --data-raw '{"name":"Mole"}'"#).unwrap();
    let workspace = workspace(imported.data);
    for format in ["openapi", "postman", "moleapi"] {
        let output = export(&workspace, format, false).unwrap();
        let roundtrip = import(format, &output.content).unwrap();
        assert_eq!(
            roundtrip
                .data
                .collections
                .iter()
                .map(|c| c.requests.len())
                .sum::<usize>(),
            1
        );
    }
}

#[test]
fn live_protocol_config_roundtrips_in_moleapi_and_cannot_silently_disappear_in_other_formats() {
    let mut source = workspace(import("postman", &postman().to_string()).unwrap().data);
    let request = &mut source.data.collections[0].requests[0];
    request.protocol = moleapi_core::Protocol::Sse;
    let encoded = export(&source, "moleapi", true).unwrap();
    let restored = import("moleapi", &encoded.content).unwrap();
    assert_eq!(
        restored.data.collections[0].requests[0].protocol,
        moleapi_core::Protocol::Sse
    );
    for format in ["postman", "openapi"] {
        let error = export(&source, format, true)
            .err()
            .expect("unsupported format must report protocol loss")
            .to_string();
        assert!(error.contains("MoleAPI"), "{error}");
    }
}

#[test]
fn postman_graphql_body_preserves_document_variables_and_scripts() {
    let source = json!({"info":{"name":"GraphQL","schema":"https://schema.getpostman.com/json/collection/v2.1.0/collection.json"},"item":[{"name":"Hello","request":{"method":"POST","url":"https://example.com/graphql","body":{"mode":"graphql","graphql":{"query":"query Hello($name:String!){hello(name:$name)}","variables":"{\"name\":\"MoleAPI\"}"}}},"event":[{"listen":"test","script":{"type":"text/javascript","exec":["console.log('graphql');"]}}]}]});
    let imported = import("postman", &source.to_string()).unwrap();
    assert!(imported.warnings.is_empty());
    let request = &imported.data.collections[0].requests[0];
    let moleapi_core::Protocol::Graphql {
        document,
        variables,
        variables_source,
        ..
    } = &request.protocol
    else {
        panic!("GraphQL mode was lost")
    };
    assert!(document.contains("hello(name:$name)"));
    assert_eq!(variables["name"], "MoleAPI");
    assert!(variables_source.as_ref().unwrap().contains("MoleAPI"));
    assert!(request.post_response_script.contains("graphql"));
    let exported = export(&workspace(imported.data), "postman", true).unwrap();
    let roundtrip = import("postman", &exported.content).unwrap();
    assert!(matches!(
        roundtrip.data.collections[0].requests[0].protocol,
        moleapi_core::Protocol::Graphql { .. }
    ));
}

#[test]
fn retained_protobuf_sources_cannot_silently_disappear_when_requests_switch_to_http() {
    let mut source = workspace(import("postman", &postman().to_string()).unwrap().data);
    let definition = moleapi_core::Specification {
        id: "protobuf-source".into(), name: "Proto source".into(), kind: "protobuf".into(), dialect: "proto3".into(),
        source: json!({"kind":"proto","files":[{"path":"service.proto","content":"syntax = \"proto3\"; message Payload { string text = 1; } service Endpoint { rpc Send(Payload) returns (Payload); }"}],"entry_files":["service.proto"]}).to_string(),
    };
    source.data.specifications.push(definition.clone());
    for format in ["postman", "openapi"] {
        assert!(
            export(&source, format, true)
                .err()
                .expect("foreign export must reject source loss")
                .to_string()
                .contains("MoleAPI")
        );
    }
    let encoded = export(&source, "moleapi", true).unwrap();
    let restored = import("moleapi", &encoded.content).unwrap();
    assert_eq!(
        restored
            .data
            .specifications
            .iter()
            .find(|spec| spec.id == definition.id)
            .unwrap()
            .source,
        definition.source
    );
}

#[test]
fn default_native_export_redacts_protocol_draft_credentials_but_explicit_export_preserves_them() {
    let mut source = workspace(import("postman", &postman().to_string()).unwrap().data);
    source.data.collections[0].requests[0].protocol = moleapi_core::Protocol::Grpc {
        service: "Service".into(),
        method: "Call".into(),
        message_source:
            r#"{"password":"grpc-secret","nested":{"token":"nested-secret"},"text":"keep"}"#.into(),
    };
    let safe = export(&source, "moleapi", false).unwrap();
    assert!(!safe.content.contains("grpc-secret"));
    assert!(!safe.content.contains("nested-secret"));
    assert!(safe.content.contains("keep"));
    let explicit = export(&source, "moleapi", true).unwrap();
    assert!(explicit.content.contains("grpc-secret"));
    source.data.collections[0].requests[0].protocol = serde_json::from_value(json!({"kind":"graphql","document":"query {value}","variables":{"password":"gql-secret"},"variables_source":"{\"token\":\"gql-source-secret\"}","connection_params":{"token":"gql-connection-secret"},"subscription_url":"wss://user:password@example.com/graphql?token=gql-url-secret"})).unwrap();
    let safe = export(&source, "moleapi", false).unwrap();
    for secret in [
        "gql-secret",
        "gql-source-secret",
        "gql-connection-secret",
        "gql-url-secret",
        "user:password",
    ] {
        assert!(!safe.content.contains(secret), "{secret}");
    }
}

#[test]
fn socketio_native_drafts_roundtrip_and_default_export_removes_auth_and_argument_credentials() {
    let mut source = workspace(import("postman", &postman().to_string()).unwrap().data);
    source.data.collections[0].requests[0].url = "wss://example.com/socket.io".into();
    source.data.collections[0].requests[0].protocol = serde_json::from_value(json!({"kind":"socketio","namespace":"/fixture","path":"/custom/socket.io/","auth_source":"{\"token\":\"namespace-secret\"}","listeners":["echo"],"event":"echo","arguments_source":"[{\"password\":\"argument-secret\",\"nested\":{\"_placeholder\":true,\"num\":0}}]","attachments_base64":["AP8="],"request_ack":true,"ack_timeout_ms":5000})).unwrap();
    let explicit = export(&source, "moleapi", true).unwrap();
    let restored = import("moleapi", &explicit.content).unwrap();
    assert_eq!(
        restored.data.collections[0].requests[0].protocol,
        source.data.collections[0].requests[0].protocol
    );
    let safe = export(&source, "moleapi", false).unwrap();
    assert!(!safe.content.contains("namespace-secret"));
    assert!(!safe.content.contains("argument-secret"));
    assert!(safe.content.contains("AP8="));
    for format in ["postman", "openapi"] {
        assert!(
            export(&source, format, true)
                .err()
                .expect("Socket.IO configuration must not disappear")
                .to_string()
                .contains("MoleAPI")
        );
    }
}

#[test]
fn default_protocol_export_scrubs_non_string_credentials_without_erasing_schema_definitions() {
    let mut source = workspace(import("postman", &postman().to_string()).unwrap().data);
    source.data.collections[0].requests[0].url = "wss://example.com".into();
    source.data.collections[0].requests[0].protocol = serde_json::from_value(json!({"kind":"socketio","auth_source":"{\"token\":123456,\"password\":654321,\"client_secret\":{\"encoded\":\"structured-secret\"}}","arguments_source":"[{\"api_token\":987654},{\"secret\":true,\"value\":\"marked-row-secret\"}]"})).unwrap();
    source.data.specifications.push(moleapi_core::Specification {id:"schema-fields".into(),name:"Schema".into(),kind:"openapi".into(),dialect:"3.1.0".into(),source:json!({"openapi":"3.1.0","info":{"title":"Schema","version":"1"},"paths":{},"components":{"schemas":{"Login":{"type":"object","properties":{"password":{"type":"integer"},"token":{"type":"string"}}}}}}).to_string()});
    let safe = export(&source, "moleapi", false).unwrap();
    for value in [
        "123456",
        "654321",
        "987654",
        "structured-secret",
        "marked-row-secret",
    ] {
        assert!(!safe.content.contains(value), "{value}");
    }
    let restored = import("moleapi", &safe.content).unwrap();
    let raw = &restored
        .data
        .specifications
        .iter()
        .find(|item| item.id == "schema-fields")
        .unwrap()
        .source;
    let schema: serde_json::Value = serde_json::from_str(raw).unwrap();
    assert_eq!(
        schema["components"]["schemas"]["Login"]["properties"]["password"]["type"],
        "integer"
    );
    assert_eq!(
        schema["components"]["schemas"]["Login"]["properties"]["token"]["type"],
        "string"
    );
    let explicit = export(&source, "moleapi", true).unwrap();
    assert!(explicit.content.contains("123456"));
}

#[test]
fn payload_url_and_raw_strings_keep_prior_credential_redaction() {
    let mut source = workspace(import("postman", &postman().to_string()).unwrap().data);
    source.data.collections[0].requests[0].url = "wss://example.com".into();
    source.data.collections[0].requests[0].protocol = serde_json::from_value(json!({"kind":"socketio","auth_source":json!({"url":"https://user:url-password@example.com/?token=query-token","raw":json!({"password":7654321}).to_string()}).to_string(),"arguments_source":json!([{"url":"wss://user:ws-password@example.com/?token=ws-query-token"},{"raw":json!({"token":4567891}).to_string()}]).to_string()})).unwrap();
    let exported = export(&source, "moleapi", false).unwrap();
    for value in [
        "url-password",
        "query-token",
        "ws-password",
        "ws-query-token",
        "7654321",
        "4567891",
    ] {
        assert!(!exported.content.contains(value), "{value}");
    }
}

#[test]
fn mqtt_native_drafts_preserve_messages_properties_and_will_with_explicit_private_exports() {
    let mut source = workspace(import("postman", &postman().to_string()).unwrap().data);
    let request = &mut source.data.collections[0].requests[0];
    request.url = "mqtt://example.com:1883".into();
    request.headers.clear();
    request.query.clear();
    request.protocol=serde_json::from_value(json!({"kind":"mqtt","version":"5","client_id":"draft-client","message":{"topic":"private-topic","topic_secret":true,"payload_source":"private-payload","payload_secret":true,"encoding":"text","qos":2,"retain":true,"properties":{"user_properties":[{"key":"tenant","value":"property-secret","secret":true}]}},"subscriptions":[{"filter":"private-filter/#","filter_secret":true}],"saved_messages":[{"id":"sample","name":"Sample","message":{"topic":"demo","encoding":"json","payload_source":"{\"password\":1234567}","properties":{"user_properties":[{"key":"token","value":"saved-prop-secret","secret":false}]}}}],"will":{"message":{"topic":"will-topic","payload_source":"will-secret","payload_secret":true},"delay_interval":1}})).unwrap();
    let explicit = export(&source, "moleapi", true).unwrap();
    let restored = import("moleapi", &explicit.content).unwrap();
    assert_eq!(
        restored.data.collections[0].requests[0].protocol,
        source.data.collections[0].requests[0].protocol
    );
    let safe = export(&source, "moleapi", false).unwrap();
    for value in [
        "private-topic",
        "private-payload",
        "property-secret",
        "private-filter",
        "1234567",
        "saved-prop-secret",
        "will-secret",
    ] {
        assert!(!safe.content.contains(value), "{value}");
    }
    for format in ["postman", "openapi"] {
        assert!(
            export(&source, format, true)
                .err()
                .expect("MQTT must be explicit unsupported")
                .to_string()
                .contains("MoleAPI")
        );
    }
}

#[test]
fn mqtt_private_saved_topic_cannot_survive_default_export_as_its_generated_label() {
    let mut source = workspace(import("postman", &postman().to_string()).unwrap().data);
    let request = &mut source.data.collections[0].requests[0];
    request.url = "mqtt://example.com".into();
    request.headers.clear();
    request.protocol=serde_json::from_value(json!({"kind":"mqtt","saved_messages":[{"id":"saved","name":"customer/acme","message":{"topic":"customer/acme","topic_secret":true}}]})).unwrap();
    let safe = export(&source, "moleapi", false).unwrap();
    assert!(!safe.content.contains("customer/acme"));
    assert!(
        export(&source, "moleapi", true)
            .unwrap()
            .content
            .contains("customer/acme")
    );
}

#[test]
fn soap_exports_screen_xml_values_and_preserve_schema_definitions_and_explicit_sources() {
    let mut source = workspace(import("postman", &postman().to_string()).unwrap().data);
    let request = &mut source.data.collections[0].requests[0];
    request.protocol =
        serde_json::from_value(json!({"kind":"soap","version":"1.1","action":"Echo"})).unwrap();
    request.body = r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><s:Body><Echo><password>payload-secret</password><text xsi:nil="true" xml:lang="en"/></Echo></s:Body></s:Envelope>"#.into();
    let definition = include_str!("../../server/tests/fixtures/soap/spyne11.wsdl")
        .replace(r#"<xs:complexType name="Item">"#, r#"<xs:element name="password" type="xs:string" default="source-password"/><xs:element name="token" type="xs:string"/><xs:complexType name="Item">"#)
        .replace("http://127.0.0.1:18897/", "https://alice:address-password@example.test/service?api_key=address-token");
    let original =
        json!({"entry_file":"entry.wsdl","files":[{"path":"entry.wsdl","content":definition}]})
            .to_string();
    source
        .data
        .specifications
        .push(moleapi_core::Specification {
            id: "wsdl".into(),
            name: "Service".into(),
            kind: "wsdl".into(),
            dialect: "wsdl1.1".into(),
            source: original.clone(),
        });
    let safe = export(&source, "moleapi", false).unwrap();
    for secret in [
        "payload-secret",
        "source-password",
        "address-password",
        "address-token",
        "alice",
    ] {
        assert!(!safe.content.contains(secret), "{secret}");
    }
    let restored = import("moleapi", &safe.content).unwrap();
    let bundle: Value =
        serde_json::from_str(&restored.data.specifications.last().unwrap().source).unwrap();
    let xml = bundle["files"][0]["content"].as_str().unwrap();
    assert!(xml.contains("name=\"password\""));
    assert!(xml.contains("name=\"token\""));
    assert!(xml.contains("type=\"xs:string\""));
    assert!(
        restored.data.collections[0].requests[0]
            .body
            .contains("xsi:nil=\"true\"")
    );
    let explicit = export(&source, "moleapi", true).unwrap();
    let restored = import("moleapi", &explicit.content).unwrap();
    assert_eq!(
        restored.data.specifications.last().unwrap().source,
        original
    );
    assert_eq!(
        restored.data.collections[0].requests[0].body,
        source.data.collections[0].requests[0].body
    );
    source.data.collections[0].requests[0].body = "<broken>payload-secret".into();
    assert!(
        !export(&source, "moleapi", false)
            .unwrap()
            .content
            .contains("payload-secret")
    );
}

#[test]
fn soap_default_export_withholds_decoded_private_entities_in_plain_data_fields() {
    let mut source = workspace(import("postman", &postman().to_string()).unwrap().data);
    source.data.collections[0].variables[0].value = "a&b".into();
    let request = &mut source.data.collections[0].requests[0];
    request.protocol = serde_json::from_value(json!({"kind":"soap","version":"1.1"})).unwrap();
    request.body = r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body><text>a&#38;b</text></s:Body></s:Envelope>"#.into();
    let original = request.body.clone();
    for body in [original.clone(), r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><!--a&b--><?test a&b?><s:Body><text>public</text></s:Body></s:Envelope>"#.into()] {
        source.data.collections[0].requests[0].body = body.clone();
        let safe = export(&source, "moleapi", false).unwrap();
        let restored = import("moleapi", &safe.content).unwrap();
        assert!(restored.data.collections[0].requests[0].body.is_empty());
        let explicit = export(&source, "moleapi", true).unwrap();
        let restored = import("moleapi", &explicit.content).unwrap();
        assert_eq!(restored.data.collections[0].requests[0].body, body);
    }
}

#[test]
fn mcp_native_and_host_sources_roundtrip_and_default_exports_screen_credentials() {
    let mut source = workspace(import("postman", &postman().to_string()).unwrap().data);
    let request = &mut source.data.collections[0].requests[0];
    let raw = r#"{ "command": "/usr/bin/node", "args": ["server.js", "--password", "cli-secret", "--api-key=key-secret"], "env": { "TENANT": "private-env" }, "extra": { "duplicate": "cli-secret", "public": "keep" } }"#;
    request.protocol = serde_json::from_value(json!({"kind":"mcp","transport":"stdio","command":"/usr/bin/node","args":["server.js","--password","cli-secret","--api-key=key-secret"],"env":[{"id":"env","key":"TENANT","value":"private-env","secret":true,"enabled":true}],"arguments_source":"{\"plain\":\"private-env\",\"password\":23,\"public\":\"keep\"}","config_source":raw})).unwrap();
    let safe = export(&source, "moleapi", false).unwrap();
    for secret in ["private-env", "cli-secret", "key-secret"] {
        assert!(!safe.content.contains(secret), "{secret}");
    }
    assert!(safe.content.contains("server.js"));
    assert!(safe.content.contains("keep"));
    let restored = import("moleapi", &safe.content).unwrap();
    assert!(matches!(
        restored.data.collections[0].requests[0].protocol,
        moleapi_core::Protocol::Mcp { .. }
    ));
    let explicit = export(&source, "moleapi", true).unwrap();
    let restored = import("moleapi", &explicit.content).unwrap();
    assert_eq!(
        restored.data.collections[0].requests[0].protocol,
        source.data.collections[0].requests[0].protocol
    );
    if let moleapi_core::Protocol::Mcp { config } =
        &restored.data.collections[0].requests[0].protocol
    {
        assert_eq!(config.config_source.as_deref(), Some(raw));
    }
    for format in ["openapi", "postman"] {
        assert!(
            export(&source, format, true)
                .err()
                .expect("MCP must reject foreign export")
                .to_string()
                .contains("MoleAPI")
        );
    }
}

#[test]
fn mcp_default_exports_screen_original_and_repeated_transport_credentials_after_edits() {
    let mut source = workspace(import("postman", &postman().to_string()).unwrap().data);
    let request = &mut source.data.collections[0].requests[0];
    request.url = "https://example.test/known-bearer/private".into();
    request.auth.token = "known-bearer".into();
    request.headers.push(
        serde_json::from_value(
            json!({"id":"custom","key":"X-Other","value":"known-bearer","enabled":true}),
        )
        .unwrap(),
    );
    request.protocol = serde_json::from_value(json!({"kind":"mcp","command":"/usr/bin/node","transport":"stdio","env":[{"id":"env","key":"TENANT","value":"new-private","secret":true,"enabled":true},{"id":"copy","key":"OTHER","value":"known-bearer","enabled":true}],"args":["server.js","--password","new-cli-private"],"config_source":"{\"command\":\"/usr/bin/node\",\"env\":{\"TENANT\":\"old-private\"},\"headers\":{\"X-Custom\":\"old-header\"},\"args\":[\"--password\",\"old-cli-private\"],\"extra\":{\"env\":\"old-private\",\"header\":\"old-header\",\"arg\":\"old-cli-private\"}}"})).unwrap();
    let safe = export(&source, "moleapi", false).unwrap();
    for value in [
        "known-bearer",
        "new-private",
        "old-private",
        "old-header",
        "old-cli-private",
        "new-cli-private",
    ] {
        assert!(!safe.content.contains(value), "{value}");
    }
    let restored = import(
        "moleapi",
        &export(&source, "moleapi", true).unwrap().content,
    )
    .unwrap();
    assert_eq!(
        restored.data.collections[0].requests[0].protocol,
        source.data.collections[0].requests[0].protocol
    );
    assert_eq!(
        restored.data.collections[0].requests[0].url,
        source.data.collections[0].requests[0].url
    );
}

#[test]
fn mcp_default_exports_do_not_leak_known_secrets_in_json_keys_or_scalars() {
    let mut source = workspace(import("postman", &postman().to_string()).unwrap().data);
    source.data.collections[0].variables[0].value = "31337".into();
    let request = &mut source.data.collections[0].requests[0];
    request.protocol = serde_json::from_value(json!({"kind":"mcp","arguments_source":"{\"31337\":\"public\",\"count\":31337,\"keep\":\"public\"}","config_source":"{\"url\":\"https://example.test/mcp\",\"extra\":{\"31337\":\"public\",\"count\":31337}}"})).unwrap();
    let safe = export(&source, "moleapi", false).unwrap();
    assert!(!safe.content.contains("31337"));
    assert!(safe.content.contains("public"));
}

#[test]
fn a2a_native_sources_and_drafts_restore_and_default_export_screens_credentials() {
    let mut source = workspace(import("postman", &postman().to_string()).unwrap().data);
    let request = &mut source.data.collections[0].requests[0];
    let card = r#"{ "name": "Fixture", "description": "retained", "version": "1", "protocolVersion": "0.3.0", "url": "https://example.test/a2a?api_key=card-secret", "capabilities": { "streaming": true }, "defaultInputModes": ["text/plain"], "defaultOutputModes": ["text/plain"], "skills": [], "token": "literal-card-secret", "extension": { "preserved": true } }"#;
    request.protocol = serde_json::from_value(json!({"kind":"a2a","params_source":"{\"message\":{\"kind\":\"message\",\"messageId\":\"request\",\"role\":\"user\",\"parts\":[{\"kind\":\"text\",\"text\":\"keep\"}]},\"metadata\":{\"token\":\"params-secret\"}}","card_source":card,"interface_url":"https://example.test/a2a?api_key=interface-secret"})).unwrap();
    let safe = export(&source, "moleapi", false).unwrap();
    for value in [
        "card-secret",
        "literal-card-secret",
        "params-secret",
        "interface-secret",
    ] {
        assert!(!safe.content.contains(value), "{value}");
    }
    assert!(safe.content.contains("preserved"));
    assert!(safe.content.contains("keep"));
    let explicit = export(&source, "moleapi", true).unwrap();
    let restored = import("moleapi", &explicit.content).unwrap();
    assert_eq!(
        restored.data.collections[0].requests[0].protocol,
        source.data.collections[0].requests[0].protocol
    );
    for format in ["openapi", "postman"] {
        assert!(
            export(&source, format, true)
                .err()
                .expect("A2A must reject lossy formats")
                .to_string()
                .contains("MoleAPI")
        );
    }
}

#[test]
fn a2a_canonical_card_source_default_export_excludes_copied_private_data() {
    let mut source = workspace(import("postman", &postman().to_string()).unwrap().data);
    source.data.collections[0].variables[0].value = "31337".into();
    let card = r#"{ "name": "Fixture", "description": "safe", "version": "1", "protocolVersion": "0.3.0", "url": "https://example.test/a2a", "capabilities": {}, "defaultInputModes": ["text/plain"], "defaultOutputModes": ["text/plain"], "skills": [], "extension": { "31337": "private-key", "number": 31337, "copied": "31337", "keep": "public" } }"#;
    source
        .data
        .specifications
        .push(moleapi_core::Specification {
            id: "card".into(),
            name: "Agent".into(),
            kind: "a2a-agent-card".into(),
            dialect: "a2a-0.3".into(),
            source: card.into(),
        });
    let safe = export(&source, "moleapi", false).unwrap();
    assert!(!safe.content.contains("31337"));
    assert!(safe.content.contains("public"));
    let restored = import(
        "moleapi",
        &export(&source, "moleapi", true).unwrap().content,
    )
    .unwrap();
    assert_eq!(restored.data.specifications.last().unwrap().source, card);
}

#[test]
fn tcp_native_full_drafts_restore_and_default_binary_copies_are_withheld() {
    let mut workspace = workspace(import("curl", "curl https://example.com").unwrap().data);
    workspace.data.global_variables.push(moleapi_core::Pair {
        id: "private".into(),
        key: "seed".into(),
        value: "private-seed".into(),
        enabled: true,
        secret: Some(true),
        local_value: None,
    });
    let request = &mut workspace.data.collections[0].requests[0];
    request.protocol=serde_json::from_value(serde_json::json!({"kind":"tcp","framing":"length_le","message":{"encoding":"hex","payload_source":"70 72 69 76 61 74 65 2d 73 65 65 64"}})).unwrap();
    request.url = "tcp://localhost:1234".into();
    request.body_kind = "none".into();
    let full = moleapi_formats::export(&workspace, "moleapi", true).unwrap();
    let restored = moleapi_formats::import("moleapi", &full.content).unwrap();
    assert_eq!(restored.data, workspace.data);
    let safe = moleapi_formats::export(&workspace, "moleapi", false).unwrap();
    let value: serde_json::Value = serde_json::from_str(&safe.content).unwrap();
    assert_eq!(
        value["data"]["collections"][0]["requests"][0]["protocol"]["message"]["payload_source"],
        ""
    );
}

#[test]
fn tcp_encoded_json_private_fields_and_copies_are_removed_from_default_export() {
    let mut w = workspace(import("curl", "curl https://example.com").unwrap().data);
    for encoding in ["hex", "base64"] {
        let body = b"{\"password\":\"only-in-this-payload\",\"copy\":\"only-in-this-payload\"}";
        let source = if encoding == "hex" {
            hex::encode(body)
        } else {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD.encode(body)
        };
        let r = &mut w.data.collections[0].requests[0];
        r.url = "tcp://localhost:1234".into();
        r.body_kind = "none".into();
        r.protocol = serde_json::from_value(
            json!({"kind":"tcp","message":{"encoding":encoding,"payload_source":source}}),
        )
        .unwrap();
        let safe = export(&w, "moleapi", false).unwrap();
        let value: Value = serde_json::from_str(&safe.content).unwrap();
        assert_eq!(
            value["data"]["collections"][0]["requests"][0]["protocol"]["message"]["payload_source"],
            ""
        );
        let full = export(&w, "moleapi", true).unwrap();
        assert_eq!(import("moleapi", &full.content).unwrap().data, w.data);
    }
}

#[test]
fn tcp_opaque_binary_draft_requires_explicit_full_export() {
    let mut w = workspace(import("curl", "curl https://example.com").unwrap().data);
    let r = &mut w.data.collections[0].requests[0];
    r.url = "tcp://localhost:1234".into();
    r.body_kind = "none".into();
    r.protocol = serde_json::from_value(
        json!({"kind":"tcp","message":{"encoding":"hex","payload_source":"00 ff 41"}}),
    )
    .unwrap();
    let safe = export(&w, "moleapi", false).unwrap();
    let value: Value = serde_json::from_str(&safe.content).unwrap();
    assert_eq!(
        value["data"]["collections"][0]["requests"][0]["protocol"]["message"]["payload_source"],
        ""
    );
    let full = export(&w, "moleapi", true).unwrap();
    assert_eq!(import("moleapi", &full.content).unwrap().data, w.data);
}

#[test]
fn data_request_exports_screen_credentials_copied_sql_and_keep_explicit_originals() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let config = moleapi_core::DataConfig {
        source: moleapi_core::DataSource::Postgresql,
        sql: "SELECT 'it''s-private' AS copied".into(),
        ..Default::default()
    };
    let request:moleapi_core::RequestSpec=serde_json::from_value(json!({"id":"data","name":"Data request","protocol":{"kind":"data","source":"postgresql","sql":config.sql},"method":"GET","url":"postgresql://user:it%27s-private@db.example/main","description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"verify_tls":true,"follow_redirects":false,"assertions":[],"examples":[]})).unwrap();
    let mut w=workspace(serde_json::from_value(json!({"schema_version":1,"collections":[{"id":"c","name":"Data","description":"","requests":[request]}],"environments":[],"active_environment_id":null})).unwrap());
    let format = "moleapi";
    {
        let safe = export(&w, format, false).unwrap();
        assert!(
            !safe.content.contains("it%27s-private"),
            "{format}:{}",
            safe.content
        );
        assert!(
            !safe.content.contains("it''s-private"),
            "{format}:{}",
            safe.content
        );
        let full = export(&w, format, true).unwrap();
        let restored = import(format, &full.content).unwrap();
        assert_eq!(
            restored.data.collections[0].requests[0].protocol,
            w.data.collections[0].requests[0].protocol
        );
        assert_eq!(
            restored.data.collections[0].requests[0].url,
            w.data.collections[0].requests[0].url
        );
    }
    assert!(
        export(&w, "postman", true).is_err(),
        "Dedicated Data configuration must not be silently exported as HTTP"
    );
    let request = &mut w.data.collections[0].requests[0];
    request.url = String::new();
    request.protocol = moleapi_core::Protocol::Data {
        config: Box::new(moleapi_core::DataConfig {
            source: moleapi_core::DataSource::LocalFile,
            file_name: "selected.csv".into(),
            file_base64: STANDARD.encode(vec![b'a'; 4 * 1024 * 1024]),
            ..Default::default()
        }),
    };
    let safe = export(&w, "moleapi", false).unwrap();
    let safe = import("moleapi", &safe.content).unwrap();
    let moleapi_core::Protocol::Data { config } = &safe.data.collections[0].requests[0].protocol
    else {
        panic!("lost Data protocol")
    };
    assert!(config.file_base64.is_empty());
    let full = export(&w, "moleapi", true).unwrap();
    assert!(full.content.len() > 5 * 1024 * 1024);
    let restored = import("moleapi", &full.content).unwrap();
    assert_eq!(
        restored.data.collections[0].requests[0].protocol,
        w.data.collections[0].requests[0].protocol
    );
}

#[test]
fn structured_file_bodies_private_roundtrip_default_withholding_and_dynamic_modes() {
    let original=import("postman",r#"{"info":{"name":"files","schema":"https://schema.getpostman.com/json/collection/v2.1.0/collection.json"},"item":[{"name":"file","request":{"method":"POST","url":"https://example.com","header":[],"body":{"mode":"formdata","formdata":[{"key":"upload","type":"file","src":[]},{"key":"same","type":"text","value":"first"},{"key":"same","type":"text","value":"second"}]}}}]}"#).unwrap();
    let mut w = workspace(original.data);
    assert!(!original.warnings.is_empty());
    let r = &mut w.data.collections[0].requests[0];
    let body: moleapi_core::MultipartBody = serde_json::from_str(&r.body).unwrap();
    assert_eq!(body.parts.len(), 3);
    assert!(moleapi_core::validate_request(r, false).is_err());
    r.body_kind = "binary".into();
    r.body = json!({"file_name":"a.bin","mime":"image/png","base64":"AP9B"}).to_string();
    let full = export(&w, "moleapi", true).unwrap();
    assert_eq!(import("moleapi", &full.content).unwrap().data, w.data);
    let default = export(&w, "moleapi", false).unwrap();
    let parsed = import("moleapi", &default.content).unwrap();
    let file: moleapi_core::BinaryBody =
        serde_json::from_str(&parsed.data.collections[0].requests[0].body).unwrap();
    assert!(file.base64.is_none());
    let pm = export(&w, "postman", false).unwrap();
    let imported = import("postman", &pm.content).unwrap();
    let r = &imported.data.collections[0].requests[0];
    let file: moleapi_core::BinaryBody = serde_json::from_str(&r.body).unwrap();
    assert_eq!(file.mime, "image/png");
    assert!(file.base64.is_none());
    assert!(export(&w, "postman", true).is_err());
    assert!(export(&w, "openapi", false).is_err());
    w.data.collections[0].requests[0].body_kind = "{{mode}}".into();
    w.data.global_variables.push(moleapi_core::Pair {
        id: "mode".into(),
        key: "mode".into(),
        value: "binary".into(),
        enabled: true,
        secret: None,
        local_value: None,
    });
    let default = export(&w, "moleapi", false).unwrap();
    assert!(!default.content.contains("AP9B"));
    assert!(export(&w, "postman", false).is_err());
    assert!(
        export(&w, "moleapi", true)
            .unwrap()
            .content
            .contains("AP9B")
    );
}

#[test]
fn multipart_privacy_budget_withholding_keeps_default_source_importable() {
    let mut w = workspace(
        import("curl", "curl -X POST https://example.com")
            .unwrap()
            .data,
    );
    w.data.global_variables.push(moleapi_core::Pair {
        id: "private".into(),
        key: "private".into(),
        value: "p".repeat(5000),
        enabled: true,
        secret: Some(true),
        local_value: None,
    });
    let r = &mut w.data.collections[0].requests[0];
    r.body_kind = "multipart".into();
    r.body=json!({"parts":[{"id":"one","name":"field","value":{"kind":"text","text":"hello","mime":""}}]}).to_string();
    let out = export(&w, "moleapi", false).unwrap();
    let parsed = import("moleapi", &out.content).unwrap();
    let body: moleapi_core::MultipartBody =
        serde_json::from_str(&parsed.data.collections[0].requests[0].body).unwrap();
    assert_eq!(body.parts[0].name, "[REDACTED]");
}

#[test]
fn privacy_replacements_cannot_expand_file_body_metadata_past_import_limits() {
    let mut w = workspace(
        import("curl", "curl -X POST https://example.com")
            .unwrap()
            .data,
    );
    let r = &mut w.data.collections[0].requests[0];
    r.auth.password = "x".into();
    r.body_kind = "multipart".into();
    r.body=json!({"parts":[{"id":"p","name":"x".repeat(100),"value":{"kind":"file","file":{"file_name":"x".repeat(100),"mime":"","base64":"AA=="}}}]}).to_string();
    let out = export(&w, "moleapi", false).unwrap();
    let imported = import("moleapi", &out.content).unwrap();
    let body: moleapi_core::MultipartBody =
        serde_json::from_str(&imported.data.collections[0].requests[0].body).unwrap();
    assert!(body.parts[0].name.len() <= 512);
}
