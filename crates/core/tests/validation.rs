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
            local_value: None,
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
    assert!(w.documents.is_empty());
    assert_eq!(w.script_mode, ScriptMode::V1);
    let mut async_workspace = w.clone();
    async_workspace.script_mode = ScriptMode::V2;
    let restored: WorkspaceData =
        serde_json::from_value(serde_json::to_value(&async_workspace).unwrap()).unwrap();
    assert_eq!(restored.script_mode, ScriptMode::V2);
    assert!(
        serde_json::from_value::<WorkspaceData>(
            json!({"schema_version":1,"collections":[],"environments":[],"script_mode":"v3"})
        )
        .is_err()
    );
    let r = request();
    assert!(r.specification_id.is_none());
    assert!(r.operation_id.is_none());
    assert!(validate_workspace(&w).is_ok());
}

#[test]
fn markdown_documents_validate_ids_folders_titles_and_source_budgets() {
    let mut data: WorkspaceData = serde_json::from_value(json!({
        "schema_version":1,
        "collections":[{"id":"c","name":"C","description":"","requests":[]}],
        "environments":[],"active_environment_id":null,
        "documents":[{"id":"d","name":"Guide","collection_id":"c","source":"# Hello"}]
    }))
    .unwrap();
    assert!(data.documents[0].secret);
    assert!(validate_workspace(&data).is_ok());
    let encoded = serde_json::to_value(&data).unwrap();
    assert_eq!(encoded["documents"][0]["secret"], true);

    data.documents.push(data.documents[0].clone());
    assert!(validate_workspace(&data).is_err());
    data.documents.pop();
    data.documents[0].collection_id = Some("missing".into());
    assert!(validate_workspace(&data).is_err());
    data.documents[0].collection_id = Some("c".into());
    for invalid in ["", " padded ", "bad\nname"] {
        data.documents[0].name = invalid.into();
        assert!(validate_workspace(&data).is_err(), "{invalid:?}");
    }
    data.documents[0].name = "Guide".into();
    data.documents[0].source = "x".repeat(256 * 1024 + 1);
    assert!(validate_workspace(&data).is_err());
    data.documents[0].source = "x".repeat(256 * 1024);
    data.documents = (0..9)
        .map(|index| MarkdownDocument {
            id: format!("d{index}"),
            name: format!("Guide {index}"),
            collection_id: Some("c".into()),
            source: "x".repeat(256 * 1024),
            secret: true,
        })
        .collect();
    assert!(validate_workspace(&data).is_err());
}
#[test]
fn mock_implementation_scripts_require_explicit_v2_mode() {
    let mut request = request();
    request.examples.push(serde_json::from_value(json!({"id":"mock","name":"Dynamic","status":200,"headers":[],"body":"static","mock_script":"pm.mock.setBody('dynamic')"})).unwrap());
    let mut workspace: WorkspaceData = serde_json::from_value(
        json!({"schema_version":1,"collections":[],"environments":[],"active_environment_id":null}),
    )
    .unwrap();
    workspace.collections.push(Collection {
        id: "c".into(),
        name: "C".into(),
        description: String::new(),
        parent_id: None,
        variables_enabled: None,
        auth: None,
        variables: vec![],
        pre_request_script: String::new(),
        post_response_script: String::new(),
        requests: vec![request],
    });
    assert!(validate_workspace(&workspace).is_err());
    workspace.script_mode = ScriptMode::V2;
    validate_workspace(&workspace).unwrap();
}

