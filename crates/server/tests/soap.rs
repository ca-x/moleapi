mod common;
use common::*;
fn source(version: &str) -> String {
    serde_json::to_string(&json!({"entry_file":"service.wsdl","files":[{"path":"service.wsdl","content":if version=="1.1"{include_str!("fixtures/soap/spyne11.wsdl")}else{include_str!("fixtures/soap/spyne12.wsdl")}}]})).unwrap()
}
#[tokio::test]
async fn wsdl_model_drafts_and_owned_sources() {
    let tmp = tempfile::tempdir().unwrap();
    let app = local(&tmp.path().join("soap.db")).await.unwrap();
    let (_, w) = call(
        &app,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"SOAP","data":example_data()})),
    )
    .await;
    for version in ["1.1", "1.2"] {
        let (status, c) = call(
            &app,
            "POST",
            "/api/soap/import",
            None,
            Some(json!({"workspace_id":w["id"],"name":"Fixture WSDL","source":source(version)})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{c}");
        let op = &c["schema"]["services"][0]["ports"][0]["operations"][0];
        assert!(op["error"].is_null(), "{op}");
        assert!(op["template"].as_str().unwrap().contains("Echo"));
        assert!(
            op["fields"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["repeated"] == true)
        );
        assert!(
            op["fields"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["optional"] == true)
        );
        let spec = &c["specification"];
        assert_eq!(spec["source"], source(version));
    }
    let (status, _) = call(
        &app,
        "POST",
        "/api/soap/import",
        None,
        Some(json!({"workspace_id":"foreign","name":"x","source":source("1.1")})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let mut d = example_data();
    let r = &mut d["collections"][0]["requests"][0];
    r["protocol"] = json!({"kind":"soap","version":"1.1"});
    r["method"] = json!("POST");
    r["body_kind"] = json!("text");
    r["body"] = json!("<incomplete");
    let (status, w) = call(
        &app,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"draft","name":"Draft","data":d})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    let (status, v) = call(
        &app,
        "POST",
        "/api/execute",
        None,
        Some(json!({"workspace_id":"draft","request":w["data"]["collections"][0]["requests"][0]})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{v}");
}
#[test]
fn source_budgets_and_xml_data() {
    for xml in [
        "<!DOCTYPE a [<!ENTITY x 'boom'>]><a>&x;</a>",
        "<a>",
        &format!("{}{}", "<a>".repeat(65), "</a>".repeat(65)),
    ] {
        assert!(moleapi_core::parse_bounded_xml(xml).is_err(), "{xml}");
    }
    let xml =
        moleapi_core::resolve_soap_xml("<a token='{{value}}'><name>{{value}}</name></a>", |_| {
            Ok("<&\"secret".into())
        })
        .unwrap();
    let doc = moleapi_core::parse_bounded_xml(&xml).unwrap();
    assert_eq!(doc.root_element().attribute("token"), Some("<&\"secret"));
    let scrubbed = moleapi_core::redact_soap_xml(&xml).unwrap();
    assert!(!scrubbed.contains("token=\"&lt;"));
    assert!(scrubbed.contains("[REDACTED]"));
    let mut bundle: Value = serde_json::from_str(&source("1.1")).unwrap();
    bundle["files"][0]["path"] = json!("../secret.wsdl");
    let spec = moleapi_core::Specification {
        id: "s".into(),
        name: "s".into(),
        kind: "wsdl".into(),
        dialect: "wsdl1.1".into(),
        source: bundle.to_string(),
    };
    assert!(moleapi_core::soap_schema(&spec).is_err());
}
#[tokio::test]
#[ignore = "Requires the documented real Spyne fixtures on18897/18898"]
async fn real_spyne_http_auth_envelopes_and_faults() {
    let tmp = tempfile::tempdir().unwrap();
    let app = local(&tmp.path().join("real.db")).await.unwrap();
    let (_, w) = call(
        &app,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"SOAP","data":example_data()})),
    )
    .await;
    for (version, port) in [("1.1", 18897), ("1.2", 18898)] {
        let (status,c)=call(&app,"POST","/api/soap/import-url",None,Some(json!({"workspace_id":"w","name":"Real source","url":format!("http://127.0.0.1:{port}/?wsdl")}))).await;
        assert_eq!(status, StatusCode::OK, "{c}");
        let p = &c["schema"]["services"][0]["ports"][0];
        let o = &p["operations"][0];
        let mut r = w["data"]["collections"][0]["requests"][0].clone();
        r["protocol"] = json!({"kind":"soap","version":version,"action":o["action"]});
        r["method"] = json!("POST");
        r["body_kind"] = json!("text");
        r["url"] = json!(format!("http://127.0.0.1:{port}/"));
        r["body"] = json!(
            o["template"]
                .as_str()
                .unwrap()
                .replace("<name></name>", "<name>Alice</name>")
                .replace("<m:name></m:name>", "<m:name>Alice</m:name>")
                .replace("<name />", "<name>Alice</name>")
        );
        r["auth"]["kind"] = json!("bearer");
        r["auth"]["token"] = json!("fixture-token");
        let (status, res) = call(
            &app,
            "POST",
            "/api/execute",
            None,
            Some(json!({"workspace_id":"w","request":r})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{res}");
        assert_eq!(res["status"], 200, "{res}");
        assert!(
            res["body"].as_str().unwrap().contains("Hello Alice"),
            "{res}"
        );
        r["body"] = json!(r["body"].as_str().unwrap().replace("Alice", "fault"));
        let (_, res) = call(
            &app,
            "POST",
            "/api/execute",
            None,
            Some(json!({"workspace_id":"w","request":r})),
        )
        .await;
        assert_eq!(res["status"], 500, "{res}");
        assert!(
            res["soap_fault"]["reason"]
                .as_str()
                .unwrap_or("")
                .contains("Fixture rejected"),
            "{res}"
        );
        r["auth"]["token"] = json!("wrong");
        let (status, res) = call(
            &app,
            "POST",
            "/api/execute",
            None,
            Some(json!({"workspace_id":"w","request":r})),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(res["status"], 401);
    }
}
fn envelope(version: &str, value: &str) -> String {
    let ns = if version == "1.1" {
        "http://schemas.xmlsoap.org/soap/envelope/"
    } else {
        "http://www.w3.org/2003/05/soap-envelope"
    };
    format!(
        "<soap:Envelope xmlns:soap='{ns}'><soap:Body><Echo xmlns='urn:moleapi:soap'><name>{value}</name></Echo></soap:Body></soap:Envelope>"
    )
}
#[tokio::test]
async fn finite_http_scripts_scopes_fault200_history_and_redirect_policy() {
    use axum::routing::{get, post};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let count = Arc::new(AtomicUsize::new(0));
    let hits = count.clone();
    let fault = "<s:Envelope xmlns:s='http://schemas.xmlsoap.org/soap/envelope/'><s:Body><s:Fault><faultcode>s:Client</faultcode><faultstring>Expected failure</faultstring><detail><password>literal-credential</password><message>a&#38;b</message></detail></s:Fault></s:Body></s:Envelope>";
    let (url, server) = serve(
        Router::new()
            .route(
                "/soap",
                post(move |headers: axum::http::HeaderMap, body: String| {
                    let count = hits.clone();
                    async move {
                        count.fetch_add(1, Ordering::SeqCst);
                        assert_eq!(headers["authorization"], "Basic dXNlcjpwYXNz");
                        assert_eq!(headers["soapaction"], "\"Echo\"");
                        let doc = moleapi_core::parse_bounded_xml(&body).unwrap();
                        assert_eq!(
                            doc.descendants()
                                .find(|n| n.is_element() && n.tag_name().name() == "name")
                                .unwrap()
                                .text(),
                            Some("a&b")
                        );
                        ([("content-type", "text/xml")], fault)
                    }
                }),
            )
            .route(
                "/redirect",
                post(|| async { (StatusCode::FOUND, [("location", "/soap")]) }),
            )
            .route(
                "/wsdl",
                get(|| async { include_str!("fixtures/soap/spyne11.wsdl") }),
            ),
    )
    .await;
    let tmp = tempfile::tempdir().unwrap();
    let app = local(&tmp.path().join("finite.db")).await.unwrap();
    let mut d = example_data();
    d["environments"] = json!([{"id":"e","name":"Private","variables":[{"id":"v","key":"private","value":"a&b","enabled":true,"secret":true}]}]);
    d["active_environment_id"] = json!("e");
    let r = &mut d["collections"][0]["requests"][0];
    r["name"] = json!("SOAP {{private}}");
    r["protocol"] = json!({"kind":"soap","version":"1.1","action":"Echo"});
    r["method"] = json!("POST");
    r["url"] = json!(format!("{url}/soap"));
    r["body_kind"] = json!("text");
    r["body"] = json!(envelope("1.1", "before"));
    r["auth"] = json!({"kind":"basic","username":"user","password":"pass","token":""});
    r["pre_request_script"] = json!(
        "pm.test('real envelope',()=>pm.expect(pm.request.body.raw.includes('Envelope')).to.eql(true)); pm.request.body.update({mode:'text',raw:pm.request.body.raw.replace('before','{{private}}')});"
    );
    r["post_response_script"] =
        json!("pm.test('fault keeps HTTP status',()=>pm.expect(pm.response.code).to.eql(200));");
    let (_, w) = call(
        &app,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"SOAP","data":d})),
    )
    .await;
    let base = w["data"]["collections"][0]["requests"][0].clone();
    let (status, res) = call(
        &app,
        "POST",
        "/api/execute",
        None,
        Some(json!({"workspace_id":"w","request":base})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{res}");
    assert_eq!(res["status"], 200, "{res}");
    assert_eq!(res["soap_fault"]["reason"], "Expected failure");
    assert!(
        res["tests"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["passed"] == true),
        "{res}"
    );
    assert_eq!(count.load(Ordering::SeqCst), 1);
    let (_, history) = call(&app, "GET", "/api/workspaces/w/history", None, None).await;
    assert!(
        !history.to_string().contains("literal-credential"),
        "{history}"
    );
    assert!(!history.to_string().contains("a&amp;b"), "{history}");
    assert!(!history.to_string().contains("a&#38;b"), "{history}");
    assert!(
        history.to_string().contains("private SOAP XML values"),
        "{history}"
    );
    assert!(
        history.to_string().contains("SOAP {{private}}"),
        "{history}"
    );
    for script in [
        "pm.request.method='GET';",
        "pm.request.body.update({mode:'text',raw:'<broken'});",
        "pm.request.headers.upsert({key:'SOAPAction',value:'\\\"wrong\\\"'});",
    ] {
        let mut r = base.clone();
        r["pre_request_script"] = json!(script);
        let (status, _) = call(
            &app,
            "POST",
            "/api/execute",
            None,
            Some(json!({"workspace_id":"w","request":r})),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    let mut r = base.clone();
    r["url"] = json!(format!("{url}/redirect"));
    let (status, _) = call(
        &app,
        "POST",
        "/api/execute",
        None,
        Some(json!({"workspace_id":"w","request":r})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(count.load(Ordering::SeqCst), 1);
    let (status, _) = call(
        &app,
        "POST",
        "/api/soap/import",
        None,
        Some(json!({"workspace_id":"w","name":"a&b private label","source":source("1.1")})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let encoded_source = source("1.1").replace(
        "</wsdl:types>",
        "<wsdl:documentation>a&#38;b</wsdl:documentation></wsdl:types>",
    );
    let (status, res) = call(
        &app,
        "POST",
        "/api/soap/import",
        None,
        Some(json!({"workspace_id":"w","name":"Public label","source":encoded_source})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{res}");
    assert!(res.to_string().contains("private scoped"), "{res}");
    let (status, _) = call(
        &app,
        "POST",
        "/api/soap/import-url",
        None,
        Some(json!({"workspace_id":"w","name":"x","url":format!("{url}/redirect")})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    server.abort();
}
#[test]
fn imported_wsdl_xsd_qnames_and_source_guards() {
    let source = json!({"entry_file":"entry.wsdl","files":[{"path":"entry.wsdl","content":include_str!("fixtures/soap/entry.wsdl")},{"path":"bindings.wsdl","content":include_str!("fixtures/soap/bindings.wsdl")},{"path":"types.xsd","content":include_str!("fixtures/soap/types.xsd")}]});
    let spec = moleapi_core::Specification {
        id: "s".into(),
        name: "Imported fixture".into(),
        kind: "wsdl".into(),
        dialect: "wsdl1.1".into(),
        source: source.to_string(),
    };
    let before = spec.source.clone();
    let schema = moleapi_core::soap_schema(&spec).unwrap();
    assert_eq!(spec.source, before);
    let op = &schema.services[0].ports[0].operations[0];
    assert!(op.error.is_none(), "{:?}", op.error);
    assert!(op.template.contains("Echo"));
    assert!(
        op.fields
            .iter()
            .any(|f| f.namespace == "urn:moleapi:soap" && f.repeated)
    );
    for source in [
        source
            .to_string()
            .replace("bindings.wsdl", "https://example.com/bindings.wsdl"),
        source
            .to_string()
            .replace("bindings.wsdl", "../bindings.wsdl"),
        source.to_string().replace(
            r#"schemaLocation=\"types.xsd\""#,
            r#"schemaLocation=\"missing.xsd\""#,
        ),
    ] {
        let spec = moleapi_core::Specification {
            source,
            ..spec.clone()
        };
        assert!(moleapi_core::soap_schema(&spec).is_err());
    }
    for replace in [
        (
            "http://schemas.xmlsoap.org/wsdl/",
            "http://www.w3.org/ns/wsdl",
        ),
        ("style=\\\"document\\\"", "style=\\\"rpc\\\""),
        ("use=\\\"literal\\\"", "use=\\\"encoded\\\""),
    ] {
        let mut spec = spec.clone();
        spec.source = spec.source.replace(replace.0, replace.1);
        let model = moleapi_core::soap_schema(&spec);
        if replace.0.starts_with("http") {
            assert!(model.is_err())
        } else {
            assert!(
                model.unwrap().services[0].ports[0].operations[0]
                    .error
                    .is_some()
            )
        }
    }
    let recursive = spec.source.replace(
        "name=\\\"label\\\" type=\\\"xs:string\\\"",
        "name=\\\"label\\\" type=\\\"tns:Item\\\"",
    );
    let recursive = moleapi_core::soap_schema(&moleapi_core::Specification {
        source: recursive,
        ..spec.clone()
    })
    .unwrap();
    assert!(
        recursive.services[0].ports[0].operations[0]
            .error
            .as_deref()
            .unwrap()
            .contains("budget")
    );
    let huge=moleapi_core::Specification{source:json!({"entry_file":"service.wsdl","files":[{"path":"service.wsdl","content":"x".repeat(moleapi_core::MAX_SOAP_SOURCE+1)}]}).to_string(),..spec};
    assert!(moleapi_core::soap_schema(&huge).is_err());
}
#[tokio::test]
async fn owned_source_restore_selection_and_hosted_network_policy() {
    let tmp = tempfile::tempdir().unwrap();
    let app = hosted(config(
        moleapi_server::sqlite_database_url(&tmp.path().join("owners.db")).unwrap(),
        true,
    ))
    .await
    .unwrap();
    let alice = register(&app, "soap-alice").await;
    let bob = register(&app, "soap-bob").await;
    let (_, w) = call(
        &app,
        "POST",
        "/api/workspaces",
        Some(&alice),
        Some(json!({"id":"w","name":"SOAP","data":example_data()})),
    )
    .await;
    let (status, c) = call(
        &app,
        "POST",
        "/api/soap/import",
        Some(&alice),
        Some(json!({"workspace_id":"w","name":"Original WSDL","source":source("1.1")})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{c}");
    let mut d = w["data"].clone();
    d["specifications"] = json!([c["specification"],{"id":"other","name":"Other retained source","kind":"graphql-sdl","dialect":"graphql","source":"type Query {hello:String}"}]);
    let service = &c["schema"]["services"][0];
    let p = &service["ports"][0];
    let o = &p["operations"][0];
    let r = &mut d["collections"][0]["requests"][0];
    r["protocol"] = json!({"kind":"soap","version":p["version"],"service":service["name"],"port":p["name"],"operation":o["name"],"action":o["action"]});
    r["method"] = json!("POST");
    r["url"] = p["address"].clone();
    r["body_kind"] = json!("text");
    r["body"] = o["template"].clone();
    r["specification_id"] = c["specification"]["id"].clone();
    let (status, saved) = call(
        &app,
        "PUT",
        "/api/workspaces/w",
        Some(&alice),
        Some(json!({"expected_revision":w["revision"],"name":"SOAP","data":d})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["data"]["specifications"][1]["id"], "other");
    let (status, restored) = call(
        &app,
        "POST",
        "/api/soap/schema",
        Some(&alice),
        Some(json!({"workspace_id":"w","specification_id":c["specification"]["id"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{restored}");
    assert_eq!(restored["specification"], c["specification"]);
    let (status,template)=call(&app,"POST","/api/soap/template",Some(&alice),Some(json!({"workspace_id":"w","specification_id":c["specification"]["id"],"service":service["name"],"port":p["name"],"operation":o["name"]}))).await;
    assert_eq!(status, StatusCode::OK, "{template}");
    assert_eq!(template["template"], o["template"]);
    for route in ["/api/soap/schema", "/api/soap/import"] {
        let (status,_)=call(&app,"POST",route,Some(&bob),Some(json!({"workspace_id":"w","specification_id":c["specification"]["id"],"name":"x","source":source("1.1")}))).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
    let base = saved["data"]["collections"][0]["requests"][0].clone();
    let (status, res) = call(
        &app,
        "POST",
        "/api/execute",
        Some(&alice),
        Some(json!({"workspace_id":"w","request":base})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{res}");
    assert!(
        res.to_string().contains("private") || res.to_string().contains("Private"),
        "{res}"
    );
    let (status, res) = call(
        &app,
        "POST",
        "/api/soap/import-url",
        Some(&alice),
        Some(json!({"workspace_id":"w","name":"x","url":"http://127.0.0.1:18897/?wsdl"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{res}");
    let mut r = base.clone();
    r["protocol"]["operation"] = json!("Missing");
    let (status, res) = call(
        &app,
        "POST",
        "/api/execute",
        Some(&alice),
        Some(json!({"workspace_id":"w","request":r})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(res.to_string().contains("operation not found"), "{res}");
    let mut r = base;
    r["body"] = json!(envelope("1.1", "wrong").replace("Echo", "Wrong"));
    let (status, res) = call(
        &app,
        "POST",
        "/api/execute",
        Some(&alice),
        Some(json!({"workspace_id":"w","request":r})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(res.to_string().contains("payload does not match"), "{res}");
}
#[test]
fn xml_qualified_attributes_and_definition_privacy_remain_semantic() {
    let raw = "<s:Envelope xmlns:s='http://schemas.xmlsoap.org/soap/envelope/' xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'><s:Body><Echo xmlns='urn:moleapi:soap' xml:lang='en' xsi:nil='true' plain='{{value}}'><password>literal</password><name>{{value}}</name></Echo></s:Body></s:Envelope>";
    let resolved =
        moleapi_core::resolve_soap_xml(raw, |s| Ok(s.replace("{{value}}", "a&b"))).unwrap();
    let doc = moleapi_core::parse_bounded_xml(&resolved).unwrap();
    let echo = doc
        .descendants()
        .find(|n| n.has_tag_name(("urn:moleapi:soap", "Echo")))
        .unwrap();
    assert_eq!(
        echo.attribute(("http://www.w3.org/2001/XMLSchema-instance", "nil")),
        Some("true")
    );
    assert_eq!(
        echo.attribute(("http://www.w3.org/XML/1998/namespace", "lang")),
        Some("en")
    );
    assert_eq!(echo.attribute("plain"), Some("a&b"));
    let redacted = moleapi_core::redact_soap_xml(&resolved).unwrap();
    assert!(!redacted.contains("literal"));
    let doc = moleapi_core::parse_bounded_xml(&redacted).unwrap();
    assert_eq!(
        doc.descendants()
            .find(|n| n.has_tag_name(("urn:moleapi:soap", "Echo")))
            .unwrap()
            .attribute(("http://www.w3.org/2001/XMLSchema-instance", "nil")),
        Some("true")
    );
    let raw = "<w:definitions xmlns:w='http://schemas.xmlsoap.org/wsdl/' xmlns:x='http://www.w3.org/2001/XMLSchema' xmlns:soap='http://schemas.xmlsoap.org/wsdl/soap/'><w:types><x:schema><x:element name='password' type='x:string' default='literal-source-password'/><x:attribute name='token' fixed='literal-source-token'/></x:schema></w:types><soap:address location='https://alice:literal-source-password@example.test/service?api_key=literal-source-token&amp;plain=preserved'/></w:definitions>";
    let redacted = moleapi_core::redact_soap_source_xml(raw).unwrap();
    assert!(!redacted.contains("literal-source"));
    assert!(!redacted.contains("alice:"));
    let doc = moleapi_core::parse_bounded_xml(&redacted).unwrap();
    let e = doc
        .descendants()
        .find(|n| n.has_tag_name(("http://www.w3.org/2001/XMLSchema", "element")))
        .unwrap();
    assert_eq!(e.attribute("name"), Some("password"));
    assert_eq!(e.attribute("type"), Some("x:string"));
    assert_eq!(e.attribute("default"), Some("[REDACTED]"));
    assert!(redacted.contains("plain=preserved"));
}
#[test]
fn inline_complex_local_form_and_reference_occurrences_are_preserved() {
    let mut bundle: Value = serde_json::from_str(&source("1.1")).unwrap();
    let document = bundle["files"][0]["content"].as_str().unwrap();
    let parsed = moleapi_core::parse_bounded_xml(document).unwrap();
    let schema = parsed
        .descendants()
        .find(|n| n.has_tag_name(("http://www.w3.org/2001/XMLSchema", "schema")))
        .unwrap();
    let replacement = "<xs:schema targetNamespace='urn:moleapi:soap' elementFormDefault='unqualified'><xs:element name='Shared' type='xs:string'/><xs:element name='Echo'><xs:complexType><xs:sequence><xs:element name='local' type='xs:string'/><xs:element ref='tns:Shared' minOccurs='0' maxOccurs='unbounded'/></xs:sequence></xs:complexType></xs:element></xs:schema>";
    bundle["files"][0]["content"] = json!(document.replace(&document[schema.range()], replacement));
    let spec = moleapi_core::Specification {
        id: "s".into(),
        name: "Forms".into(),
        kind: "wsdl".into(),
        dialect: "wsdl1.1".into(),
        source: bundle.to_string(),
    };
    let schema = moleapi_core::soap_schema(&spec).unwrap();
    let op = &schema.services[0].ports[0].operations[0];
    assert!(op.error.is_none(), "{:?}", op.error);
    let doc = moleapi_core::parse_bounded_xml(&op.template).unwrap();
    assert!(
        doc.descendants()
            .any(|n| n.has_tag_name(("urn:moleapi:soap", "Echo")))
    );
    assert!(doc.descendants().any(|n| n.has_tag_name("local")));
    assert!(
        !doc.descendants()
            .any(|n| n.has_tag_name(("urn:moleapi:soap", "local")))
    );
    let field = op.fields.iter().find(|f| f.name == "Shared").unwrap();
    assert!(field.optional && field.repeated);
    assert_eq!(field.namespace, "urn:moleapi:soap");
}
#[test]
fn duplicate_schema_type_symbols_are_rejected() {
    let source = source("1.1");
    for duplicate in [
        r#"<xs:complexType name=\"Item\"><xs:sequence/></xs:complexType>"#,
        r#"<xs:simpleType name=\"Item\"><xs:restriction base=\"xs:string\"/></xs:simpleType>"#,
    ] {
        let spec = moleapi_core::Specification {
            id: "s".into(),
            name: "Duplicates".into(),
            kind: "wsdl".into(),
            dialect: "wsdl1.1".into(),
            source: source.replace("</xs:schema>", &format!("{duplicate}</xs:schema>")),
        };
        let error = moleapi_core::soap_schema(&spec).err().unwrap().to_string();
        assert!(error.contains("Duplicate global schema type"), "{error}");
    }
}
#[test]
fn decoded_privacy_does_not_expand_inherited_namespace_work() {
    let mut xml = String::from("<root");
    for i in 0..2000 {
        xml.push_str(&format!(" xmlns:n{i}='urn:{i}'"));
    }
    xml.push('>');
    xml.push_str(&"<a/>".repeat(19000));
    xml.push_str("</root>");
    let decoded = moleapi_core::soap_xml_data_values(&xml).unwrap();
    assert_eq!(decoded.len(), 2001);
    let wsdl = xml
        .replace(
            "<root",
            "<w:definitions xmlns:w='http://schemas.xmlsoap.org/wsdl/'",
        )
        .replace("</root>", "</w:definitions>");
    let spec = moleapi_core::Specification {
        id: "s".into(),
        name: "Namespaces".into(),
        kind: "wsdl".into(),
        dialect: "wsdl1.1".into(),
        source:
            json!({"entry_file":"service.wsdl","files":[{"path":"service.wsdl","content":wsdl}]})
                .to_string(),
    };
    let error = moleapi_core::soap_schema(&spec).err().unwrap().to_string();
    assert!(error.contains("namespace work budget"), "{error}");
}
