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