#[test]
fn mock_data_bindings_roundtrip_and_require_a_saved_view() {
    let mut request = request();
    request.url = "https://example.com/items/{id}".into();
    request.examples.push(serde_json::from_value(json!({
        "id":"e","name":"Data","status":200,"headers":[],"body":"static",
        "mock_data":{"dataset_id":"d","view_id":"by-id","shape":"first","parameters":[{"source":"path","name":"id"}]}
    })).unwrap());
    let mut workspace: WorkspaceData = serde_json::from_value(json!({
        "schema_version":1,"collections":[],"environments":[],"active_environment_id":null,
        "datasets":[{"id":"d","name":"Rows","source":{"format":"json","source":"[{\"id\":\"one\"}]"},"views":[{"id":"by-id","name":"By ID","sql":"SELECT id FROM data WHERE id = ?"}]}]
    })).unwrap();
    workspace.collections.push(Collection {
        id: "c".into(),
        name: "C".into(),
        description: String::new(),
        parent_id: None,
        variables_enabled: None,
        auth: None,
        variables: vec![],
        pre_request_script: String::new(),
        post_response_script: String::new(),
        requests: vec![request],
    });
    assert!(validate_workspace(&workspace).is_err());
    workspace.script_mode = ScriptMode::V2;
    validate_workspace(&workspace).unwrap();
    let saved = serde_json::to_value(&workspace).unwrap();
    assert_eq!(
        saved["collections"][0]["requests"][0]["examples"][0]["mock_data"]["view_id"],
        "by-id"
    );
    let restored: WorkspaceData = serde_json::from_value(saved).unwrap();
    assert_eq!(restored, workspace);
    workspace.collections[0].requests[0].examples[0] = serde_json::from_value(json!({
        "id":"e","name":"Data","status":200,"headers":[],"body":"static",
        "mock_data":{"dataset_id":"d","view_id":"missing","parameters":[]}
    }))
    .unwrap();
    assert!(validate_workspace(&workspace).is_err());
    let binding = workspace.collections[0].requests[0].examples[0]
        .mock_data
        .as_mut()
        .unwrap();
    binding.view_id = "by-id".into();
    binding.parameters = vec![MockDataParameter {
        source: MockDataSource::Query,
        name: "token".into(),
    }];
    assert!(validate_workspace(&workspace).is_err());
    workspace.collections[0].requests[0].examples[0]
        .mock_data
        .as_mut()
        .unwrap()
        .parameters = vec![MockDataParameter {
        source: MockDataSource::Query,
        name: "cookie".into(),
    }];
    assert!(validate_workspace(&workspace).is_err());
    workspace.collections[0].requests[0].examples[0]
        .mock_data
        .as_mut()
        .unwrap()
        .parameters = vec![MockDataParameter {
        source: MockDataSource::Header,
        name: "Authorization".into(),
    }];
    assert!(validate_workspace(&workspace).is_err());
    workspace.collections[0].requests[0].examples[0]
        .mock_data
        .as_mut()
        .unwrap()
        .parameters = vec![MockDataParameter {
        source: MockDataSource::Path,
        name: "missing".into(),
    }];
    assert!(validate_workspace(&workspace).is_err());
    workspace.collections[0].requests[0].examples[0]
        .mock_data
        .as_mut()
        .unwrap()
        .parameters
        .clear();
    workspace.collections[0].requests[0].examples[0]
        .headers
        .push(Pair {
            id: "h".into(),
            key: "Content-Type".into(),
            value: "text/plain".into(),
            enabled: true,
            secret: None,
            local_value: None,
        });
    assert!(validate_workspace(&workspace).is_err());
    workspace.collections[0].requests[0].examples[0]
        .headers
        .clear();
    workspace.collections[0].requests[0].examples[0].mock_schema = "{\"type\":\"object\"}".into();
    assert!(validate_workspace(&workspace).is_err());
}

#[test]
fn linked_mock_schema_requires_matching_saved_operation_status_and_media() {
    let source = json!({"openapi":"3.1.0","paths":{"/items":{"get":{"responses":{"200":{"content":{"application/json":{"schema":{"type":"string","example":"linked"}}}}}}}}}).to_string();
    let mut request = request();
    request.specification_id = Some("spec".into());
    request.examples.push(serde_json::from_value(json!({"id":"e","name":"Linked","status":200,"headers":[],"body":"static","mock_schema_link":{"specification_id":"spec","pointer":"#/paths/~1items/get/responses/200/content/application~1json/schema"}})).unwrap());
    let mut workspace: WorkspaceData = serde_json::from_value(
        json!({"schema_version":1,"collections":[],"environments":[],"active_environment_id":null}),
    )
    .unwrap();
    workspace.specifications.push(Specification {
        id: "spec".into(),
        name: "Spec".into(),
        kind: "openapi".into(),
        source,
        dialect: "3.1.0".into(),
    });
    workspace.collections.push(Collection {
        id: "c".into(),
        name: "C".into(),
        description: String::new(),
        parent_id: None,
        variables_enabled: None,
        auth: None,
        variables: vec![],
        pre_request_script: String::new(),
        post_response_script: String::new(),
        requests: vec![request],
    });
    assert!(validate_workspace(&workspace).is_ok());
    let example = &mut workspace.collections[0].requests[0].examples[0];
    example.status = 201;
    assert!(validate_workspace(&workspace).is_err());
    workspace.collections[0].requests[0].examples[0].status = 200;
    workspace.collections[0].requests[0].method = "POST".into();
    assert!(validate_workspace(&workspace).is_err());
    workspace.collections[0].requests[0].method = "GET".into();
    workspace.collections[0].requests[0].examples[0].mock_schema = r#"{"type":"string"}"#.into();
    assert!(validate_workspace(&workspace).is_err());
    workspace.collections[0].requests[0].examples[0]
        .mock_schema
        .clear();
    workspace.collections[0].requests[0].examples[0]
        .headers
        .push(Pair {
            id: "h".into(),
            key: "Content-Type".into(),
            value: "text/plain".into(),
            enabled: true,
            secret: None,
            local_value: None,
        });
    assert!(validate_workspace(&workspace).is_err());
    workspace.collections[0].requests[0].examples[0]
        .headers
        .clear();
    let original = workspace.collections[0].requests[0].examples[0].clone();
    for index in 1..=256 {
        let mut extra = original.clone();
        extra.id = format!("linked-{index}");
        workspace.collections[0].requests[0].examples.push(extra);
    }
    assert!(validate_workspace(&workspace).is_err());
}

