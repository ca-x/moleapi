mod common;
use common::*;
#[tokio::test]
async fn saved_mock_example_uses_status_body_and_only_safe_headers() {
    let dir = tempfile::tempdir().unwrap();
    let router = common::local(&dir.path().join("local.db")).await.unwrap();
    call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"mock","name":"Mock","data":example_data()})),
    )
    .await;
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/mock/mock/r/e")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.headers()["x-example"], "yes");
    assert!(!response.headers().contains_key("connection"));
    assert!(!response.headers().contains_key("x-hop"));
    assert_eq!(
        to_bytes(response.into_body(), 100).await.unwrap().as_ref(),
        b"example body"
    );
    assert_eq!(
        call(&router, "GET", "/api/mock/mock/r/missing", None, None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn saved_schema_generates_direct_and_routed_json_before_optional_script() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("schema.db")).await.unwrap();
    let mut data = example_data();
    data["script_mode"] = json!("v2");
    data["collections"][0]["requests"][0]["url"] = json!("https://example.test/items");
    data["collections"][0]["requests"][0]["examples"][0]["mock_schema"] = json!(
        r#"{"type":"object","properties":{"id":{"type":"integer","minimum":3}},"required":["id"]}"#
    );
    let (status, body) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"Schema","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    for path in ["/api/mock/w/r/e", "/api/mocks/w/items"] {
        let response = router
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(response.headers()["content-type"], "application/json");
        assert_eq!(
            to_bytes(response.into_body(), 1024).await.unwrap().as_ref(),
            br#"{"id":3}"#
        );
    }
    let (_, mut saved) = call(&router, "GET", "/api/workspaces/w", None, None).await;
    saved["data"]["collections"][0]["requests"][0]["examples"][0]["mock_script"] =
        json!("pm.mock.setBody({id:99});");
    let (status, updated) = call(
        &router,
        "PUT",
        "/api/workspaces/w",
        None,
        Some(json!({"name":"Schema","expected_revision":saved["revision"],"data":saved["data"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    let (status, result) = call(&router, "GET", "/api/mock/w/r/e", None, None).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(result, json!({"id":99}));
}

#[tokio::test]
async fn composed_schema_generates_the_same_direct_and_routed_tuple() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("composed-schema.db")).await.unwrap();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = json!("https://example.test/items");
    data["collections"][0]["requests"][0]["examples"][0]["mock_schema"] = json!(
        r#"{"type":"array","prefixItems":[{"oneOf":[{"type":"string","const":"Ada"},{"type":"integer","minimum":5}]},{"type":["null","integer"],"minimum":3}],"items":false,"minItems":2}"#
    );
    let (status, body) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"Composed","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    for path in ["/api/mock/w/r/e", "/api/mocks/w/items"] {
        let response = router
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(response.headers()["content-type"], "application/json");
        assert_eq!(
            to_bytes(response.into_body(), 1024).await.unwrap().as_ref(),
            br#"["Ada",3]"#
        );
    }
}

#[tokio::test]
async fn opted_in_name_inference_reaches_direct_and_routed_mock_without_changing_static_body() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("named-schema.db")).await.unwrap();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = json!("https://example.test/items");
    data["collections"][0]["requests"][0]["examples"][0]["body"] = json!("unchanged static body");
    data["collections"][0]["requests"][0]["examples"][0]["mock_schema"] = json!(
        r#"{"type":"object","properties":{"id":{"type":"integer"},"name":{"type":"string"}},"required":["id","name"]}"#
    );
    data["collections"][0]["requests"][0]["examples"][0]["mock_name_inference"] = json!(true);
    let (status, body) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"Named","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    for path in ["/api/mock/w/r/e", "/api/mocks/w/items"] {
        assert_eq!(
            call(&router, "GET", path, None, None).await,
            (StatusCode::CREATED, json!({"id":1,"name":"Ada Lovelace"}))
        );
    }
    let (_, saved) = call(&router, "GET", "/api/workspaces/w", None, None).await;
    let example = &saved["data"]["collections"][0]["requests"][0]["examples"][0];
    assert_eq!(example["body"], "unchanged static body");
    assert_eq!(example["mock_name_inference"], true);
}

