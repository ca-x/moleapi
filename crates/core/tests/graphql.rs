use moleapi_core::*;
use serde_json::json;
fn request(protocol: serde_json::Value) -> RequestSpec {
    serde_json::from_value(json!({"protocol":protocol,"id":"r","name":"GraphQL","method":"GET","url":"https://example.com/graphql","description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":true,"verify_tls":true,"assertions":[],"examples":[]})).unwrap()
}
#[test]
fn selected_ast_variables_bounds_and_backward_defaults() {
    for protocol in [
        json!({"kind":"graphql"}),
        json!({"kind":"graphql","document":"query {"}),
        json!({"kind":"graphql","document":"query A {one} query B {two}"}),
        json!({"kind":"graphql","document":"query Required($n:Int!) {value(n:$n)}","variables":{}}),
        json!({"kind":"graphql","document":"query Required($n:Int!) {value(n:$n)}","variables":{"n":2147483648i64}}),
    ] {
        let r = request(protocol);
        assert!(validate_request(&r, true).is_ok(), "drafts remain editable");
        assert!(resolve_request(&prepare_graphql(&r).unwrap(), None).is_err());
    }
    let r = request(
        json!({"kind":"graphql","document":"query Default($n:Int!=3){value(n:$n)}","variables":{}}),
    );
    assert!(resolve_request(&prepare_graphql(&r).unwrap(), None).is_ok());
    let r = request(json!({"kind":"graphql","document":"query {nested{value}}","variables":{}}));
    assert!(
        resolve_request(&prepare_graphql(&r).unwrap(), None).is_ok(),
        "adjacent GraphQL closing braces are syntax"
    );
    let r = request(json!({"kind":"graphql","document":" ".repeat(MAX_GRAPHQL_DOCUMENT+1)}));
    assert!(validate_request(&r, true).is_err());
    assert!(validate_request(&request(json!({"kind":"graphql","variables":[]})), true).is_err());
    let mut retained = request(json!({"kind":"graphql","document":"{one}"}));
    retained.body_kind = "json".into();
    retained.body = "{incomplete HTTP draft".into();
    assert!(validate_request(&retained, true).is_ok());
    assert!(resolve_request(&prepare_graphql(&retained).unwrap(), None).is_ok());
    let old = request(json!({"kind":"http"}));
    assert_eq!(old.protocol, Protocol::Http);
}
#[test]
fn invalid_variable_drafts_preserved_and_sources_authoritative() {
    let r = request(
        json!({"kind":"graphql","document":"query Q($name:String!){hello(name:$name)}","variables":{"name":"old"},"variables_source":"{broken"}),
    );
    assert!(validate_request(&r, true).is_ok());
    assert!(prepare_graphql(&r).is_err());
    assert_eq!(
        serde_json::to_value(&r).unwrap()["protocol"]["variables_source"],
        "{broken"
    );
    let r = request(
        json!({"kind":"graphql","document":"query Q($name:String!){hello(name:$name)}","variables":{"name":"old"},"variables_source":"{\"name\":\"new\"}"}),
    );
    let resolved = resolve_request(&prepare_graphql(&r).unwrap(), None).unwrap();
    assert_eq!(graphql_payload(&resolved).unwrap().variables["name"], "new");
    let r = request(json!({"kind":"graphql","document":"{one}","variables_source":"[]"}));
    assert!(prepare_graphql(&r).is_err());
}
#[test]
fn schema_parser_keeps_original_and_reports_real_syntax_errors() {
    let source = "# source retained\ntype Query { hello: String! }\n";
    let mut spec = Specification {
        id: "s".into(),
        name: "s".into(),
        kind: "graphql-sdl".into(),
        source: source.into(),
        dialect: "graphql".into(),
    };
    assert_eq!(graphql_schema_sdl(&spec).unwrap(), source);
    spec.source = "type Query {".into();
    assert!(graphql_schema_sdl(&spec).is_err());
    spec.kind = "graphql-introspection".into();
    spec.source = "{\"data\":{}}".into();
    assert!(graphql_schema_sdl(&spec).is_err());
}

#[test]
fn introspection_sdl_preserves_null_roots_despite_conventional_object_names() {
    let types: Vec<_> = ["Query", "Mutation", "Subscription"].into_iter().map(|name|json!({
        "kind":"OBJECT","name":name,"description":null,"fields":[{"name":"value","description":null,"args":[],"type":{"kind":"SCALAR","name":"String","ofType":null},"isDeprecated":false,"deprecationReason":null}],"interfaces":[],"inputFields":null,"enumValues":null,"possibleTypes":null
    })).chain(std::iter::once(json!({"kind":"SCALAR","name":"String","description":null,"fields":null,"interfaces":null,"inputFields":null,"enumValues":null,"possibleTypes":null}))).collect();
    let data = json!({"__schema":{"queryType":{"name":"Query"},"mutationType":null,"subscriptionType":null,"types":types,"directives":[]}});
    let introspected: cynic_introspection::IntrospectionQuery =
        serde_json::from_value(data.clone()).unwrap();
    let expected = introspected.into_schema().unwrap();
    let specification = Specification {
        id: "s".into(),
        name: "s".into(),
        kind: "graphql-introspection".into(),
        source: json!({"data":data}).to_string(),
        dialect: "graphql".into(),
    };
    let sdl = graphql_schema_sdl(&specification).unwrap();
    let parsed = async_graphql_parser::parse_schema(&sdl).unwrap();
    let roots = parsed
        .definitions
        .iter()
        .find_map(|definition| match definition {
            async_graphql_parser::types::TypeSystemDefinition::Schema(definition) => {
                Some(&definition.node)
            }
            _ => None,
        })
        .expect("explicit schema prevents conventional-name root inference");
    assert_eq!(
        roots.query.as_ref().map(|root| root.node.as_str()),
        Some(expected.query_type.as_str())
    );
    assert_eq!(
        roots.mutation.as_ref().map(|root| root.node.as_str()),
        expected.mutation_type.as_deref()
    );
    assert_eq!(
        roots.subscription.as_ref().map(|root| root.node.as_str()),
        expected.subscription_type.as_deref()
    );
    assert!(sdl.contains("type Mutation"));
    assert!(sdl.contains("type Subscription"));
}
#[test]
fn nested_sdl_is_rejected_before_recursive_parsing() {
    let depth = 40;
    let specification = Specification {
        id: "s".into(),
        name: "s".into(),
        kind: "graphql-sdl".into(),
        source: format!(
            "type Query {{ value: {}String{} }}",
            "[".repeat(depth),
            "]".repeat(depth)
        ),
        dialect: "graphql".into(),
    };
    assert!(
        graphql_schema_sdl(&specification).is_err(),
        "small deeply nested list types require a structural quota before recursive parsing"
    );
}

