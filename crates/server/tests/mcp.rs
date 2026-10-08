mod common;
#[path = "fixtures/mcp/server.rs"]
mod fixture;
use common::*;
use std::time::Duration;
fn mcp_data(url: &str) -> Value {
    let mut data = example_data();
    let request = &mut data["collections"][0]["requests"][0];
    request["protocol"] = json!({"kind":"mcp","transport":"http","arguments_source":"{unfinished","config_source":"{\"vendor\":true}"});
    request["url"] = json!(url);
    request["timeout_ms"] = json!(5000);
    request["examples"] = json!([]);
    data
}
async fn save(router: &Router, data: Value, token: Option<&str>) -> Value {
    let (status, value) = call(
        router,
        "POST",
        "/api/workspaces",
        token,
        Some(json!({"id":"w","name":"MCP","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    value
}
async fn connect(router: &Router, w: &Value, token: Option<&str>) -> Value {
    let (status, value) = call(
        router,
        "POST",
        "/api/sessions",
        token,
        Some(json!({"workspace_id":"w","request":w["data"]["collections"][0]["requests"][0]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    value
}
async fn event(router: &Router, id: &str, predicate: impl Fn(&Value) -> bool) -> Value {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let (status, batch) = call(
                router,
                "GET",
                &format!("/api/sessions/{id}/events?after=0"),
                None,
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{batch}");
            if let Some(event) = batch["events"]
                .as_array()
                .unwrap()
                .iter()
                .find(|event| predicate(&event["message"]))
            {
                return event["message"].clone();
            }
            let (_, state) = call(router, "GET", &format!("/api/sessions/{id}"), None, None).await;
            assert_ne!(state["state"], "error", "{state}; {batch}");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("MCP event missing")
}
async fn send(router: &Router, id: &str, message: Value) {
    let (status, value) = call(
        router,
        "POST",
        &format!("/api/sessions/{id}/send"),
        None,
        Some(message),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
}
fn tool(id: &str, name: &str) -> Value {
    json!({"kind":"mcp_request","request_id":id,"method":"tools/call","name":name,"arguments_source":"{\"text\":\"fixture-input\"}"})
}
async fn setup() -> (
    tempfile::TempDir,
    Router,
    String,
    tokio::task::JoinHandle<()>,
) {
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("mcp.db")).await.unwrap();
    let (url, server) = serve(fixture::router(false)).await;
    let w = save(&router, mcp_data(&format!("{url}/mcp")), None).await;
    let session = connect(&router, &w, None).await;
    let id = session["id"].as_str().unwrap().to_string();
    event(&router, &id, |e| e["kind"] == "mcp_capabilities").await;
    (temp, router, id, server)
}
#[tokio::test]
async fn official_sdk_http_discovery_calls_resources_templates_prompts_errors_and_stop() {
    let (_temp, router, id, server) = setup().await;
    let caps = event(&router, &id, |e| e["kind"] == "mcp_capabilities").await;
    assert_eq!(caps["resources"][0]["uri"], "fixture://hello");
    assert_eq!(
        caps["resource_templates"][0]["uriTemplate"],
        "fixture://item/{id}"
    );
    assert_eq!(caps["prompts"][0]["name"], "greeting");
    let initialized = event(&router, &id, |e| e["kind"] == "mcp_initialized").await;
    assert_eq!(initialized["info"]["protocolVersion"], "2025-11-25");
    send(&router, &id, tool("echo", "echo")).await;
    let result = event(&router, &id, |e| e["request_id"] == "echo").await;
    assert_eq!(
        result["result"]["structuredContent"]["text"],
        "fixture-input"
    );
    send(&router, &id, tool("fail", "fail")).await;
    let result = event(&router, &id, |e| e["request_id"] == "fail").await;
    assert_eq!(result["result"]["isError"], true);
    send(&router, &id, tool("rpc", "protocol_error")).await;
    let result = event(&router, &id, |e| e["request_id"] == "rpc").await;
    assert_eq!(result["error"]["code"], -32602);
    assert_eq!(result["error"]["data"]["detail"], "kept");
    send(&router,&id,json!({"kind":"mcp_request","request_id":"invalid","method":"tools/call","name":"echo","arguments_source":"{\"text\":3}"})).await;
    assert_eq!(
        event(&router, &id, |e| e["request_id"] == "invalid").await["kind"],
        "mcp_error"
    );
    send(&router,&id,json!({"kind":"mcp_request","request_id":"resource","method":"resources/read","uri":"file:///must-not-be-opened"})).await;
    let result = event(&router, &id, |e| e["request_id"] == "resource").await;
    assert_eq!(
        result["result"]["contents"][0]["text"],
        "SDK resource content"
    );
    send(&router,&id,json!({"kind":"mcp_request","request_id":"prompt","method":"prompts/get","name":"greeting","arguments_source":"{\"who\":\"Alice\"}"})).await;
    assert_eq!(
        event(&router, &id, |e| e["request_id"] == "prompt").await["result"]["messages"][0]["content"]
            ["text"],
        "Hello Alice"
    );
    send(&router,&id,json!({"kind":"mcp_request","request_id":"subscribe","method":"resources/subscribe","uri":"fixture://hello"})).await;
    event(&router, &id, |e| {
        e["method"] == "notifications/resources/updated"
    })
    .await;
    send(&router, &id, tool("progress", "progress")).await;
    event(&router, &id, |e| e["method"] == "notifications/progress").await;
    send(&router, &id, tool("change", "change")).await;
    event(&router, &id, |e| {
        e["method"] == "notifications/tools/list_changed"
    })
    .await;
    send(&router, &id, tool("slow", "slow")).await;
    send(
        &router,
        &id,
        json!({"kind":"mcp_cancel","request_id":"slow"}),
    )
    .await;
    assert_eq!(
        event(&router, &id, |e| e["request_id"] == "slow").await["kind"],
        "mcp_error"
    );
    let (status, summary) = call(
        &router,
        "POST",
        &format!("/api/sessions/{id}/close"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(summary["state"], "closed");
    server.abort();
}
#[tokio::test]
async fn official_sdk_manual_sampling_elicitation_roots_accept_deny_and_cancel() {
    let (_temp, router, id, server) = setup().await;
    for (name, answer) in [
        (
            "sampling",
            json!({"role":"assistant","content":{"type":"text","text":"Manual response"},"model":"manual"}),
        ),
        (
            "elicitation",
            json!({"action":"accept","content":{"answer":"yes"}}),
        ),
        (
            "roots",
            json!({"roots":[{"uri":"file:///explicit","name":"Manual root"}]}),
        ),
    ] {
        send(&router, &id, tool(name, name)).await;
        let callback = event(&router, &id, |e| {
            e["kind"] == "mcp_callback"
                && e["method"]
                    == if name == "sampling" {
                        "sampling/createMessage"
                    } else if name == "roots" {
                        "roots/list"
                    } else {
                        "elicitation/create"
                    }
        })
        .await;
        send(
            &router,
            &id,
            json!({"kind":"mcp_callback","callback_id":callback["callback_id"],"result":answer}),
        )
        .await;
        assert_eq!(
            event(&router, &id, |e| e["request_id"] == name).await["kind"],
            "mcp_result"
        );
    }
    let (_, batch) = call(
        &router,
        "GET",
        &format!("/api/sessions/{id}/events?after=0"),
        None,
        None,
    )
    .await;
    let old_ids = batch["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|event| event["message"]["callback_id"].as_str().map(str::to_owned))
        .collect::<std::collections::HashSet<_>>();
    send(&router, &id, tool("deny", "sampling")).await;
    let newest = event(&router, &id, |event| {
        event["kind"] == "mcp_callback"
            && event["callback_id"]
                .as_str()
                .is_some_and(|id| !old_ids.contains(id))
    })
    .await;
    send(&router,&id,json!({"kind":"mcp_callback","callback_id":newest["callback_id"],"error":{"code":-32000,"message":"User denied"}})).await;
    event(&router, &id, |e| e["request_id"] == "deny").await;
    call(
        &router,
        "POST",
        &format!("/api/sessions/{id}/close"),
        None,
        None,
    )
    .await;
    server.abort();
}
#[tokio::test]
async fn mcp_drafts_owner_policy_hosted_stdio_denial_and_cancellation() {
    let temp = tempfile::tempdir().unwrap();
    let router = hosted(config(
        format!(
            "sqlite://{}?mode=rwc",
            temp.path().join("hosted.db").display()
        ),
        true,
    ))
    .await
    .unwrap();
    let owner = register(&router, "mcpowner").await;
    let other = register(&router, "mcpother").await;
    let mut data = mcp_data("");
    data["collections"][0]["requests"][0]["protocol"]["transport"] = json!("stdio");
    let w = save(&router, data, Some(&owner)).await;
    assert_eq!(
        w["data"]["collections"][0]["requests"][0]["protocol"]["arguments_source"],
        "{unfinished"
    );
    let mut request = w["data"]["collections"][0]["requests"][0].clone();
    request["protocol"]["command"] = json!("/bin/echo");
    let (status, result) = call(
        &router,
        "POST",
        "/api/sessions",
        Some(&owner),
        Some(json!({"workspace_id":"w","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{result}");
    assert!(result["error"].as_str().unwrap().contains("administrator"));
    let mut request = w["data"]["collections"][0]["requests"][0].clone();
    request["protocol"]["transport"] = json!("http");
    request["url"] = json!("http://127.0.0.1:18902/mcp");
    let (status, session) = call(
        &router,
        "POST",
        "/api/sessions",
        Some(&owner),
        Some(json!({"workspace_id":"w","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{session}");
    let id = session["id"].as_str().unwrap();
    let (status, _) = call(
        &router,
        "GET",
        &format!("/api/sessions/{id}/events"),
        Some(&other),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let blocked = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let (_, summary) = call(
                &router,
                "GET",
                &format!("/api/sessions/{id}"),
                Some(&owner),
                None,
            )
            .await;
            if summary["state"] == "error" {
                break summary;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(
        blocked["reason"].as_str().unwrap().contains("blocked"),
        "{blocked}"
    );
    let (status, summary) = call(
        &router,
        "POST",
        &format!("/api/sessions/{id}/close"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(summary["state"] == "closed" || summary["state"] == "error");
}
#[tokio::test]
async fn official_sdk_pagination_oversize_and_malformed_commands_are_bounded() {
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("mcp.db")).await.unwrap();
    let (url, server) = serve(fixture::router(true)).await;
    let w = save(&router, mcp_data(&format!("{url}/mcp")), None).await;
    let session = connect(&router, &w, None).await;
    let id = session["id"].as_str().unwrap();
    let result = event(&router, id, |e| {
        e["kind"] == "mcp_error" && e["method"] == "refresh"
    })
    .await;
    assert!(
        result["error"]["message"]
            .as_str()
            .unwrap()
            .contains("pagination")
    );
    let(status,_)=call(&router,"POST",&format!("/api/sessions/{id}/send"),None,Some(json!({"kind":"mcp_request","request_id":"bad","method":"tools/call","arguments_source":"[1]"}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    call(
        &router,
        "POST",
        &format!("/api/sessions/{id}/close"),
        None,
        None,
    )
    .await;
    server.abort();
    let (_temp, router, id, server) = setup().await;
    send(&router, &id, tool("oversize", "oversize")).await;
    let result = event(&router, &id, |e| e["request_id"] == "oversize").await;
    assert_eq!(result["kind"], "mcp_error");
    call(
        &router,
        "POST",
        &format!("/api/sessions/{id}/close"),
        None,
        None,
    )
    .await;
    server.abort();
}
#[tokio::test]
#[ignore = "build official fixture with cargo build -p moleapi-server --example mcp_fixture"]
async fn official_sdk_stdio_explicit_environment_and_reaping() {
    let executable = std::env::var("MOLEAPI_MCP_STDIO_FIXTURE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::current_exe()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("examples/mcp_fixture")
        });
    assert!(
        executable.exists(),
        "Build fixture first: {}",
        executable.display()
    );
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("stdio.db")).await.unwrap();
    let mut data = mcp_data("");
    data["collections"][0]["requests"][0]["protocol"] = json!({"kind":"mcp","transport":"stdio","command":"{{executable}}","args":["{{stdio_arg}}"],"env":[{"id":"e","key":"MCP_FIXTURE_VALUE","value":"{{declared}}","enabled":true}],"name":"{{name}}","uri":"{{uri}}"});
    data["environments"] = json!([{"id":"selected","name":"Selected","variables":[{"id":"exe","key":"executable","value":executable,"enabled":true},{"id":"arg","key":"stdio_arg","value":"--stdio","enabled":true},{"id":"value","key":"declared","value":"declared","enabled":true},{"id":"name","key":"name","value":"environment","enabled":true},{"id":"uri","key":"uri","value":"fixture://hello","enabled":true}]}]);
    data["active_environment_id"] = json!("selected");
    let w = save(&router, data, None).await;
    let session = connect(&router, &w, None).await;
    let id = session["id"].as_str().unwrap();
    assert_eq!(session["url"], "MCP STDIO");
    assert_eq!(
        w["data"]["collections"][0]["requests"][0]["protocol"]["command"],
        "{{executable}}"
    );
    assert_eq!(
        w["data"]["collections"][0]["requests"][0]["protocol"]["args"][0],
        "{{stdio_arg}}"
    );
    event(&router, id, |e| e["kind"] == "mcp_capabilities").await;
    send(&router, id, tool("environment", "environment")).await;
    let result = event(&router, id, |e| e["request_id"] == "environment").await;
    assert_eq!(
        result["result"]["structuredContent"]["explicit"],
        "declared"
    );
    assert!(result["result"]["structuredContent"]["ambient"].is_null());
    assert!(result["result"]["structuredContent"]["home"].is_null());
    let pid = result["result"]["structuredContent"]["pid"]
        .as_u64()
        .unwrap();
    let (status, summary) = call(
        &router,
        "POST",
        &format!("/api/sessions/{id}/close"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(summary["state"], "closed");
    #[cfg(target_os = "linux")]
    assert!(
        !std::path::Path::new(&format!("/proc/{pid}")).exists(),
        "STDIO child must be reaped before close returns"
    );
}

#[tokio::test]
async fn official_sdk_auth_headers_scoped_arguments_and_private_values_stay_transient() {
    use axum::{
        http::Request,
        middleware::{self, Next},
        response::Response,
    };
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    for (auth, expected) in [
        (
            json!({"kind":"bearer","token":"{{credential}}","username":"","password":""}),
            "Bearer synthetic-credential",
        ),
        (
            json!({"kind":"basic","token":"","username":"fixture","password":"{{credential}}"}),
            "Basic Zml4dHVyZTpzeW50aGV0aWMtY3JlZGVudGlhbA==",
        ),
    ] {
        let count = Arc::new(AtomicUsize::new(0));
        let captured = count.clone();
        let app = fixture::router(false).layer(middleware::from_fn(
            move |request: Request<Body>, next: Next| {
                let captured = captured.clone();
                async move {
                    if request
                        .headers()
                        .get("authorization")
                        .and_then(|h| h.to_str().ok())
                        != Some(expected)
                        || request
                            .headers()
                            .get("x-fixture")
                            .and_then(|h| h.to_str().ok())
                            != Some("scoped-header")
                    {
                        return Response::builder().status(401).body(Body::empty()).unwrap();
                    }
                    captured.fetch_add(1, Ordering::Relaxed);
                    next.run(request).await
                }
            },
        ));
        let (url, server) = serve(app).await;
        let temp = tempfile::tempdir().unwrap();
        let router = local(&temp.path().join("auth.db")).await.unwrap();
        let mut data = mcp_data(&format!("{url}/mcp"));
        data["global_variables"] =
            json!([{"id":"v","key":"credential","value":"wrong-global","enabled":true}]);
        data["environments"] = json!([{"id":"chosen","name":"Chosen","variables":[{"id":"v","key":"credential","value":"synthetic-credential","enabled":true,"secret":true},{"id":"h","key":"header","value":"scoped-header","enabled":true},{"id":"value","key":"text","value":"private-argument","enabled":true,"secret":true}]}]);
        let request = &mut data["collections"][0]["requests"][0];
        request["auth"] = auth;
        request["headers"] =
            json!([{"id":"h","key":"X-Fixture","value":"{{header}}","enabled":true}]);
        let w = save(&router, data, None).await;
        let(status,session)=call(&router,"POST","/api/sessions",None,Some(json!({"workspace_id":"w","environment_id":"chosen","request":w["data"]["collections"][0]["requests"][0]}))).await;
        assert_eq!(status, StatusCode::OK, "{session}");
        let id = session["id"].as_str().unwrap();
        event(&router, id, |e| e["kind"] == "mcp_capabilities").await;
        send(&router,id,json!({"kind":"mcp_request","request_id":"private","method":"tools/call","name":"echo","arguments_source":"{\"text\":\"{{text}}\"}"})).await;
        let result = event(&router, id, |e| e["request_id"] == "private").await;
        assert_eq!(result["result"]["structuredContent"]["text"], "[REDACTED]");
        assert!(count.load(Ordering::Relaxed) > 3);
        let (_, saved) = call(&router, "GET", "/api/workspaces/w", None, None).await;
        assert_eq!(
            saved["data"]["collections"][0]["requests"][0]["auth"]["password"],
            w["data"]["collections"][0]["requests"][0]["auth"]["password"]
        );
        assert_eq!(
            saved["data"]["collections"][0]["requests"][0]["protocol"]["arguments_source"],
            "{unfinished"
        );
        call(
            &router,
            "POST",
            &format!("/api/sessions/{id}/close"),
            None,
            None,
        )
        .await;
        server.abort();
    }
}
async fn state(router: &Router, id: &str, target: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let (_, value) = call(router, "GET", &format!("/api/sessions/{id}"), None, None).await;
            if value["state"] == target {
                return value;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}
#[tokio::test]
async fn sdk_transport_rejects_redirects_malformed_and_cumulative_depth_limits() {
    use axum::routing::post;
    for app in [
        Router::new().route(
            "/mcp",
            post(|| async {
                axum::response::Redirect::temporary("http://127.0.0.1:18902/should-not-contact")
            }),
        ),
        Router::new().route(
            "/mcp",
            post(|| async { axum::Json(json!({"invalid":"not JSONRPC"})) }),
        ),
        Router::new().route(
            "/mcp",
            post(|| async { axum::Json(json!({"oversize":"x".repeat(2*1024*1024)})) }),
        ),
    ] {
        let (url, server) = serve(app).await;
        let temp = tempfile::tempdir().unwrap();
        let router = local(&temp.path().join("negative.db")).await.unwrap();
        let w = save(&router, mcp_data(&format!("{url}/mcp")), None).await;
        let session = connect(&router, &w, None).await;
        state(&router, session["id"].as_str().unwrap(), "error").await;
        server.abort();
    }
    let (_temp, router, id, server) = setup().await;
    let mut deep = json!(null);
    for _ in 0..35 {
        deep = json!({"nested":deep});
    }
    let(status,_)=call(&router,"POST",&format!("/api/sessions/{id}/send"),None,Some(json!({"kind":"mcp_request","request_id":"deep","method":"tools/call","name":"echo","arguments_source":deep.to_string()}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let(status,_)=call(&router,"POST",&format!("/api/sessions/{id}/send"),None,Some(json!({"kind":"mcp_callback","callback_id":"unknown","result":{},"error":{"code":-32000,"message":"both"}}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    call(
        &router,
        "POST",
        &format!("/api/sessions/{id}/close"),
        None,
        None,
    )
    .await;
    server.abort();
}
#[tokio::test]
async fn sdk_tls_verification_and_explicit_opt_out() {
    use std::sync::Arc;
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(
        vec![cert.cert.der().clone()],
        rustls::pki_types::PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der()).into(),
    )
    .unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "https://localhost:{}/mcp",
        listener.local_addr().unwrap().port()
    );
    let app = fixture::router(false);
    let server = tokio::spawn(async move {
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let acceptor = acceptor.clone();
            let app = app.clone();
            tokio::spawn(async move {
                if let Ok(stream) = acceptor.accept(stream).await {
                    let service = hyper_util::service::TowerToHyperService::new(app);
                    let _ = hyper_util::server::conn::auto::Builder::new(
                        hyper_util::rt::TokioExecutor::new(),
                    )
                    .serve_connection(hyper_util::rt::TokioIo::new(stream), service)
                    .await;
                }
            });
        }
    });
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("tls.db")).await.unwrap();
    let w = save(&router, mcp_data(&url), None).await;
    let session = connect(&router, &w, None).await;
    state(&router, session["id"].as_str().unwrap(), "error").await;
    let mut request = w["data"]["collections"][0]["requests"][0].clone();
    request["verify_tls"] = json!(false);
    let (status, session) = call(
        &router,
        "POST",
        "/api/sessions",
        None,
        Some(json!({"workspace_id":"w","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let id = session["id"].as_str().unwrap();
    event(&router, id, |e| e["kind"] == "mcp_capabilities").await;
    send(&router, id, tool("tls", "echo")).await;
    assert_eq!(
        event(&router, id, |e| e["request_id"] == "tls").await["kind"],
        "mcp_result"
    );
    call(
        &router,
        "POST",
        &format!("/api/sessions/{id}/close"),
        None,
        None,
    )
    .await;
    server.abort();
}

#[tokio::test]
async fn official_sdk_content_blocks_are_preserved_without_secondary_fetches() {
    let (_temp, router, id, server) = setup().await;
    send(&router, &id, tool("content", "content")).await;
    let result = event(&router, &id, |e| e["request_id"] == "content").await;
    let content = result["result"]["content"].as_array().unwrap();
    assert_eq!(
        content
            .iter()
            .map(|c| c["type"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["text", "image", "audio", "resource_link", "resource"]
    );
    assert_eq!(content[3]["uri"], "https://example.invalid/never-fetch");
    assert_eq!(content[4]["resource"]["text"], "{\"ok\":true}");
    call(
        &router,
        "POST",
        &format!("/api/sessions/{id}/close"),
        None,
        None,
    )
    .await;
    server.abort();
}
#[tokio::test]
#[ignore = "build official fixture with cargo build -p moleapi-server --example mcp_fixture"]
async fn stdio_oversize_raw_line_is_bounded_before_parse() {
    let executable = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/mcp_fixture");
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("oversize-stdio.db")).await.unwrap();
    let mut data = mcp_data("");
    data["collections"][0]["requests"][0]["protocol"] =
        json!({"kind":"mcp","transport":"stdio","command":executable,"args":["--oversize-stdio"]});
    data["collections"][0]["requests"][0]["timeout_ms"] = json!(1000);
    let w = save(&router, data, None).await;
    let session = connect(&router, &w, None).await;
    state(&router, session["id"].as_str().unwrap(), "error").await;
}

#[tokio::test]
async fn official_sdk_owner_logout_and_workspace_delete_end_live_sessions() {
    let (url, server) = serve(fixture::router(false)).await;
    let temp = tempfile::tempdir().unwrap();
    let mut config = config(
        format!(
            "sqlite://{}?mode=rwc",
            temp.path().join("lifecycle.db").display()
        ),
        true,
    );
    config.allow_private_network = true;
    let router = hosted(config).await.unwrap();
    let token = register(&router, "lifecycle-owner").await;
    let w = save(&router, mcp_data(&format!("{url}/mcp")), Some(&token)).await;
    let session = connect(&router, &w, Some(&token)).await;
    let id = session["id"].as_str().unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let (_, value) = call(
                &router,
                "GET",
                &format!("/api/sessions/{id}/events"),
                Some(&token),
                None,
            )
            .await;
            if value["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["message"]["kind"] == "mcp_capabilities")
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let (status, _) = call(
        &router,
        "POST",
        &format!("/api/sessions/{id}/send"),
        Some(&token),
        Some(tool("slow", "slow")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(&router, "POST", "/api/auth/logout", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(
        &router,
        "GET",
        &format!("/api/sessions/{id}/events"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, login) = call(
        &router,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({"username":"lifecycle-owner","password":"goodpassword123"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let new_token = login["token"].as_str().unwrap();
    let (status, old_session) = call(
        &router,
        "GET",
        &format!("/api/sessions/{id}"),
        Some(new_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(old_session["state"], "closed");
    let session = connect(&router, &w, Some(new_token)).await;
    let new_id = session["id"].as_str().unwrap();
    let (status, _) = call(
        &router,
        "DELETE",
        "/api/workspaces/w",
        Some(new_token),
        Some(json!({"expected_revision":w["revision"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(
        &router,
        "GET",
        &format!("/api/sessions/{new_id}/events"),
        Some(new_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    server.abort();
}

#[tokio::test]
async fn private_method_keys_and_scalar_reflections_are_withheld() {
    let (url, server) = serve(fixture::router(false)).await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("privacy.db")).await.unwrap();
    let mut data = mcp_data(&format!("{url}/mcp"));
    data["environments"] = json!([{"id":"privacy","name":"Privacy","variables":[{"id":"text","key":"private_text","value":"synthetic-sensitive-key","enabled":true,"secret":true},{"id":"number","key":"private_number","value":"173","enabled":true,"secret":true},{"id":"boolean","key":"private_boolean","value":"true","enabled":true,"secret":true}]}]);
    data["active_environment_id"] = json!("privacy");
    let w = save(&router, data, None).await;
    let session = connect(&router, &w, None).await;
    let id = session["id"].as_str().unwrap();
    event(&router, id, |e| e["kind"] == "mcp_capabilities").await;
    send(&router,id,json!({"kind":"mcp_request","request_id":"reflection","method":"tools/call","name":"private_reflection","arguments_source":"{\"text\":\"{{private_text}}\"}"})).await;
    let result = event(&router, id, |e| e["request_id"] == "reflection").await;
    let reflected = &result["result"]["structuredContent"];
    assert_eq!(reflected["[REDACTED]"]["number"], "[REDACTED]");
    assert_eq!(reflected["[REDACTED]"]["boolean"], "[REDACTED]");
    let notification = event(&router, id, |e| {
        e["kind"] == "mcp_notification" && e["method"] == "private/[REDACTED]"
    })
    .await;
    assert_eq!(notification["params"]["field"], "[REDACTED]");
    assert_eq!(notification["params"]["flag"], "[REDACTED]");
    let (_, batch) = call(
        &router,
        "GET",
        &format!("/api/sessions/{id}/events"),
        None,
        None,
    )
    .await;
    assert!(!batch.to_string().contains("synthetic-sensitive-key"));
    call(
        &router,
        "POST",
        &format!("/api/sessions/{id}/close"),
        None,
        None,
    )
    .await;
    server.abort();
}

#[tokio::test]
async fn mcp_http_sdk_applies_scoped_network_dns_settings_to_initialization_and_calls() {
    let (url, server) = serve(fixture::router(false)).await;
    let port = url::Url::parse(&url).unwrap().port().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("mcp-network.db")).await.unwrap();
    let mut data = mcp_data(&format!("http://mcp-network.test:{port}/mcp"));
    data["global_variables"] =
        json!([{"id":"host","key":"network_host","value":"mcp-network.test","enabled":true}]);
    data["collections"][0]["requests"][0]["network"] =
        json!({"dns":[{"hostname":"{{network_host}}","addresses":["127.0.0.1"]}]});
    let w = save(&router, data, None).await;
    let session = connect(&router, &w, None).await;
    let id = session["id"].as_str().unwrap();
    event(&router, id, |e| e["kind"] == "mcp_capabilities").await;
    send(&router, id, tool("network-echo", "echo")).await;
    let result = event(&router, id, |e| e["request_id"] == "network-echo").await;
    assert_eq!(
        result["result"]["structuredContent"]["text"], "fixture-input",
        "{result}"
    );
    call(
        &router,
        "DELETE",
        &format!("/api/sessions/{id}"),
        None,
        None,
    )
    .await;
    server.abort();
}