#[tokio::test]
async fn linked_composed_response_preserves_component_schema_on_real_calls() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("linked-composed.db")).await.unwrap();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = json!("https://example.test/items");
    data["collections"][0]["requests"][0]["specification_id"] = json!("spec");
    data["collections"][0]["requests"][0]["examples"][0]["mock_schema_link"] = json!({"specification_id":"spec","pointer":"#/paths/~1items/get/responses/201/content/application~1json/schema"});
    data["collections"][0]["requests"][0]["examples"][0]["mock_name_inference"] = json!(true);
    let source = json!({"openapi":"3.1.0","paths":{"/items":{"get":{"responses":{"201":{"content":{"application/json":{"schema":{"allOf":[{"type":"object","properties":{"id":{"$ref":"#/components/schemas/Id"}},"required":["id"]},{"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}]}}}}}}}},"components":{"schemas":{"Id":{"type":"integer","minimum":3}}}});
    data["specifications"] = json!([{"id":"spec","name":"Spec","kind":"openapi","dialect":"3.1.0","source":source.to_string()}]);
    let (status, body) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"Linked composed","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    for path in ["/api/mock/w/r/e", "/api/mocks/w/items"] {
        let response = router
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(response.headers()["content-type"], "application/json");
        assert_eq!(
            to_bytes(response.into_body(), 1024).await.unwrap().as_ref(),
            br#"{"id":3,"name":"Ada Lovelace"}"#
        );
    }
}

#[tokio::test]
async fn linked_openapi_schema_uses_current_saved_specification_for_direct_and_routed_calls() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("linked-schema.db")).await.unwrap();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = json!("https://example.test/items");
    data["collections"][0]["requests"][0]["specification_id"] = json!("spec");
    let pointer = "#/paths/~1items/get/responses/201/content/application~1vnd.moleapi+json/schema";
    data["collections"][0]["requests"][0]["examples"][0]["mock_schema_link"] =
        json!({"specification_id":"spec","pointer":pointer});
    let definition = |id: u64| {
        json!({"openapi":"3.1.0","paths":{"/items":{"get":{"responses":{"201":{"content":{"application/vnd.moleapi+json":{"schema":{"type":"object","properties":{"id":{"const":id}},"required":["id"]}}}}}}}}}).to_string()
    };
    data["specifications"] = json!([{"id":"spec","name":"Spec","kind":"openapi","dialect":"3.1.0","source":definition(1)}]);
    let (status, body) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"Linked","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    for path in ["/api/mock/w/r/e", "/api/mocks/w/items"] {
        let response = router
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(
            response.headers()["content-type"],
            "application/vnd.moleapi+json"
        );
        assert_eq!(
            to_bytes(response.into_body(), 1024).await.unwrap().as_ref(),
            br#"{"id":1}"#
        );
    }
    let (_, mut saved) = call(&router, "GET", "/api/workspaces/w", None, None).await;
    saved["data"]["specifications"][0]["source"] = json!(definition(2));
    let (status, updated) = call(
        &router,
        "PUT",
        "/api/workspaces/w",
        None,
        Some(json!({"name":"Linked","expected_revision":saved["revision"],"data":saved["data"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(
        call(&router, "GET", "/api/mock/w/r/e", None, None).await,
        (StatusCode::CREATED, json!({"id":2}))
    );
}

#[tokio::test]
async fn linked_openapi_choice_validation_is_owner_scoped_and_rejects_unsupported_schema() {
    let dir = tempfile::tempdir().unwrap();
    let router = hosted(config(
        moleapi_server::sqlite_database_url(&dir.path().join("linked-choice.db")).unwrap(),
        true,
    ))
    .await
    .unwrap();
    let owner = register(&router, "choiceowner").await;
    let foreign = register(&router, "choiceother").await;
    let mut data = example_data();
    data["collections"][0]["requests"][0]["specification_id"] = json!("spec");
    let source = |unsupported: bool| {
        json!({"openapi":"3.1.0","paths":{"/items":{"get":{"responses":{"201":{"content":{"application/json":{"schema":if unsupported {json!({"type":"string","pattern":".*"})} else {json!({"type":"integer","minimum":3})}}}}}}}}}).to_string()
    };
    data["specifications"] = json!([{"id":"spec","name":"Spec","kind":"openapi","dialect":"3.1.0","source":source(false)}]);
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"W","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let endpoint = "/api/workspaces/w/mock-schema/validate";
    let input = json!({"request_id":"r","specification_id":"spec","pointer":"#/paths/~1items/get/responses/201/content/application~1json/schema","expected_revision":saved["revision"]});
    assert_eq!(
        call(
            &router,
            "POST",
            endpoint,
            Some(&foreign),
            Some(input.clone())
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, result) = call(&router, "POST", endpoint, Some(&owner), Some(input.clone())).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(
        result,
        json!({"method":"GET","status":201,"media_type":"application/json"})
    );
    let mut changed = saved;
    changed["data"]["specifications"][0]["source"] = json!(source(true));
    let (status, updated) = call(
        &router,
        "PUT",
        "/api/workspaces/w",
        Some(&owner),
        Some(json!({"name":"W","expected_revision":changed["revision"],"data":changed["data"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(
        call(&router, "POST", endpoint, Some(&owner), Some(input.clone()))
            .await
            .0,
        StatusCode::CONFLICT
    );
    let mut current = input;
    current["expected_revision"] = updated["revision"].clone();
    let (status, invalid) = call(&router, "POST", endpoint, Some(&owner), Some(current)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{invalid}");
}

#[tokio::test]
async fn saved_error_status_example_retains_body_and_headers() {
    let directory = tempfile::tempdir().unwrap();
    let router = common::local(&directory.path().join("errors.db"))
        .await
        .unwrap();
    let mut payload = example_data();
    payload["collections"][0]["requests"][0]["examples"][0]["status"] = json!(404);
    payload["collections"][0]["requests"][0]["examples"][0]["body"] = json!("custom mock failure");
    let (status, _) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"mock-error","name":"Errors","data":payload})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/mock/mock-error/r/e")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(response.headers().get("x-example").unwrap(), "yes");
    assert_eq!(
        to_bytes(response.into_body(), 1000).await.unwrap().as_ref(),
        b"custom mock failure"
    );
}
#[tokio::test]
async fn dynamic_mock_queries_owned_dataset_and_sees_only_safe_incoming_request_fields() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("dynamic.db")).await.unwrap();
    let mut data = example_data();
    data["script_mode"] = json!("v2");
    data["datasets"] = json!([{"id":"d","name":"Rows","source":{"format":"json","source":"[{\"value\":\"dynamic\"}]"}}]);
    data["collections"][0]["requests"][0]["examples"][0]["mock_script"] = json!(
        "if(!pm.request.url.includes('safe=yes')||pm.request.url.includes('token=hidden')||pm.request.headers.get('cookie')!==undefined||pm.request.headers.get('x-hop')!==undefined||pm.request.headers.get('x-safe')!=='visible')throw new Error('unsafe incoming Mock request');const result=await pm.datasets('d').executeQuery('SELECT value FROM data');for await(const row of result.rows){pm.mock.setStatus(202);pm.mock.setHeader('x-row',row.value);pm.mock.setBody({value:row.value});}"
    );
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"Dynamic","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/mock/w/r/e?safe=yes&token=hidden")
                .header("cookie", "private-cookie")
                .header("connection", "X-Hop")
                .header("x-hop", "private-hop")
                .header("x-safe", "visible")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(response.headers()["x-row"], "dynamic");
    assert_eq!(
        to_bytes(response.into_body(), 1024).await.unwrap().as_ref(),
        b"{\"value\":\"dynamic\"}"
    );
    let (_, mut saved) = call(&router, "GET", "/api/workspaces/w", None, None).await;
    for script in [
        "pm.request.url='http://evil.test';pm.mock.setBody('leak');",
        "pm.mock.setHeader('bad name','leak');",
    ] {
        saved["data"]["collections"][0]["requests"][0]["examples"][0]["mock_script"] =
            json!(script);
        let (status,updated)=call(&router,"PUT","/api/workspaces/w",None,Some(json!({"name":"Dynamic","expected_revision":saved["revision"],"data":saved["data"]}))).await;
        assert_eq!(status, StatusCode::OK, "{updated}");
        saved = updated;
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/mock/w/r/e")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        assert!(!String::from_utf8_lossy(&body).contains("leak"));
    }
}
#[tokio::test]
async fn dynamic_mock_loads_query_backed_dataset_through_host_bridge() {
    use base64::Engine;
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("dynamic-live.db")).await.unwrap();
    let mut data = example_data();
    data["script_mode"] = json!("v2");
    let mut source = data["collections"][0]["requests"][0].clone();
    source["id"] = json!("data-query");
    source["url"] = json!("");
    source["protocol"] = json!({"kind":"data","source":"local_file","file_name":"rows.csv","file_format":"csv","file_base64":base64::engine::general_purpose::STANDARD.encode(b"value\nlive-private\n"),"table_name":"data","sql":"SELECT value FROM data","read_only":true});
    data["collections"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"sources","name":"Sources","description":"","requests":[source]}));
    data["datasets"] = json!([{"id":"live","name":"Live","query":{"request_id":"data-query"}}]);
    data["collections"][0]["requests"][0]["examples"][0]["mock_script"] = json!(
        "const result=await pm.datasets('live').executeQuery('SELECT value FROM data');for await(const row of result.rows)pm.mock.setBody(row.value);"
    );
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"Dynamic","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/mock/w/r/e")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(
        to_bytes(response.into_body(), 1024).await.unwrap().as_ref(),
        b"live-private"
    );
    let (_, mut saved) = call(&router, "GET", "/api/workspaces/w", None, None).await;
    saved["data"]["collections"][0]["requests"][0]["examples"][0]["mock_script"] = json!(
        "const result=await pm.datasets('live').executeQuery('SELECT value FROM data');for await(const row of result.rows)throw new Error(row.value);"
    );
    let (status, updated) = call(
        &router,
        "PUT",
        "/api/workspaces/w",
        None,
        Some(json!({"name":"Dynamic","expected_revision":saved["revision"],"data":saved["data"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/mock/w/r/e")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(
        !String::from_utf8_lossy(&to_bytes(response.into_body(), 1024).await.unwrap())
            .contains("live-private")
    );
}
#[tokio::test]
async fn hosted_dynamic_mock_does_not_cross_workspace_owners() {
    let dir = tempfile::tempdir().unwrap();
    let router = hosted(config(
        format!(
            "sqlite://{}?mode=rwc",
            dir.path().join("hosted.db").display()
        ),
        true,
    ))
    .await
    .unwrap();
    let alice = register(&router, "mockalice").await;
    let bob = register(&router, "mockbobby").await;
    let mut data = example_data();
    data["script_mode"] = json!("v2");
    data["collections"][0]["requests"][0]["examples"][0]["mock_script"] =
        json!("pm.mock.setBody('owner-only');");
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            Some(&alice),
            Some(json!({"id":"w","name":"W","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&router, "GET", "/api/mock/w/r/e", Some(&bob), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (status, result) = call(&router, "GET", "/api/mock/w/r/e", Some(&alice), None).await;
    assert_eq!(status, StatusCode::CREATED, "{result}");
    assert_eq!(result["raw"], "owner-only");
}

#[tokio::test]
async fn routed_mock_selects_matching_example_then_falls_back() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("matching.db")).await.unwrap();
    let mut data = example_data();
    let request = &mut data["collections"][0]["requests"][0];
    request["method"] = json!("POST");
    request["url"] = json!("https://example.test/v1/items");
    request["examples"] = json!([
        {"id":"default","name":"Default","status":200,"headers":[],"body":"fallback"},
        {"id":"matched","name":"Matched","status":202,"headers":[],"body":"selected","mock_match":{"priority":5,"query":[{"id":"q","key":"mode","value":"fast","enabled":true}],"headers":[{"id":"h","key":"X-Choice","value":"fast","enabled":true}],"body":"hello"}},
        {"id":"lower","name":"Lower","status":203,"headers":[],"body":"lower","mock_match":{"priority":1,"body":"hello"}}
    ]);
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"W","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/mocks/w/v1/items?mode=fast")
                .header("x-choice", "fast")
                .body(Body::from("hello"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(
        to_bytes(response.into_body(), 100).await.unwrap().as_ref(),
        b"selected"
    );
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/mocks/w/v1/items?mode=slow")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        to_bytes(response.into_body(), 100).await.unwrap().as_ref(),
        b"fallback"
    );
    assert_eq!(
        call(&router, "GET", "/api/mocks/w/v1/items", None, None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn routed_mock_matches_path_variables_duplicate_query_and_selected_script_only() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("matching-script.db")).await.unwrap();
    let mut data = example_data();
    data["script_mode"] = json!("v2");
    let request = &mut data["collections"][0]["requests"][0];
    request["method"] = json!("POST");
    request["url"] = json!("https://example.test/v1/{item}");
    request["examples"] = json!([
        {"id":"default","name":"Default","status":200,"headers":[],"body":"fallback"},
        {"id":"matched","name":"Matched","status":202,"headers":[],"body":"seed","mock_match":{"query":[{"id":"q1","key":"tag","value":"one","enabled":true},{"id":"q2","key":"tag","value":"one","enabled":true}],"headers":[{"id":"h","key":"X-Mode","value":"fast","enabled":true}]},"mock_script":"pm.mock.setBody(pm.request.method+':'+pm.request.body.raw)"},
        {"id":"unselected","name":"Unselected","status":203,"headers":[],"body":"never","mock_match":{"priority":90,"headers":[{"id":"h2","key":"X-Mode","value":"never","enabled":true}]},"mock_script":"throw new Error('unselected-secret')"}
    ]);
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"W","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/mocks/w/v1/123?tag=one&tag=one")
                .header("x-mode", "fast")
                .body(Body::from("hello"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(
        to_bytes(response.into_body(), 100).await.unwrap().as_ref(),
        b"POST:hello"
    );
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/mocks/w/v1/123?tag=one")
                .header("x-mode", "fast")
                .body(Body::from("hello"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        to_bytes(response.into_body(), 100).await.unwrap().as_ref(),
        b"fallback"
    );
}

#[tokio::test]
async fn hosted_routed_mock_denies_another_owner() {
    let dir = tempfile::tempdir().unwrap();
    let router = hosted(config(
        format!(
            "sqlite://{}?mode=rwc",
            dir.path().join("owners.db").display()
        ),
        true,
    ))
    .await
    .unwrap();
    let alice = register(&router, "matchalice").await;
    let bob = register(&router, "matchbobby").await;
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = json!("https://example.test/items");
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            Some(&alice),
            Some(json!({"id":"w","name":"W","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&router, "GET", "/api/mocks/w/items", Some(&bob), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&router, "GET", "/api/mocks/w/items", Some(&alice), None)
            .await
            .0,
        StatusCode::CREATED
    );
}

#[tokio::test]
async fn routed_mock_prefers_literal_path_and_requires_saved_query_pairs() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("specificity.db")).await.unwrap();
    let mut data = example_data();
    let mut generic = data["collections"][0]["requests"][0].clone();
    generic["id"] = json!("generic");
    generic["url"] = json!("https://example.test/v1/{id}");
    generic["examples"][0]["body"] = json!("generic");
    let mut literal = generic.clone();
    literal["id"] = json!("literal");
    literal["url"] = json!("https://example.test/v1/special");
    literal["query"] = json!([{"id":"q","key":"mode","value":"exact","enabled":true}]);
    literal["examples"][0]["body"] = json!("literal");
    data["collections"][0]["requests"] = json!([generic, literal]);
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"W","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, answer) = call(
        &router,
        "GET",
        "/api/mocks/w/v1/special?mode=exact",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(answer["raw"], "literal");
    let (status, answer) = call(&router, "GET", "/api/mocks/w/v1/special", None, None).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(answer["raw"], "generic");
}

#[tokio::test]
async fn routed_mock_excludes_connection_nominated_headers_and_bounds_body() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("safe-input.db")).await.unwrap();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = json!("https://example.test/items");
    data["collections"][0]["requests"][0]["examples"] = json!([
        {"id":"default","name":"Default","status":200,"headers":[],"body":"safe"},
        {"id":"hop","name":"Hop","status":202,"headers":[],"body":"unsafe","mock_match":{"headers":[{"id":"h","key":"X-Hop","value":"private","enabled":true}]}}
    ]);
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"W","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/mocks/w/items")
                .header("connection", "x-hop")
                .header("x-hop", "private")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        to_bytes(response.into_body(), 100).await.unwrap().as_ref(),
        b"safe"
    );
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/mocks/w/items")
                .body(Body::from(vec![b'a'; 1024 * 1024 + 1]))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(
        !String::from_utf8_lossy(&to_bytes(response.into_body(), 1024).await.unwrap())
            .contains("unsafe")
    );
}

#[tokio::test]
async fn routed_mock_rejects_nested_template_segments_and_large_uri_components() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("uri-bounds.db")).await.unwrap();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = json!("https://example.test/v1/{{id}}");
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"W","data":data.clone()}))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&router, "GET", "/api/mocks/w/v1/ordinary", None, None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let mut bounded = data["collections"][0]["requests"][0].clone();
    bounded["id"] = json!("bounded");
    bounded["url"] = json!("https://example.test/v1/{id}");
    data["collections"][0]["requests"]
        .as_array_mut()
        .unwrap()
        .push(bounded);
    assert_eq!(
        call(
            &router,
            "PUT",
            "/api/workspaces/w",
            None,
            Some(json!({"name":"W","expected_revision":1,"data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let long_query = format!("/api/mocks/w/v1/ordinary?x={}", "a".repeat(9_000));
    assert_eq!(
        call(&router, "GET", &long_query, None, None).await.0,
        StatusCode::BAD_REQUEST
    );
    let long_path = format!("/api/mocks/w/v1/{}", "a".repeat(5_000));
    assert_eq!(
        call(&router, "GET", &long_path, None, None).await.0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn saved_mock_delay_applies_after_static_or_dynamic_response_construction_and_is_logged() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("delay.db")).await.unwrap();
    let mut data = example_data();
    data["script_mode"] = json!("v2");
    data["collections"][0]["requests"][0]["url"] = json!("https://example.test/items");
    data["collections"][0]["requests"][0]["examples"][0]["delay_ms"] = json!(120);
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"W","data":data.clone()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let started = std::time::Instant::now();
    let (status, response) = call(&router, "GET", "/api/mock/w/r/e", None, None).await;
    assert_eq!(status, StatusCode::CREATED, "{response}");
    assert!(started.elapsed() >= std::time::Duration::from_millis(100));
    let (_, logs) = call(&router, "GET", "/api/workspaces/w/mock-calls", None, None).await;
    assert!(logs[0]["elapsed_ms"].as_u64().unwrap() >= 100);
    data["collections"][0]["requests"][0]["examples"][0]["mock_script"] =
        json!("pm.mock.setBody('dynamic')");
    assert_eq!(
        call(
            &router,
            "PUT",
            "/api/workspaces/w",
            None,
            Some(json!({"name":"W","expected_revision":saved["revision"],"data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let started = std::time::Instant::now();
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/mocks/w/items")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(
        to_bytes(response.into_body(), 100).await.unwrap().as_ref(),
        b"dynamic"
    );
    assert!(started.elapsed() >= std::time::Duration::from_millis(100));
}

#[tokio::test]
async fn delayed_mock_admission_is_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let slots = std::sync::Arc::new(tokio::sync::Semaphore::new(32));
    let router = moleapi_server::local_with_mock_delay_slots_for_test(
        &dir.path().join("delay-capacity.db"),
        std::path::Path::new(env!("CARGO_BIN_EXE_moleapi-server")),
        slots.clone(),
    )
    .await
    .unwrap();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["examples"][0]["delay_ms"] = json!(30_000);
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"W","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let mut calls = tokio::task::JoinSet::new();
    for _ in 0..32 {
        let router = router.clone();
        calls.spawn(async move { call(&router, "GET", "/api/mock/w/r/e", None, None).await.0 });
    }
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while slots.available_permits() != 0 {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("32 requests did not enter the delayed Mock queue");
    assert_eq!(
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            call(&router, "GET", "/api/mock/w/r/e", None, None)
        )
        .await
        .expect("the 33rd Mock request did not receive a prompt capacity response")
        .0,
        StatusCode::TOO_MANY_REQUESTS
    );
    calls.abort_all();
    while calls.join_next().await.is_some() {}
    let (status, logs) = call(&router, "GET", "/api/workspaces/w/mock-calls", None, None).await;
    assert_eq!(status, StatusCode::OK, "{logs}");
    assert_eq!(logs.as_array().unwrap().len(), 1);
    assert_eq!(
        logs.as_array()
            .unwrap()
            .iter()
            .filter(|row| row["response_status"] == 429)
            .count(),
        1
    );
}

#[tokio::test]
async fn edited_workspace_cannot_serve_a_delayed_static_mock() {
    let dir = tempfile::tempdir().unwrap();
    let slots = std::sync::Arc::new(tokio::sync::Semaphore::new(32));
    let router = moleapi_server::local_with_mock_delay_slots_for_test(
        &dir.path().join("delay-revision.db"),
        std::path::Path::new(env!("CARGO_BIN_EXE_moleapi-server")),
        slots.clone(),
    )
    .await
    .unwrap();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["examples"][0]["delay_ms"] = json!(5_000);
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"W","data":data.clone()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let running_router = router.clone();
    let running =
        tokio::spawn(
            async move { call(&running_router, "GET", "/api/mock/w/r/e", None, None).await },
        );
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while slots.available_permits() != 31 {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("Mock request did not enter the delay after reading the workspace");
    data["collections"][0]["requests"][0]["examples"][0]["body"] = json!("updated");
    assert_eq!(
        call(
            &router,
            "PUT",
            "/api/workspaces/w",
            None,
            Some(json!({"name":"W","expected_revision":saved["revision"],"data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, body) = running.await.unwrap();
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(!body.to_string().contains("example body"));
}
