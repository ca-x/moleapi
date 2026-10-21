mod common;
use common::*;
struct Shared {
    _dir: tempfile::TempDir,
    router: Router,
    owner: String,
    editor: String,
    viewer: String,
    foreign: String,
    base: String,
    saved: Value,
}
async fn setup(url: &str) -> Shared {
    setup_with(url).await
}
async fn setup_with(url: &str) -> Shared {
    let dir = tempfile::tempdir().unwrap();
    let mut settings = config(
        moleapi_server::sqlite_database_url(&dir.path().join("mock-preview.db")).unwrap(),
        true,
    );
    settings.allow_private_network = true;
    let router = hosted(settings).await.unwrap();
    let owner = register(&router, "routeowner").await;
    let editor = register(&router, "routeeditor").await;
    let other = register(&router, "routeother").await;
    let viewer = register(&router, "routeviewer").await;
    let foreign = register(&router, "routeforeign").await;
    let mut data = example_data();
    data["script_mode"] = "v2".into();
    data["collections"][0]["auth"] =
        json!({"kind":"bearer","token":"folder-private","username":"","password":""});
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = url.into();
    request["examples"] = json!([]);
    request["protocol"] = json!({"kind":"http"});
    request["pre_request_script"] = json!("");
    request["auth"] = json!({"kind":"none","token":"","username":"","password":""});
    request["examples"] =
        json!([{"id":"e","name":"Selected","status":201,"body":"selected","headers":[]}]);
    request["timeout_ms"] = json!(60000);
    data["global_variables"] = json!([{"id":"v","key":"message","value":"owner-value","enabled":true},{"id":"s","key":"fixture_token","value":"fixture-token","enabled":true,"secret":true}]);
    data["global_variables"].as_array_mut().unwrap().push(
        json!({"id":"n","key":"numeric_private","value":"987654","enabled":true,"secret":true}),
    );
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Dubbo","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let mut collision = data;
    collision["global_variables"][0]["value"] = "wrong-member".into();
    collision["global_variables"][1]["value"] = "wrong-token".into();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            Some(&editor),
            Some(json!({"id":"w","name":"Collision","data":collision}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let mut base = String::new();
    for (token, role) in [(&editor, "editor"), (&other, "editor"), (&viewer, "viewer")] {
        let (_, invite) = call(
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
            Some(json!({"token":invite["token"]})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{accepted}");
        base = format!(
            "/api/owners/{}/workspaces/w",
            accepted["owner_id"].as_str().unwrap()
        );
    }
    Shared {
        _dir: dir,
        router,
        owner,
        editor,
        viewer,
        foreign,
        base,
        saved,
    }
}
#[tokio::test]
async fn member_route_preview_is_canonical_revision_bound_and_wraps_real_mock_response() {
    let shared = setup("https://example.test/orders/{id}").await;
    let (status,value)=call(&shared.router,"POST",&format!("{}/mock/route-preview",shared.base),Some(&shared.editor),Some(json!({"expected_revision":shared.saved["revision"],"method":"GET","path":"/orders/7","query":[],"headers":[],"body":""}))).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(value["status"], 201);
    assert_eq!(value["body"], "selected");
}
async fn revise(shared: &mut Shared, data: Value) {
    let(status,value)=call(&shared.router,"PUT",&shared.base,Some(&shared.editor),Some(json!({"name":shared.saved["name"],"expected_revision":shared.saved["revision"],"data":data}))).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    shared.saved = value;
}
fn input(shared: &Shared) -> Value {
    json!({"expected_revision":shared.saved["revision"],"method":"GET","path":"/orders/7","query":[],"headers":[],"body":""})
}
async fn preview(shared: &Shared, input: Value) -> (StatusCode, Value) {
    call(
        &shared.router,
        "POST",
        &format!("{}/mock/route-preview", shared.base),
        Some(&shared.editor),
        Some(input),
    )
    .await
}
#[tokio::test]
async fn preview_request_url_matches_the_real_wildcard_script_snapshot() {
    let mut shared = setup("https://example.test/orders/{id}").await;
    let mut data = shared.saved["data"].clone();
    data["collections"][0]["requests"][0]["examples"][0]["mock_script"] =
        json!("pm.mock.setBody({url:pm.request.url});");
    revise(&mut shared, data).await;
    let mut incoming = input(&shared);
    incoming["path"] = json!("/orders/a%20b");
    incoming["query"] = json!([{"key":"filter","value":"a b"}]);
    let (status, value) = preview(&shared, incoming).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    let response = shared
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("{}/mocks/orders/a%20b?filter=a+b", shared.base))
                .header("authorization", format!("Bearer {}", shared.editor))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    assert_eq!(value["body"].as_str().unwrap().as_bytes(), body.as_ref());
}
#[tokio::test]
async fn preview_rejects_outer_access_overrides_and_budgets_without_mock_dispatch() {
    let shared = setup("https://example.test/orders/{id}").await;
    let path = format!("{}/mock/route-preview", shared.base);
    for (actor, expected) in [
        (&shared.viewer, StatusCode::FORBIDDEN),
        (&shared.foreign, StatusCode::NOT_FOUND),
    ] {
        assert_eq!(
            call(
                &shared.router,
                "POST",
                &path,
                Some(actor),
                Some(input(&shared))
            )
            .await
            .0,
            expected
        );
    }
    let mut stale = input(&shared);
    stale["expected_revision"] = json!(0);
    assert_eq!(preview(&shared, stale).await.0, StatusCode::CONFLICT);
    let mut bads = Vec::new();
    for (field, value) in [
        ("owner_id", json!("foreign")),
        ("locals", json!([])),
        ("request", json!({})),
        ("method", json!("TRACE")),
        ("path", json!("https://other.test/orders/7")),
        ("path", json!("//other.test/orders/7")),
        ("path", json!("/orders/7?mode=fast")),
        ("path", json!("/orders/7#fragment")),
        ("path", json!(format!("/{}", "x".repeat(4097)))),
        (
            "query",
            json!([{"key":"access_token","value":"not-transport"}]),
        ),
        (
            "headers",
            json!([{"key":"Authorization","value":"not-transport"}]),
        ),
        ("headers", json!([{"key":"Cookie","value":"not-transport"}])),
        (
            "headers",
            json!([{"key":"Connection","value":"not-transport"}]),
        ),
        ("headers", json!([{"key":"X-Match","value":"bad\r\nvalue"}])),
        ("body", json!("x".repeat(1024 * 1024 + 1))),
    ] {
        let mut v = input(&shared);
        v[field] = value;
        bads.push(v);
    }
    for incoming in bads {
        let (status, response) = preview(&shared, incoming).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{response}");
    }
    let (_, calls) = call(
        &shared.router,
        "GET",
        &format!("{}/mock-calls", shared.base),
        Some(&shared.editor),
        None,
    )
    .await;
    assert_eq!(calls, json!([]));
    let (status, _) = call(
        &shared.router,
        "GET",
        "/api/auth/me",
        Some(&shared.editor),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let mut local = input(&shared);
    local["path"] = json!("/api/auth/logout");
    let (status, response) = preview(&shared, local).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(response["status"], 404);
    assert_eq!(response["outcome"], "unmatched");
    assert_eq!(
        call(
            &shared.router,
            "GET",
            "/api/auth/me",
            Some(&shared.editor),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
}
#[tokio::test]
async fn preview_matches_real_rules_owner_script_and_keeps_control_credentials_out() {
    let mut shared = setup("https://example.test/orders/{id}").await;
    let mut data = shared.saved["data"].clone();
    data["collections"][0]["requests"][0]["method"] = json!("POST");
    data["collections"][0]["requests"][0]["examples"] = json!([
 {"id":"fallback","name":"Fallback","status":200,"headers":[],"body":"fallback"},
 {"id":"match","name":"Matched","status":202,"headers":[],"body":"seed","mock_match":{"priority":10,"query":[{"id":"q1","key":"mode","value":"fast","enabled":true},{"id":"q2","key":"mode","value":"fast","enabled":true}],"headers":[{"id":"h","key":"X-Mode","value":"fast","enabled":true}],"body":"hello"},"mock_script":"if(pm.request.headers.get('authorization')!==undefined||pm.request.headers.get('cookie')!==undefined||pm.request.headers.get('x-outer')!==undefined)throw new Error('outer credential leaked');pm.mock.setBody({owner:pm.variables.get('message'),method:pm.request.method,body:pm.request.body.raw});"},
 {"id":"unselected","name":"Unselected","status":203,"headers":[],"body":"must not run","mock_match":{"priority":99,"headers":[{"id":"h","key":"X-Mode","value":"never","enabled":true}]},"mock_script":"throw new Error('unselected script ran');"}]);
    revise(&mut shared, data).await;
    let mut incoming = input(&shared);
    incoming["method"] = json!("POST");
    incoming["query"] = json!([{"key":"mode","value":"fast"},{"key":"mode","value":"fast"}]);
    incoming["headers"] = json!([{"key":"x-MODE","value":"fast"}]);
    incoming["body"] = json!("hello");
    let response = shared
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("{}/mock/route-preview", shared.base))
                .header("authorization", format!("Bearer {}", shared.editor))
                .header("cookie", "outer-secret")
                .header("x-outer", "not-simulated")
                .header("content-type", "application/json")
                .body(Body::from(incoming.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 6 * 1024 * 1024)
        .await
        .unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["status"], 202, "{value}");
    assert_eq!(value["matched"]["request_id"], "r");
    assert_eq!(value["matched"]["example_id"], "match");
    assert_eq!(value["outcome"], "served");
    let body: Value = serde_json::from_str(value["body"].as_str().unwrap()).unwrap();
    assert_eq!(
        body,
        json!({"owner":"owner-value","method":"POST","body":"hello"})
    );
    incoming["query"] = json!([{"key":"mode","value":"fast"}]);
    let (status, fallback) = preview(&shared, incoming).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fallback["body"], "fallback");
    assert_eq!(fallback["matched"]["example_id"], "fallback");
    let (_, canonical) = call(
        &shared.router,
        "GET",
        "/api/workspaces/w",
        Some(&shared.owner),
        None,
    )
    .await;
    assert_eq!(canonical, shared.saved);
    let (_, audit) = call(
        &shared.router,
        "GET",
        &format!("{}/audit", shared.base),
        Some(&shared.owner),
        None,
    )
    .await;
    assert!(audit.to_string().contains("mock.route.preview"));
    for private in [
        "/orders/7",
        "not-simulated",
        "outer-secret",
        "hello",
        "owner-value",
    ] {
        assert!(
            !audit.to_string().contains(private),
            "Audit must contain only action metadata"
        );
    }
}
#[tokio::test]
async fn mock_error_status_headers_and_traffic_are_inert_envelope_data() {
    let mut shared = setup("https://example.test/orders/{id}").await;
    let mut data = shared.saved["data"].clone();
    data["collections"][0]["requests"][0]["examples"][0]["status"] = json!(401);
    data["collections"][0]["requests"][0]["examples"][0]["headers"] = json!([{"id":"cookie","key":"Set-Cookie","value":"mock-cookie=1","enabled":true},{"id":"location","key":"Location","value":"https://example.invalid/never-navigate","enabled":true}]);
    data["collections"][0]["requests"][0]["examples"][0]["body"] =
        json!("<script>never execute</script>");
    revise(&mut shared, data).await;
    let response = shared
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("{}/mock/route-preview", shared.base))
                .header("authorization", format!("Bearer {}", shared.editor))
                .header("content-type", "application/json")
                .body(Body::from(input(&shared).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response.headers().get("set-cookie").is_none()
            && response.headers().get("location").is_none()
    );
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["status"], 401);
    assert_eq!(value["body"], "<script>never execute</script>");
    assert!(
        value["headers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|h| h["key"] == "set-cookie")
    );
    assert_eq!(
        call(
            &shared.router,
            "GET",
            "/api/auth/me",
            Some(&shared.editor),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    let mut data = shared.saved["data"].clone();
    let example = &mut data["collections"][0]["requests"][0]["examples"][0];
    example["fault_rate_percent"] = json!(100);
    example["fault_status"] = json!(503);
    example["rate_limit_per_minute"] = json!(1);
    example["mock_script"] = json!("throw new Error('must not run under traffic simulation');");
    revise(&mut shared, data).await;
    let (_, fault) = preview(&shared, input(&shared)).await;
    assert_eq!(fault["status"], 503);
    assert_eq!(fault["outcome"], "simulated_failure");
    let (_, rate) = preview(&shared, input(&shared)).await;
    assert_eq!(rate["status"], 429);
    assert_eq!(rate["outcome"], "rate_limited");
    assert!(!rate["body"].as_str().unwrap().contains("must not run"));
    let (_, calls) = call(
        &shared.router,
        "GET",
        &format!("{}/mock-calls", shared.base),
        Some(&shared.editor),
        None,
    )
    .await;
    assert_eq!(calls[0]["outcome"], "rate_limited");
    assert!(!calls.to_string().contains("mock-cookie") && !calls.to_string().contains("<script>"));
}
#[tokio::test]
async fn route_preview_uses_parameterized_owner_dataset_and_never_static_fallback() {
    let mut shared = setup("https://example.test/orders/{id}").await;
    let mut data = shared.saved["data"].clone();
    data["datasets"] = json!([{"id":"rows","name":"Private rows","secret":true,"source":{"format":"json","source":"[{\"id\":\"7\",\"region\":\"west\",\"tier\":\"gold\",\"value\":\"owner-row\"}]"},"views":[{"id":"filtered","name":"Filtered","sql":"SELECT value FROM data WHERE id = ? AND region = ? AND tier = ?"}]}]);
    let example = &mut data["collections"][0]["requests"][0]["examples"][0];
    example["body"] = json!("do-not-fallback");
    example["mock_data"] = json!({"dataset_id":"rows","view_id":"filtered","shape":"first","parameters":[{"source":"path","name":"id"},{"source":"query","name":"region"},{"source":"header","name":"X-Tier"}]});
    revise(&mut shared, data).await;
    let mut incoming = input(&shared);
    incoming["query"] = json!([{"key":"region","value":"west"}]);
    incoming["headers"] = json!([{"key":"x-tier","value":"gold"}]);
    let (status, response) = preview(&shared, incoming.clone()).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], 201, "{response}");
    assert_eq!(
        serde_json::from_str::<Value>(response["body"].as_str().unwrap()).unwrap(),
        json!({"value":"owner-row"})
    );
    incoming["query"]
        .as_array_mut()
        .unwrap()
        .push(json!({"key":"region","value":"west"}));
    let (_, ambiguous) = preview(&shared, incoming.clone()).await;
    assert_eq!(ambiguous["status"], 400);
    assert!(
        !ambiguous["body"]
            .as_str()
            .unwrap()
            .contains("do-not-fallback")
    );
    incoming["query"] = json!([]);
    let (_, missing) = preview(&shared, incoming).await;
    assert_eq!(missing["status"], 400);
    assert!(
        !missing["body"]
            .as_str()
            .unwrap()
            .contains("do-not-fallback")
    );
    let (_, canonical) = call(
        &shared.router,
        "GET",
        "/api/workspaces/w",
        Some(&shared.owner),
        None,
    )
    .await;
    assert_eq!(canonical, shared.saved);
    let (_, logs) = call(
        &shared.router,
        "GET",
        &format!("{}/mock-calls", shared.base),
        Some(&shared.editor),
        None,
    )
    .await;
    assert!(
        !logs.to_string().contains("owner-row")
            && !logs.to_string().contains("west")
            && !logs.to_string().contains("gold")
    );
}
#[tokio::test]
async fn delayed_preview_and_existing_wildcard_are_fenced_by_role_revision_and_logout() {
    use std::time::Duration;
    for mode in ["preview", "wildcard"] {
        for boundary in ["role", "revision", "logout"] {
            let mut shared = setup("https://example.test/orders/{id}").await;
            let mut data = shared.saved["data"].clone();
            data["collections"][0]["requests"][0]["examples"][0]["delay_ms"] = json!(2000);
            revise(&mut shared, data).await;
            let router = shared.router.clone();
            let actor = shared.editor.clone();
            let path = format!("{}/mock/route-preview", shared.base);
            let uri = format!("{}/mocks/orders/7", shared.base);
            let body = input(&shared);
            let work = tokio::spawn(async move {
                if mode == "preview" {
                    call(&router, "POST", &path, Some(&actor), Some(body))
                        .await
                        .0
                } else {
                    router
                        .oneshot(
                            Request::builder()
                                .uri(uri)
                                .header("authorization", format!("Bearer {actor}"))
                                .body(Body::empty())
                                .unwrap(),
                        )
                        .await
                        .unwrap()
                        .status()
                }
            });
            tokio::time::sleep(Duration::from_millis(100)).await;
            assert!(!work.is_finished());
            let expected = match boundary {
                "revision" => {
                    let data = shared.saved["data"].clone();
                    revise(&mut shared, data).await;
                    StatusCode::CONFLICT
                }
                "role" => {
                    let (_, members) = call(
                        &shared.router,
                        "GET",
                        &format!("{}/members", shared.base),
                        Some(&shared.owner),
                        None,
                    )
                    .await;
                    let member = members
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|m| m["username"] == "routeeditor")
                        .unwrap();
                    assert_eq!(
                        call(
                            &shared.router,
                            "PATCH",
                            &format!(
                                "{}/members/{}",
                                shared.base,
                                member["member_id"].as_str().unwrap()
                            ),
                            Some(&shared.owner),
                            Some(json!({"role":"viewer","expected_revision":member["revision"]}))
                        )
                        .await
                        .0,
                        StatusCode::OK
                    );
                    StatusCode::NOT_FOUND
                }
                _ => {
                    assert_eq!(
                        call(
                            &shared.router,
                            "POST",
                            "/api/auth/logout",
                            Some(&shared.editor),
                            None
                        )
                        .await
                        .0,
                        StatusCode::OK
                    );
                    StatusCode::UNAUTHORIZED
                }
            };
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(3), work)
                    .await
                    .unwrap()
                    .unwrap(),
                expected,
                "{mode}/{boundary}"
            );
        }
    }
}

#[tokio::test]
async fn preview_preserves_dot_segments_like_raw_wildcard_without_selecting_another_route() {
    let shared = setup("https://example.test/7").await;
    for path in ["/orders/../7", "/orders/%2e%2e/7", "/orders/./7"] {
        let mut incoming = input(&shared);
        incoming["path"] = json!(path);
        let (status, value) = preview(&shared, incoming).await;
        assert_eq!(status, StatusCode::OK, "{value}");
        let response = shared
            .router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("{}/mocks{path}", shared.base))
                    .header("authorization", format!("Bearer {}", shared.editor))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(value["status"], response.status().as_u16(), "path {path}");
        assert_eq!(value["status"], 404, "must not select /7");
    }
}