#[test]
fn mock_match_rules_are_additive_and_bounded() {
    let mut request = request();
    request.examples.push(serde_json::from_value(json!({"id":"e","name":"Matched","status":200,"headers":[],"body":"ok","mock_match":{"priority":7,"query":[{"id":"q","key":"mode","value":"fast","enabled":true}],"headers":[],"body":"exact"}})).unwrap());
    assert!(validate_request(&request, true).is_ok());
    let saved = serde_json::to_value(&request).unwrap();
    assert_eq!(saved["examples"][0]["mock_match"]["priority"], 7);
    assert_eq!(saved["examples"][0]["mock_match"]["body"], "exact");
    let restored: RequestSpec = serde_json::from_value(saved).unwrap();
    assert_eq!(
        serde_json::to_value(restored).unwrap()["examples"][0]["mock_match"]["priority"],
        7
    );

    let mut invalid = request.clone();
    invalid.examples[0] = serde_json::from_value(json!({"id":"e","name":"Matched","status":200,"headers":[],"body":"ok","mock_match":{"priority":101,"query":[{"id":"q","key":"mode","value":"fast","enabled":true}]}})).unwrap();
    assert!(validate_request(&invalid, true).is_err());
    invalid.examples[0] = serde_json::from_value(json!({"id":"e","name":"Matched","status":200,"headers":[],"body":"ok","mock_match":{"headers":[{"id":"h","key":"Authorization","value":"secret","enabled":true}]}})).unwrap();
    assert!(validate_request(&invalid, true).is_err());
    invalid.examples[0] = serde_json::from_value(
        json!({"id":"e","name":"Matched","status":200,"headers":[],"body":"ok","mock_match":{}}),
    )
    .unwrap();
    assert!(validate_request(&invalid, true).is_err());
}

#[test]
fn saved_mock_delay_is_additive_and_limited_to_thirty_seconds() {
    let mut request = request();
    request.examples.push(
        serde_json::from_value(
            json!({"id":"e","name":"Delayed","status":202,"headers":[],"body":"ok","delay_ms":125}),
        )
        .unwrap(),
    );
    assert!(validate_request(&request, true).is_ok());
    assert_eq!(
        serde_json::to_value(&request).unwrap()["examples"][0]["delay_ms"],
        125
    );
    let old: Example = serde_json::from_value(
        json!({"id":"old","name":"Old","status":200,"headers":[],"body":"ok"}),
    )
    .unwrap();
    assert!(serde_json::to_value(old).unwrap().get("delay_ms").is_none());
    request.examples[0] = serde_json::from_value(
        json!({"id":"e","name":"Too slow","status":202,"headers":[],"body":"ok","delay_ms":30001}),
    )
    .unwrap();
    assert!(validate_request(&request, true).is_err());
    assert!(
        serde_json::from_value::<Example>(
            json!({"id":"e","name":"Invalid","status":200,"headers":[],"body":"ok","delay_ms":-1})
        )
        .is_err()
    );
}
#[test]
fn canonical_specification_references_and_size_are_checked() {
    let mut r = request();
    r.specification_id = Some("spec".into());
    let mut w = WorkspaceData {
        script_mode: ScriptMode::V1,
        scenarios: vec![],
        datasets: vec![],
        auth: None,
        global_variables: vec![],
        pre_request_script: String::new(),
        post_response_script: String::new(),
        specifications: vec![],
        documents: vec![],
        schema_version: 1,
        collections: vec![Collection {
            variables_enabled: None,
            parent_id: None,
            auth: None,
            variables: vec![],
            pre_request_script: String::new(),
            post_response_script: String::new(),
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
