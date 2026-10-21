mod common;
use common::*;
use std::time::Duration;

struct Fixture {
    _directory: tempfile::TempDir,
    router: Router,
    other: Router,
    owner: String,
    editor: String,
    viewer: String,
    foreign: String,
    member_id: String,
    base: String,
}

async fn fixture() -> Fixture {
    let directory = tempfile::tempdir().unwrap();
    let settings = config(
        moleapi_server::sqlite_database_url(&directory.path().join("shared-mock.db")).unwrap(),
        true,
    );
    let router = hosted(settings.clone()).await.unwrap();
    let other = hosted(settings).await.unwrap();
    let owner = register(&router, "mockowner").await;
    let editor = register(&router, "mockeditor").await;
    let viewer = register(&router, "mockviewer").await;
    let foreign = register(&router, "mockforeign").await;
    let mut data = example_data();
    data["script_mode"] = json!("v2");
    let request = &mut data["collections"][0]["requests"][0];
    request["method"] = json!("POST");
    request["url"] = json!("https://example.test/items/{id}");
    request["examples"] = json!([
        {"id":"fallback","name":"Fallback","status":200,"headers":[],"body":"fallback"},
        {"id":"matched","name":"Matched","status":202,"headers":[],"body":"seed","mock_match":{"priority":10,"query":[{"id":"a","key":"mode","value":"fast","enabled":true},{"id":"b","key":"mode","value":"fast","enabled":true}],"headers":[{"id":"h","key":"X-Mode","value":"fast","enabled":true}],"body":"hello"},"mock_script":"if(pm.request.headers.get('authorization')!==undefined||pm.request.headers.get('cookie')!==undefined||pm.request.url.includes('private-query'))throw new Error('private input');pm.mock.setBody({method:pm.request.method,body:pm.request.body.raw});"},
        {"id":"unselected","name":"Unselected","status":203,"headers":[],"body":"never","mock_match":{"priority":99,"headers":[{"id":"none","key":"X-Mode","value":"never","enabled":true}]},"mock_script":"throw new Error('unselected-private')"}
    ]);
    let (status, created) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Shared matching","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let mut owner_id = String::new();
    let mut member_id = String::new();
    for (token, role) in [(&editor, "editor"), (&viewer, "viewer")] {
        let (_, invitation) = call(
            &router,
            "POST",
            "/api/workspaces/w/invitations",
            Some(&owner),
            Some(json!({"role":role,"expires_in_days":1})),
        )
        .await;
        let (status, accepted) = call(
            &router,
            "POST",
            "/api/workspace-invitations/accept",
            Some(token),
            Some(json!({"token":invitation["token"]})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{accepted}");
        owner_id = accepted["owner_id"].as_str().unwrap().into();
        if role == "editor" {
            member_id = accepted["member_id"].as_str().unwrap().into();
        }
    }
    Fixture {
        _directory: directory,
        router,
        other,
        owner,
        editor,
        viewer,
        foreign,
        member_id,
        base: format!("/api/owners/{owner_id}/workspaces/w"),
    }
}

async fn invoke(
    router: &Router,
    path: &str,
    token: &str,
    method: &str,
    body: &str,
) -> (StatusCode, Value) {
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("authorization", format!("Bearer {token}"))
                .header("cookie", "private-cookie")
                .header("x-mode", "fast")
                .header("content-type", "text/plain")
                .body(Body::from(body.to_owned()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 30 * 1024 * 1024)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| json!({"raw":String::from_utf8_lossy(&bytes)})),
    )
}

#[tokio::test]
async fn shared_routed_mock_matches_saved_rules_and_runs_only_the_selected_script() {
    let fixture = fixture().await;
    let path = format!(
        "{}/mocks/items/123?mode=fast&mode=fast&access_token=private-query",
        fixture.base
    );
    let (status, matched) = invoke(&fixture.router, &path, &fixture.editor, "POST", "hello").await;
    assert_eq!(status, StatusCode::ACCEPTED, "{matched}");
    assert_eq!(matched, json!({"method":"POST","body":"hello"}));
    let (status, fallback) = invoke(
        &fixture.router,
        &format!("{}/mocks/items/123?mode=fast", fixture.base),
        &fixture.editor,
        "POST",
        "hello",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{fallback}");
    assert_eq!(fallback["raw"], "fallback");
    assert_eq!(
        invoke(&fixture.router, &path, &fixture.editor, "POST", "different")
            .await
            .1["raw"],
        "fallback"
    );
    assert_eq!(
        invoke(&fixture.router, &path, &fixture.editor, "GET", "hello")
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        invoke(&fixture.router, &path, &fixture.viewer, "POST", "hello")
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        invoke(&fixture.router, &path, &fixture.foreign, "POST", "hello")
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        invoke(
            &fixture.router,
            "/api/owners/other/workspaces/w/mocks/items/123",
            &fixture.editor,
            "POST",
            "hello"
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        invoke(
            &fixture.router,
            "/api/mocks/w/items/123?mode=fast&mode=fast",
            &fixture.editor,
            "POST",
            "hello"
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        invoke(
            &fixture.router,
            &path,
            &fixture.editor,
            "POST",
            &"x".repeat(1024 * 1024 + 1)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (_, audit) = call(
        &fixture.router,
        "GET",
        &format!("{}/audit", fixture.base),
        Some(&fixture.owner),
        None,
    )
    .await;
    assert!(
        audit
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["actor_id"] == fixture.member_id
                && event["action"] == "mock.route"
                && event["result"] == "started")
    );
    for private in [
        "hello",
        "private-cookie",
        "private-query",
        fixture.editor.as_str(),
    ] {
        assert!(!audit.to_string().contains(private));
    }
}

#[tokio::test]
async fn shared_routed_mock_withholds_a_delayed_result_after_role_or_workspace_change() {
    for change in ["role", "workspace"] {
        let fixture = fixture().await;
        let (_, mut saved) = call(
            &fixture.router,
            "GET",
            "/api/workspaces/w",
            Some(&fixture.owner),
            None,
        )
        .await;
        saved["data"]["collections"][0]["requests"][0]["examples"][1]["delay_ms"] = json!(2000);
        let (status, _) = call(&fixture.router, "PUT", "/api/workspaces/w", Some(&fixture.owner), Some(json!({"name":saved["name"],"data":saved["data"],"expected_revision":saved["revision"]}))).await;
        assert_eq!(status, StatusCode::OK);
        let router = fixture.router.clone();
        let member = fixture.editor.clone();
        let path = format!("{}/mocks/items/123?mode=fast&mode=fast", fixture.base);
        let pending =
            tokio::spawn(async move { invoke(&router, &path, &member, "POST", "hello").await });
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let (_, audit) = call(
                    &fixture.other,
                    "GET",
                    &format!("{}/audit", fixture.base),
                    Some(&fixture.owner),
                    None,
                )
                .await;
                if audit
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|event| event["action"] == "mock.route")
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let expected = if change == "role" {
            let (status, _) = call(
                &fixture.other,
                "PATCH",
                &format!("{}/members/{}", fixture.base, fixture.member_id),
                Some(&fixture.owner),
                Some(json!({"role":"viewer","expected_revision":1})),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            StatusCode::NOT_FOUND
        } else {
            let (_, current) = call(
                &fixture.other,
                "GET",
                "/api/workspaces/w",
                Some(&fixture.owner),
                None,
            )
            .await;
            let (status, _) = call(&fixture.other, "PUT", "/api/workspaces/w", Some(&fixture.owner), Some(json!({"name":"Changed","data":current["data"],"expected_revision":current["revision"]}))).await;
            assert_eq!(status, StatusCode::OK);
            StatusCode::CONFLICT
        };
        let (status, result) = tokio::time::timeout(Duration::from_secs(3), pending)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(status, expected, "{change}: {result}");
        assert!(!result.to_string().contains("hello"));
    }
}

#[tokio::test]
async fn shared_routed_mock_reads_the_canonical_owner_dataset_with_incoming_parameters() {
    let fixture = fixture().await;
    let (_, mut saved) = call(
        &fixture.router,
        "GET",
        &fixture.base,
        Some(&fixture.editor),
        None,
    )
    .await;
    saved["data"]["datasets"] = json!([{"id":"rows","name":"Rows","secret":true,"source":{"format":"json","source":"[{\"id\":\"123\",\"value\":\"owner-row\"},{\"id\":\"456\",\"value\":\"other-row\"}]"},"views":[{"id":"by-id","name":"By ID","sql":"SELECT value FROM data WHERE id = ?"}]}]);
    let example = &mut saved["data"]["collections"][0]["requests"][0]["examples"][1];
    example["mock_script"] = json!("");
    example["mock_data"] = json!({"dataset_id":"rows","view_id":"by-id","shape":"first","parameters":[{"source":"path","name":"id"}]});
    let (status, _) = call(&fixture.router, "PUT", &fixture.base, Some(&fixture.editor), Some(json!({"name":saved["name"],"data":saved["data"],"expected_revision":saved["revision"]}))).await;
    assert_eq!(status, StatusCode::OK);
    let path = format!("{}/mocks/items/123?mode=fast&mode=fast", fixture.base);
    let (status, response) = invoke(&fixture.router, &path, &fixture.editor, "POST", "hello").await;
    assert_eq!(status, StatusCode::ACCEPTED, "{response}");
    assert_eq!(response, json!({"value":"owner-row"}));
    let (status, response) = invoke(
        &fixture.router,
        &format!("{}/mocks/items/456?mode=fast&mode=fast", fixture.base),
        &fixture.editor,
        "POST",
        "hello",
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{response}");
    assert_eq!(response, json!({"value":"other-row"}));
    let (status, response) = invoke(&fixture.router, &path, &fixture.viewer, "POST", "hello").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(!response.to_string().contains("owner-row"));
    let (status, response) = invoke(
        &fixture.router,
        "/api/mocks/w/items/123?mode=fast&mode=fast",
        &fixture.editor,
        "POST",
        "hello",
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(!response.to_string().contains("owner-row"));
}

#[tokio::test]
async fn shared_linked_schema_validation_is_revision_scoped_and_actor_audited() {
    let fixture = fixture().await;
    let (_, mut saved) = call(
        &fixture.router,
        "GET",
        &fixture.base,
        Some(&fixture.editor),
        None,
    )
    .await;
    saved["data"]["collections"][0]["requests"][0]["specification_id"] = json!("spec");
    let source = json!({"openapi":"3.1.0","paths":{"/items":{"post":{"responses":{"201":{"content":{"application/json":{"schema":{"type":"object","properties":{"value":{"const":"schema-private"}},"required":["value"]}}}},"422":{"content":{"application/json":{"schema":{"type":"string","pattern":".*"}}}}}},"get":{"responses":{"200":{"content":{"application/json":{"schema":{"type":"integer"}}}}}}}}}).to_string();
    saved["data"]["specifications"] =
        json!([{"id":"spec","name":"Spec","kind":"openapi","dialect":"3.1.0","source":source}]);
    let (status, saved) = call(&fixture.router, "PUT", &fixture.base, Some(&fixture.editor), Some(json!({"name":saved["name"],"data":saved["data"],"expected_revision":saved["revision"]}))).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let endpoint = format!("{}/mock-schema/validate", fixture.base);
    let mut input = json!({"request_id":"r","specification_id":"spec","pointer":"#/paths/~1items/post/responses/201/content/application~1json/schema","expected_revision":saved["revision"]});
    for (token, expected) in [
        (&fixture.viewer, StatusCode::FORBIDDEN),
        (&fixture.foreign, StatusCode::NOT_FOUND),
    ] {
        assert_eq!(
            call(
                &fixture.router,
                "POST",
                &endpoint,
                Some(token),
                Some(input.clone())
            )
            .await
            .0,
            expected
        );
    }
    assert_eq!(
        call(
            &fixture.router,
            "POST",
            "/api/workspaces/w/mock-schema/validate",
            Some(&fixture.editor),
            Some(input.clone())
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, result) = call(
        &fixture.router,
        "POST",
        &endpoint,
        Some(&fixture.editor),
        Some(input.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(
        result,
        json!({"method":"POST","status":201,"media_type":"application/json"})
    );
    input["expected_revision"] = json!(0);
    assert_eq!(
        call(
            &fixture.router,
            "POST",
            &endpoint,
            Some(&fixture.editor),
            Some(input.clone())
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    input["expected_revision"] = saved["revision"].clone();
    for pointer in [
        "#/paths/~1items/post/responses/422/content/application~1json/schema",
        "#/paths/~1items/get/responses/200/content/application~1json/schema",
    ] {
        input["pointer"] = json!(pointer);
        assert_eq!(
            call(
                &fixture.router,
                "POST",
                &endpoint,
                Some(&fixture.editor),
                Some(input.clone())
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
    input["specification_id"] = json!("other-spec");
    assert_eq!(
        call(
            &fixture.router,
            "POST",
            &endpoint,
            Some(&fixture.editor),
            Some(input)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (_, audit) = call(
        &fixture.router,
        "GET",
        &format!("{}/audit", fixture.base),
        Some(&fixture.owner),
        None,
    )
    .await;
    assert!(
        audit
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["action"] == "mock.schema.validate"
                && event["actor_id"] == fixture.member_id
                && event["result"] == "started")
    );
    assert!(!audit.to_string().contains("schema-private"));
    assert!(!audit.to_string().contains("#/paths"));
}