#[test]
fn very_deep_graphql_input_is_contained_and_quoted_delimiters_are_not_depth() {
    const CHILD: &str = "MOLEAPI_GRAPHQL_DEPTH_PROBE";
    if std::env::var_os(CHILD).is_some() {
        let depth = 4000;
        for source in [
            format!(
                "type Query {{ value: {}String{} }}",
                "[".repeat(depth),
                "]".repeat(depth)
            ),
            format!(
                "type Query {{ value(arg: String = {}null{}): String }}",
                "[".repeat(depth),
                "]".repeat(depth)
            ),
        ] {
            let spec = Specification {
                id: "s".into(),
                name: "s".into(),
                kind: "graphql-sdl".into(),
                source,
                dialect: "graphql".into(),
            };
            assert!(
                graphql_schema_sdl(&spec)
                    .unwrap_err()
                    .to_string()
                    .contains("nesting")
            );
        }
        for document in [
            format!(
                "query Q($v:{}String{}){{value}}",
                "[".repeat(depth),
                "]".repeat(depth)
            ),
            format!(
                "query Q($v:String={}null{}){{value}}",
                "[".repeat(depth),
                "]".repeat(depth)
            ),
        ] {
            let r = request(json!({"kind":"graphql","document":document}));
            assert!(
                graphql_payload(&r)
                    .unwrap()
                    .selected_operation()
                    .unwrap_err()
                    .to_string()
                    .contains("nesting")
            );
        }
        return;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "very_deep_graphql_input_is_contained_and_quoted_delimiters_are_not_depth",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "isolated depth probe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let delimiters = "[({".repeat(4000);
    let source =
        format!("# {delimiters}\n\"\"\"{delimiters}\"\"\"\ntype Query {{ value: String }}");
    let spec = Specification {
        id: "s".into(),
        name: "s".into(),
        kind: "graphql-sdl".into(),
        source: source.clone(),
        dialect: "graphql".into(),
    };
    assert_eq!(graphql_schema_sdl(&spec).unwrap(), source);
    let document = format!(
        "{{ value(arg: {}) }}",
        serde_json::to_string(&delimiters).unwrap()
    );
    assert!(
        graphql_payload(&request(json!({"kind":"graphql","document":document})))
            .unwrap()
            .selected_operation()
            .is_ok()
    );
}

#[test]
fn graphql_js16_portable_introspection_interoperates() {
    let source = include_str!("fixtures/graphql-js16-introspection.json");
    let specification = Specification {
        id: "js16".into(),
        name: "GraphQL-JS16".into(),
        kind: "graphql-introspection".into(),
        source: source.into(),
        dialect: "graphql-june2018".into(),
    };
    let sdl = graphql_schema_sdl(&specification).unwrap_or_else(|error| panic!("{error:#}"));
    assert!(sdl.contains("type Query"));
    assert!(sdl.contains("type Mutation"));
    assert!(sdl.contains("type Subscription"));
    assert_eq!(specification.source, source);
}

#[test]
fn custom_new_directive_location_reports_precise_limit_without_dropping_source() {
    let source = include_str!("fixtures/graphql-js16-introspection.json");
    let mut data: serde_json::Value = serde_json::from_str(source).unwrap();
    data["data"]["__schema"]["directives"].as_array_mut().unwrap().push(json!({"name":"customOnDirective","description":null,"args":[],"locations":["DIRECTIVE_DEFINITION"]}));
    let original = data.to_string();
    let specification = Specification {
        id: "s".into(),
        name: "s".into(),
        kind: "graphql-introspection".into(),
        source: original.clone(),
        dialect: "graphql".into(),
    };
    let error = graphql_schema_sdl(&specification).unwrap_err();
    assert!(
        error.to_string().contains(
            "Unsupported GraphQL SDL directive location DIRECTIVE_DEFINITION for @customOnDirective"
        ),
        "{error:#}"
    );
    assert_eq!(specification.source, original);
}

#[test]
fn saved_sdl_new_directive_location_reports_specific_unsupported_location() {
    let specification = Specification {
        id: "s".into(),
        name: "s".into(),
        kind: "graphql-sdl".into(),
        source: "directive @customOnDirective on DIRECTIVE_DEFINITION\ntype Query {value:String}"
            .into(),
        dialect: "graphql".into(),
    };
    let error = graphql_schema_sdl(&specification).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Unsupported GraphQL SDL directive location DIRECTIVE_DEFINITION"),
        "{error:#}"
    );
    let mut named_field = specification.clone();
    named_field.source = "type Query { DIRECTIVE_DEFINITION:String }".into();
    assert_eq!(
        graphql_schema_sdl(&named_field).unwrap(),
        named_field.source
    );
}
