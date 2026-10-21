mod common;
#[path = "fixtures/mcp/server.rs"]
mod fixture;
use common::*;
use std::time::Duration;

struct Shared {
    _dir: tempfile::TempDir,
    router: Router,
    owner: String,
    editor: String,
    other: String,
    viewer: String,
    foreign: String,
    base: String,
    saved: Value,
}
async fn setup(url: &str) -> Shared {
    let dir = tempfile::tempdir().unwrap();
    let mut settings = config(
        moleapi_server::sqlite_database_url(&dir.path().join("mcp.db")).unwrap(),
        true,
    );
    settings.allow_private_network = true;
    let router = hosted(settings).await.unwrap();
    let owner = register(&router, "mcpowner").await;
    let editor = register(&router, "mcpeditor").await;
    let other = register(&router, "mcpother").await;
    let viewer = register(&router, "mcpviewer").await;
    let foreign = register(&router, "mcpforeign").await;
    let mut data = example_data();
    data["script_mode"] = "v2".into();
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = url.into();
    request["examples"] = json!([]);
    request["protocol"] = json!({"kind":"mcp","transport":"http","name":"echo","operation":"tools/call","arguments_source":"{\"text\":\"saved draft\"}","config_source":"{\"url\":\"http://original.invalid/mcp\",\"vendor\":true,\"token\":\"fixture-token\"}"});
    request["pre_request_script"] =
        "pm.request.headers.add({key:'X-Mcp-Owner',value:'owner-script'});".into();
    request["auth"] =
        json!({"kind":"bearer","token":"{{fixture_token}}","username":"","password":""});
    request["timeout_ms"] = json!(60000);
    data["global_variables"] = json!([{"id":"v","key":"message","value":"owner-value","enabled":true},{"id":"s","key":"fixture_token","value":"fixture-token","enabled":true,"secret":true}]);
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"MCP","data":data})),
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
        other,
        viewer,
        foreign,
        base,
        saved,
    }
}
async fn create(shared: &Shared) -> (StatusCode, Value) {
    call(
        &shared.router,
        "POST",
        &format!("{}/sessions", shared.base),
        Some(&shared.editor),
        Some(json!({"request_id":"r","expected_revision":shared.saved["revision"]})),
    )
    .await
}
async fn until(shared: &Shared, path: &str, predicate: impl Fn(&Value) -> bool) -> Value {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let (status, value) =
                call(&shared.router, "GET", path, Some(&shared.editor), None).await;
            assert_eq!(status, StatusCode::OK, "{value}");
            if predicate(&value) {
                break value;
            }
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
    })
    .await
    .unwrap()
}
async fn send(shared: &Shared, path: &str, message: Value) {
    let (status, value) = call(
        &shared.router,
        "POST",
        &format!("{path}/send"),
        Some(&shared.editor),
        Some(message),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
}
async fn event(shared: &Shared, path: &str, predicate: impl Fn(&Value) -> bool) -> Value {
    let batch = until(shared, &format!("{path}/events"), |b| {
        b["events"].as_array().unwrap().iter().any(&predicate)
    })
    .await;
    batch["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| predicate(e))
        .unwrap()
        .clone()
}
#[tokio::test]
async fn member_mcp_admission_is_saved_revision_creator_bound_and_rejects_other_commands() {
    let shared = setup("http://127.0.0.1:1/mcp").await;
    let input = json!({"request_id":"r","expected_revision":shared.saved["revision"]});
    assert_eq!(
        call(
            &shared.router,
            "POST",
            &format!("{}/sessions", shared.base),
            Some(&shared.viewer),
            Some(input)
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &shared.router,
            "POST",
            &format!("{}/sessions", shared.base),
            Some(&shared.editor),
            Some(json!({"request_id":"r","expected_revision":0}))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(
            &shared.router,
            "POST",
            &format!("{}/sessions", shared.base),
            Some(&shared.foreign),
            Some(json!({"request_id":"r","expected_revision":shared.saved["revision"]}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, opened) = create(&shared).await;
    assert_eq!(status, StatusCode::OK, "{opened}");
    let path = format!(
        "{}/sessions/{}",
        shared.base,
        opened["id"].as_str().unwrap()
    );
    for token in [&shared.other, &shared.viewer, &shared.foreign] {
        assert_eq!(
            call(&shared.router, "GET", &path, Some(token), None)
                .await
                .0,
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        call(
            &shared.router,
            "POST",
            &format!("{path}/send"),
            Some(&shared.editor),
            Some(json!({"kind":"text","text":"not-mcp"}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &shared.router,
            "POST",
            &format!("{path}/close"),
            Some(&shared.editor),
            Some(json!({}))
        )
        .await
        .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn member_mcp_config_export_is_saved_scoped_and_redacted_by_default() {
    let shared = setup("http://127.0.0.1:1/mcp").await;
    let input = json!({"request_id":"r","expected_revision":shared.saved["revision"],"include_secrets":false});
    for (actor, status) in [
        (&shared.viewer, StatusCode::FORBIDDEN),
        (&shared.foreign, StatusCode::NOT_FOUND),
    ] {
        assert_eq!(
            call(
                &shared.router,
                "POST",
                &format!("{}/mcp/config/export", shared.base),
                Some(actor),
                Some(input.clone())
            )
            .await
            .0,
            status
        );
    }
    let (status, request) = call(
        &shared.router,
        "POST",
        &format!("{}/mcp/config/export", shared.base),
        Some(&shared.editor),
        Some(input),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{request}");
    assert_eq!(request["id"], "r");
    assert_eq!(request["auth"]["kind"], "none");
    assert!(!request.to_string().contains("fixture-token"));
    assert!(request.get("data").is_none());
    let (status,full)=call(&shared.router,"POST",&format!("{}/mcp/config/export",shared.base),Some(&shared.editor),Some(json!({"request_id":"r","expected_revision":shared.saved["revision"],"include_secrets":true}))).await;
    assert_eq!(status, StatusCode::OK, "{full}");
    assert_eq!(full, shared.saved["data"]["collections"][0]["requests"][0]);
}

async fn revise(shared: &mut Shared) {
    let(status,value)=call(&shared.router,"PUT",&shared.base,Some(&shared.editor),Some(json!({"name":shared.saved["name"],"data":shared.saved["data"],"expected_revision":shared.saved["revision"]}))).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    shared.saved = value;
}
fn tool(ticket: &str, name: &str, text: &str) -> Value {
    json!({"kind":"mcp_request","request_id":ticket,"method":"tools/call","name":name,"arguments_source":json!({"text":text}).to_string()})
}
async fn result(shared: &Shared, path: &str, ticket: &str) -> Value {
    event(shared, path, |e| {
        e["message"]["request_id"] == ticket
            && (e["message"]["kind"] == "mcp_result" || e["message"]["kind"] == "mcp_error")
    })
    .await["message"]
        .clone()
}
#[tokio::test]
async fn real_member_mcp_official_sdk_owner_capabilities_calls_callbacks_privacy_and_lifecycle() {
    use axum::{
        extract::Request,
        middleware::{self, Next},
        response::{IntoResponse, Response},
    };
    async fn gate(request: Request, next: Next) -> Response {
        if request
            .headers()
            .get("authorization")
            .and_then(|h| h.to_str().ok())
            != Some("Bearer fixture-token")
            || request
                .headers()
                .get("x-mcp-owner")
                .and_then(|h| h.to_str().ok())
                != Some("owner-script")
        {
            return StatusCode::UNAUTHORIZED.into_response();
        }
        next.run(request).await
    }
    let (url, server) = serve(fixture::router(false).layer(middleware::from_fn(gate))).await;
    let mut shared = setup(&format!("{url}/mcp")).await;
    let (status, summary) = create(&shared).await;
    assert_eq!(status, StatusCode::OK, "{summary}");
    let path = format!(
        "{}/sessions/{}",
        shared.base,
        summary["id"].as_str().unwrap()
    );
    until(&shared, &path, |s| s["state"] == "open").await;
    let caps = event(&shared, &path, |e| {
        e["message"]["kind"] == "mcp_capabilities"
    })
    .await;
    assert_eq!(caps["message"]["resources"][0]["uri"], "fixture://hello");
    assert_eq!(
        caps["message"]["resource_templates"][0]["uriTemplate"],
        "fixture://item/{id}"
    );
    assert_eq!(caps["message"]["prompts"][0]["name"], "greeting");
    let (_, initial) = call(
        &shared.router,
        "GET",
        &format!("{path}/events"),
        Some(&shared.editor),
        None,
    )
    .await;
    assert!(
        initial["events"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["message"]["kind"] != "mcp_result"
                || (e["message"]["method"] == "refresh"
                    && e["message"]["request_id"] == "capabilities"))
    );
    send(&shared, &path, tool("echo", "echo", "{{message}}")).await;
    assert_eq!(
        result(&shared, &path, "echo").await["result"]["structuredContent"]["text"],
        "owner-value"
    );
    send(&shared,&path,json!({"kind":"mcp_request","request_id":"invalid","method":"tools/call","name":"echo","arguments_source":"{\"text\":3}"})).await;
    assert_eq!(result(&shared, &path, "invalid").await["kind"], "mcp_error");
    for (ticket, method, extra) in [
        (
            "resource",
            "resources/read",
            json!({"uri":"file:///never-opened"}),
        ),
        (
            "prompt",
            "prompts/get",
            json!({"name":"greeting","arguments_source":"{\"who\":\"Original\"}"}),
        ),
        (
            "subscribe",
            "resources/subscribe",
            json!({"uri":"fixture://hello"}),
        ),
        (
            "unsubscribe",
            "resources/unsubscribe",
            json!({"uri":"fixture://hello"}),
        ),
        ("refresh", "refresh", json!({})),
    ] {
        let mut command = json!({"kind":"mcp_request","request_id":ticket,"method":method});
        command
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        send(&shared, &path, command).await;
        assert_eq!(result(&shared, &path, ticket).await["kind"], "mcp_result");
    }
    for name in ["progress", "change", "fail", "protocol_error", "content"] {
        send(&shared, &path, tool(name, name, "safe value")).await;
        let value = result(&shared, &path, name).await;
        if name == "protocol_error" {
            assert_eq!(value["kind"], "mcp_error");
        } else {
            assert_eq!(value["kind"], "mcp_result");
        }
    }
    event(&shared, &path, |e| {
        e["message"]["kind"] == "mcp_notification"
    })
    .await;
    for (name, method, answer) in [
        (
            "sampling",
            "sampling/createMessage",
            json!({"role":"assistant","content":{"type":"text","text":"Manual response"},"model":"manual"}),
        ),
        (
            "elicitation",
            "elicitation/create",
            json!({"action":"accept","content":{"answer":"yes"}}),
        ),
        (
            "roots",
            "roots/list",
            json!({"roots":[{"uri":"file:///manual-only","name":"Manual root"}]}),
        ),
    ] {
        send(&shared, &path, tool(name, name, "callback")).await;
        let callback = event(&shared, &path, |e| {
            e["message"]["kind"] == "mcp_callback" && e["message"]["method"] == method
        })
        .await;
        let reply = json!({"kind":"mcp_callback","callback_id":callback["message"]["callback_id"],"result":answer});
        assert_eq!(
            call(
                &shared.router,
                "POST",
                &format!("{path}/send"),
                Some(&shared.other),
                Some(reply.clone())
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
        send(&shared, &path, reply).await;
        assert_eq!(result(&shared, &path, name).await["kind"], "mcp_result");
    }
    send(&shared, &path, tool("slow", "slow", "cancel me")).await;
    send(
        &shared,
        &path,
        json!({"kind":"mcp_cancel","request_id":"slow"}),
    )
    .await;
    assert_eq!(result(&shared, &path, "slow").await["kind"], "mcp_error");
    send(
        &shared,
        &path,
        tool("private", "private_reflection", "{{fixture_token}}"),
    )
    .await;
    assert!(
        !result(&shared, &path, "private")
            .await
            .to_string()
            .contains("fixture-token")
    );
    let (_, canonical) = call(
        &shared.router,
        "GET",
        "/api/workspaces/w",
        Some(&shared.owner),
        None,
    )
    .await;
    assert_eq!(canonical["data"], shared.saved["data"]);
    assert_eq!(canonical["revision"], shared.saved["revision"]);
    let (_, actor) = call(
        &shared.router,
        "GET",
        "/api/auth/me",
        Some(&shared.editor),
        None,
    )
    .await;
    let (_, audit) = call(
        &shared.router,
        "GET",
        &format!("{}/audit", shared.base),
        Some(&shared.owner),
        None,
    )
    .await;
    for action in [
        "protocol.mcp.request",
        "protocol.mcp.callback",
        "protocol.mcp.cancel",
    ] {
        assert!(
            audit
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["action"] == action && e["actor_id"] == actor["id"])
        );
    }
    for content in [
        "owner-value",
        "fixture-token",
        "Manual response",
        "callback_id",
        "file:///never-opened",
    ] {
        assert!(!audit.to_string().contains(content), "{audit}");
    }
    revise(&mut shared).await;
    assert!(matches!(
        call(&shared.router, "GET", &path, Some(&shared.editor), None)
            .await
            .0,
        StatusCode::CONFLICT | StatusCode::NOT_FOUND
    ));
    let (_, next) = create(&shared).await;
    let next = format!("{}/sessions/{}", shared.base, next["id"].as_str().unwrap());
    until(&shared, &next, |s| s["state"] == "open").await;
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
    assert_eq!(
        call(&shared.router, "GET", &next, Some(&shared.editor), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    server.abort();
}
#[tokio::test]
async fn member_mcp_stdio_defaults_deny_and_host_names_use_the_same_canonical_privacy_pipeline() {
    let mut shared = setup("http://127.0.0.1:1/mcp").await;
    let request = &mut shared.saved["data"]["collections"][0]["requests"][0];
    request["url"] = "".into();
    request["pre_request_script"] = "".into();
    request["auth"] = json!({"kind":"none","token":"","username":"","password":""});
    request["protocol"] = json!({"kind":"mcp","transport":"stdio","command":"/bin/echo","args":[]});
    revise(&mut shared).await;
    let (status, value) = create(&shared).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{value}");
    assert!(
        value["error"].as_str().unwrap().contains("administrator"),
        "{value}"
    );
    for (url, token) in [
        (format!("{}/mcp/config/export", shared.base), &shared.editor),
        ("/api/workspaces/w/mcp/config/export".into(), &shared.owner),
    ] {
        let mut input =
            json!({"request_id":"r","include_secrets":false,"host_name":"service/fixture-token"});
        if token == &shared.editor {
            input["expected_revision"] = shared.saved["revision"].clone();
        }
        let (status, redacted) = call(
            &shared.router,
            "POST",
            &url,
            Some(token),
            Some(input.clone()),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{redacted}");
        assert!(!redacted.to_string().contains("fixture-token"));
        input["include_secrets"] = true.into();
        let (_, full) = call(&shared.router, "POST", &url, Some(token), Some(input)).await;
        assert_eq!(full["name"], "service/fixture-token");
    }
}
#[tokio::test]
#[ignore = "requires built official mcp_fixture and its exact administrator allowlist path"]
async fn member_allowed_stdio_uses_owner_environment_and_reaps_after_revision_loss() {
    let executable = std::env::var("MOLEAPI_MCP_STDIO_FIXTURE").expect("build official fixture");
    let mut shared = setup("").await;
    let request = &mut shared.saved["data"]["collections"][0]["requests"][0];
    request["pre_request_script"] = "".into();
    request["auth"] = json!({"kind":"none","token":"","username":"","password":""});
    request["protocol"] = json!({"kind":"mcp","transport":"stdio","command":"{{executable}}","args":["--stdio"],"env":[{"id":"env","key":"MCP_FIXTURE_VALUE","value":"{{message}}","enabled":true}]});
    shared.saved["data"]["global_variables"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"exe","key":"executable","value":executable,"enabled":true}));
    revise(&mut shared).await;
    let (status, opened) = create(&shared).await;
    assert_eq!(status, StatusCode::OK, "{opened}");
    let path = format!(
        "{}/sessions/{}",
        shared.base,
        opened["id"].as_str().unwrap()
    );
    event(&shared, &path, |e| {
        e["message"]["kind"] == "mcp_capabilities"
    })
    .await;
    send(&shared, &path, tool("environment", "environment", "safe")).await;
    let result = result(&shared, &path, "environment").await;
    let env = &result["result"]["structuredContent"];
    assert_eq!(env["explicit"], "owner-value");
    assert!(env["ambient"].is_null());
    assert!(env["home"].is_null());
    let pid = env["pid"].as_u64().unwrap();
    revise(&mut shared).await;
    #[cfg(target_os = "linux")]
    tokio::time::timeout(Duration::from_secs(2), async {
        while std::path::Path::new(&format!("/proc/{pid}")).exists() {
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
    })
    .await
    .expect("member STDIO child reaped after workspace revision changed");
}
