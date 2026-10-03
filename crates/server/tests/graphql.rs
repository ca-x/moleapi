mod common;
use async_graphql::{Object, Schema, Subscription};
use axum::{Json, routing::post};
use common::*;
use futures_util::{Stream, stream};
use std::time::Duration;
struct Query;
#[Object]
impl Query {
    async fn hello(&self, name: String) -> String {
        format!("Hello {name}")
    }
    async fn failure(&self) -> async_graphql::Result<String> {
        Err("fixture resolver error".into())
    }
}
struct Mutation;
#[Object]
impl Mutation {
    async fn double(&self, value: i32) -> i32 {
        value * 2
    }
}
struct Subscriptions;
#[Subscription]
impl Subscriptions {
    async fn ticks(&self) -> impl Stream<Item = i32> {
        stream::iter([1, 2, 3])
    }
    async fn waiting(&self) -> impl Stream<Item = i32> {
        stream::pending()
    }
}
async fn fixture() -> (String, tokio::task::JoinHandle<()>) {
    let schema = Schema::build(Query, Mutation, Subscriptions).finish();
    let http_schema = schema.clone();
    serve(
        Router::new()
            .route(
                "/graphql",
                post(move |Json(request): Json<async_graphql::Request>| {
                    let schema = http_schema.clone();
                    async move { Json(schema.execute(request).await) }
                }),
            )
            .route_service(
                "/subscriptions",
                async_graphql_axum::GraphQLSubscription::new(schema),
            )
            .route(
                "/auth",
                post(|| async {
                    (
                        StatusCode::UNAUTHORIZED,
                        Json(json!({"errors":[{"message":"Target authentication required"}]})),
                    )
                }),
            ),
    )
    .await
}
fn graphql_data(url: &str, document: &str) -> Value {
    let mut d = example_data();
    let r = &mut d["collections"][0]["requests"][0];
    r["examples"] = json!([]);
    r["url"] = json!(url);
    r["protocol"] = json!({"kind":"graphql","document":document,"variables":{}});
    d
}
async fn workspace(router: &Router, token: Option<&str>, d: Value) -> Value {
    let (status, w) = call(
        router,
        "POST",
        "/api/workspaces",
        token,
        Some(json!({"id":"w","name":"GraphQL","data":d})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    w
}
async fn execute(router: &Router, w: &Value, r: Value) -> (StatusCode, Value) {
    call(
        router,
        "POST",
        "/api/execute",
        None,
        Some(json!({"workspace_id":w["id"],"request":r})),
    )
    .await
}
async fn wait(router: &Router, id: &str, state: &str) -> Value {
    for _ in 0..100 {
        let (status, s) = call(router, "GET", &format!("/api/sessions/{id}"), None, None).await;
        assert_eq!(status, StatusCode::OK, "{s}");
        if s["state"] == state {
            return s;
        }
        assert_ne!(s["state"], "error", "{s}");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("GraphQL session did not reach {state}")
}
#[tokio::test]
async fn real_queries_mutations_operation_selection_variables_and_errors() {
    let (url, server) = fixture().await;
    let tmp = tempfile::tempdir().unwrap();
    let router = local(&tmp.path().join("graphql.db")).await.unwrap();
    let w = workspace(
        &router,
        None,
        graphql_data(
            &format!("{url}/graphql"),
            "query Hello($name:String!){hello(name:$name)}",
        ),
    )
    .await;
    let mut r = w["data"]["collections"][0]["requests"][0].clone();
    r["protocol"]["variables"] = json!({"name":"quoted \"value\""});
    let (status, result) = execute(&router, &w, r.clone()).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["status"], 200);
    let body: Value = serde_json::from_str(result["body"].as_str().unwrap()).unwrap();
    assert_eq!(body["data"]["hello"], "Hello quoted \"value\"");
    r["protocol"] = json!({"kind":"graphql","document":"query First {hello(name:\"first\")} mutation Twice($value:Int!){double(value:$value)}","variables":{"value":4},"operation_name":"Twice"});
    let (status, result) = execute(&router, &w, r.clone()).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert!(result["body"].as_str().unwrap().contains("8"));
    r["protocol"]["operation_name"] = Value::Null;
    assert_eq!(
        execute(&router, &w, r.clone()).await.0,
        StatusCode::BAD_REQUEST
    );
    r["protocol"]["operation_name"] = json!("Missing");
    assert_eq!(
        execute(&router, &w, r.clone()).await.0,
        StatusCode::BAD_REQUEST
    );
    r["protocol"]["operation_name"] = json!("Twice");
    r["protocol"]["variables"] = json!({"value":"wrong"});
    assert_eq!(
        execute(&router, &w, r.clone()).await.0,
        StatusCode::BAD_REQUEST
    );
    r["protocol"] = json!({"kind":"graphql","document":"{failure}","variables":{}});
    let (status, result) = execute(&router, &w, r).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["status"], 200);
    assert!(
        result["body"]
            .as_str()
            .unwrap()
            .contains("fixture resolver error")
    );
    server.abort();
}
#[tokio::test]
async fn introspection_schema_sources_preserved_and_target401_does_not_logout() {
    let (url, server) = fixture().await;
    let tmp = tempfile::tempdir().unwrap();
    let router = local(&tmp.path().join("schema.db")).await.unwrap();
    let w = workspace(
        &router,
        None,
        graphql_data(&format!("{url}/graphql"), "{hello(name:\"a\")}"),
    )
    .await;
    let r = w["data"]["collections"][0]["requests"][0].clone();
    let (status, result) = call(
        &router,
        "POST",
        "/api/graphql/introspect",
        None,
        Some(json!({"workspace_id":"w","request":r})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert!(result["sdl"].as_str().unwrap().contains("hello"));
    assert_eq!(result["specification"]["kind"], "graphql-introspection");
    let mut d = w["data"].clone();
    let sdl = "\"\"\"Retained schema source\"\"\"\ntype Query { hello(name: String!): String! }\n";
    d["specifications"] = json!([result["specification"].clone(),{"id":"sdl","name":"Saved SDL","kind":"graphql-sdl","source":sdl,"dialect":"graphql"}]);
    let (status, saved) = call(
        &router,
        "PUT",
        "/api/workspaces/w",
        None,
        Some(json!({"expected_revision":w["revision"],"name":"GraphQL","data":d})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (status, source) = call(
        &router,
        "POST",
        "/api/graphql/schema",
        None,
        Some(json!({"workspace_id":"w","specification_id":"sdl"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{source}");
    assert_eq!(source["sdl"], sdl);
    assert_eq!(source["specification"]["source"], sdl);
    let (_, introspected) = call(
        &router,
        "POST",
        "/api/graphql/schema",
        None,
        Some(json!({"workspace_id":"w","specification_id":result["specification"]["id"]})),
    )
    .await;
    assert!(introspected["sdl"].as_str().unwrap().contains("double"));
    let mut r = r;
    r["url"] = json!(format!("{url}/auth"));
    let (status, result) = call(
        &router,
        "POST",
        "/api/graphql/introspect",
        None,
        Some(json!({"workspace_id":"w","request":r})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["response"]["status"], 401);
    assert!(result["error"].is_string());
    assert_eq!(
        call(&router, "GET", "/api/workspaces/w", None, None)
            .await
            .0,
        StatusCode::OK
    );
    server.abort();
}
#[tokio::test]
async fn mature_subscription_next_complete_error_and_cancel() {
    let (url, server) = fixture().await;
    let tmp = tempfile::tempdir().unwrap();
    let router = local(&tmp.path().join("subscriptions.db")).await.unwrap();
    let mut d = graphql_data(&format!("{url}/graphql"), "subscription {ticks}");
    d["collections"][0]["requests"][0]["protocol"]["subscription_url"] =
        json!(format!("{}/subscriptions", url.replacen("http", "ws", 1)));
    let w = workspace(&router, None, d).await;
    let base = w["data"]["collections"][0]["requests"][0].clone();
    for (document, expected) in [
        ("subscription {ticks}", "graphql_next"),
        ("subscription {unknown}", "graphql_next"),
        ("subscription {waiting}", "cancel"),
    ] {
        let mut r = base.clone();
        r["protocol"]["document"] = json!(document);
        let (status, s) = call(
            &router,
            "POST",
            "/api/sessions",
            None,
            Some(json!({"workspace_id":"w","request":r})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{s}");
        let id = s["id"].as_str().unwrap();
        assert_eq!(s["protocol"], "graphql");
        if expected == "cancel" {
            wait(&router, id, "open").await;
            assert_eq!(
                call(
                    &router,
                    "POST",
                    &format!("/api/sessions/{id}/close"),
                    None,
                    None
                )
                .await
                .1["state"],
                "closed"
            );
        } else {
            wait(&router, id, "closed").await;
            let (_, events) = call(
                &router,
                "GET",
                &format!("/api/sessions/{id}/events"),
                None,
                None,
            )
            .await;
            let messages: Vec<_> = events["events"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| &e["message"])
                .collect();
            assert!(messages.iter().any(|m| m["kind"] == expected), "{events}");
            assert!(
                messages
                    .iter()
                    .filter(|m| m["kind"] == expected)
                    .all(|m| m["operation_id"] == "1")
            );
            if expected == "graphql_next" {
                assert!(
                    messages
                        .iter()
                        .any(|m| m["kind"] == "graphql_complete" && m["payload"].is_null())
                );
            }
        }
        call(
            &router,
            "DELETE",
            &format!("/api/sessions/{id}"),
            None,
            None,
        )
        .await;
    }
    assert_eq!(execute(&router, &w, base).await.0, StatusCode::BAD_REQUEST);
    server.abort();
}
#[tokio::test]
async fn script_envelope_is_canonical_and_mutations_validate_before_network() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let count = Arc::new(AtomicUsize::new(0));
    let counted = count.clone();
    let (url, server) = serve(Router::new().route(
        "/graphql",
        post(move |Json(body): Json<Value>| {
            let count = counted.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                Json(json!({"data":{"query":body["query"],"variables":body["variables"]}}))
            }
        }),
    ))
    .await;
    let tmp = tempfile::tempdir().unwrap();
    let router = local(&tmp.path().join("scripts.db")).await.unwrap();
    let w = workspace(
        &router,
        None,
        graphql_data(&format!("{url}/graphql"), "{ original }"),
    )
    .await;
    let base = w["data"]["collections"][0]["requests"][0].clone();
    let mut r = base.clone();
    r["pre_request_script"] = json!(
        "const body=JSON.parse(pm.request.body.raw); pm.test('canonical envelope',()=>pm.expect(body.query).to.eql('{ original }')); body.query='query Changed($name:String!){hello(name:$name)}'; body.variables={name:'script'}; pm.request.body.update({mode:'json',raw:JSON.stringify(body)});"
    );
    let (status, response) = execute(&router, &w, r).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert!(response["body"].as_str().unwrap().contains("script"));
    assert_eq!(count.load(Ordering::SeqCst), 1);
    for script in [
        "pm.request.method='GET';",
        "pm.request.body.update({mode:'text',raw:'bad'});",
        "pm.request.body.update({mode:'json',raw:JSON.stringify({query:'query {',variables:{}})});",
        "pm.request.body.update({mode:'json',raw:JSON.stringify({query:'query Q($x:Int!){field}',variables:{x:'wrong'}})});",
    ] {
        let mut r = base.clone();
        r["pre_request_script"] = json!(script);
        assert_eq!(execute(&router, &w, r).await.0, StatusCode::BAD_REQUEST);
    }
    for protocol in [
        json!({"kind":"graphql","document":"query {"}),
        json!({"kind":"graphql","document":"query A {one} query B {two}"}),
        json!({"kind":"graphql","document":"{one}","variables_source":"{broken"}),
        json!({"kind":"graphql","document":"{one}","variables":[]}),
        json!({"kind":"graphql","document":" ".repeat(moleapi_core::MAX_GRAPHQL_DOCUMENT+1)}),
    ] {
        let mut r = base.clone();
        r["protocol"] = protocol;
        assert_eq!(execute(&router, &w, r).await.0, StatusCode::BAD_REQUEST);
    }
    assert_eq!(count.load(Ordering::SeqCst), 1);
    server.abort();
}

async fn terminal_error_socket(mut socket: axum::extract::ws::WebSocket) {
    use axum::extract::ws::Message;
    let init: Value =
        serde_json::from_str(socket.recv().await.unwrap().unwrap().to_text().unwrap()).unwrap();
    assert_eq!(init["type"], "connection_init");
    assert_eq!(init["payload"]["token"], "connection-private-token");
    socket
        .send(Message::Text(
            json!({"type":"ping","payload":{"probe":true}})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    let pong: Value =
        serde_json::from_str(socket.recv().await.unwrap().unwrap().to_text().unwrap()).unwrap();
    assert_eq!(pong["type"], "pong");
    socket
        .send(Message::Text(
            json!({"type":"connection_ack"}).to_string().into(),
        ))
        .await
        .unwrap();
    let subscribe: Value =
        serde_json::from_str(socket.recv().await.unwrap().unwrap().to_text().unwrap()).unwrap();
    assert_eq!(subscribe["type"], "subscribe");
    for event in [
        json!({"type":"next","id":"9999","payload":{"data":{"unsolicited":true}}}),
        json!({"type":"complete","id":"9999"}),
    ] {
        socket
            .send(Message::Text(event.to_string().into()))
            .await
            .unwrap();
    }
    socket.send(Message::Text(json!({"type":"error","id":subscribe["id"],"payload":[{"message":"terminal operation error"}]}).to_string().into())).await.unwrap();
    // Deliberately keep the wire open: the protocol error alone must end the session.
    let _ = tokio::time::timeout(Duration::from_secs(2), socket.recv()).await;
}
async fn redirected_http(headers: axum::http::HeaderMap, Json(body): Json<Value>) -> Json<Value> {
    assert!(!headers.contains_key("authorization"));
    assert!(!headers.contains_key("cookie"));
    Json(json!({"data":{"received":body["query"]}}))
}
async fn redirected_socket(mut socket: axum::extract::ws::WebSocket) {
    use axum::extract::ws::Message;
    let init: Value =
        serde_json::from_str(socket.recv().await.unwrap().unwrap().to_text().unwrap()).unwrap();
    assert_eq!(init["payload"], json!({}));
    socket
        .send(Message::Text(
            json!({"type":"connection_ack"}).to_string().into(),
        ))
        .await
        .unwrap();
    let subscribe: Value =
        serde_json::from_str(socket.recv().await.unwrap().unwrap().to_text().unwrap()).unwrap();
    for event in [
        json!({"type":"next","id":subscribe["id"],"payload":{"data":{"redirected":true}}}),
        json!({"type":"complete","id":subscribe["id"]}),
    ] {
        socket
            .send(Message::Text(event.to_string().into()))
            .await
            .unwrap();
    }
    let _ = socket.recv().await;
}

#[tokio::test]
async fn protocol_error_is_terminal_and_connection_credentials_are_not_recorded() {
    use axum::extract::ws::WebSocketUpgrade;
    let fixture = Router::new().route(
        "/wire",
        axum::routing::get(|upgrade: WebSocketUpgrade| async move {
            upgrade
                .protocols(["graphql-transport-ws"])
                .on_upgrade(terminal_error_socket)
        }),
    );
    let (url, server) = serve(fixture).await;
    let tmp = tempfile::tempdir().unwrap();
    let router = local(&tmp.path().join("wire.db")).await.unwrap();
    let mut d = graphql_data(&format!("{url}/wire"), "subscription {ticks}");
    d["collections"][0]["requests"][0]["protocol"]["connection_params"] =
        json!({"token":"connection-private-token"});
    let w = workspace(&router, None, d).await;
    let r = w["data"]["collections"][0]["requests"][0].clone();
    let (status, s) = call(
        &router,
        "POST",
        "/api/sessions",
        None,
        Some(json!({"workspace_id":"w","request":r})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{s}");
    let id = s["id"].as_str().unwrap();
    let closed = wait(&router, id, "closed").await;
    let (_, events) = call(
        &router,
        "GET",
        &format!("/api/sessions/{id}/events"),
        None,
        None,
    )
    .await;
    assert!(!s.to_string().contains("connection-private-token"));
    assert!(!closed.to_string().contains("connection-private-token"));
    assert!(!events.to_string().contains("connection-private-token"));
    assert!(!events.to_string().contains("unsolicited"));
    assert!(!events.to_string().contains("9999"));
    assert!(
        events["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["message"]["kind"] == "graphql_error"
                && e["message"]["operation_id"] == "1"
                && e["message"]["payload"][0]["message"] == "terminal operation error"),
        "{events}"
    );
    server.abort();
}
#[tokio::test]
async fn hosted_private_policy_owner_checks_and_target401_keep_login() {
    let (url, server) = fixture().await;
    let tmp = tempfile::tempdir().unwrap();
    let router = hosted(config(
        moleapi_server::sqlite_database_url(&tmp.path().join("policy.db")).unwrap(),
        true,
    ))
    .await
    .unwrap();
    let alice = register(&router, "graphql-alice").await;
    let bob = register(&router, "graphql-bob").await;
    let mut d = graphql_data(&format!("{url}/graphql"), "{hello(name:\"a\")}");
    d["specifications"] = json!([{"id":"schema","name":"Schema","kind":"graphql-sdl","source":"type Query { hello(name:String!):String! }","dialect":"graphql"}]);
    let w = workspace(&router, Some(&alice), d).await;
    let r = w["data"]["collections"][0]["requests"][0].clone();
    for (route, body) in [
        ("/api/execute", json!({"workspace_id":"w","request":r})),
        (
            "/api/graphql/introspect",
            json!({"workspace_id":"w","request":r}),
        ),
        (
            "/api/graphql/schema",
            json!({"workspace_id":"w","specification_id":"schema"}),
        ),
    ] {
        assert_eq!(
            call(&router, "POST", route, Some(&bob), Some(body)).await.0,
            StatusCode::NOT_FOUND
        );
    }
    let (status, error) = call(
        &router,
        "POST",
        "/api/execute",
        Some(&alice),
        Some(json!({"workspace_id":"w","request":r})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
    assert!(error["error"].as_str().unwrap().contains("Private"));
    let mut subscription = r.clone();
    subscription["protocol"]["document"] = json!("subscription {ticks}");
    subscription["protocol"]["subscription_url"] =
        json!(format!("{}/subscriptions", url.replacen("http", "ws", 1)));
    let (_, s) = call(
        &router,
        "POST",
        "/api/sessions",
        Some(&alice),
        Some(json!({"workspace_id":"w","request":subscription})),
    )
    .await;
    let id = s["id"].as_str().unwrap();
    assert_eq!(
        call(
            &router,
            "GET",
            &format!("/api/sessions/{id}"),
            Some(&bob),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    for _ in 0..100 {
        let (_, s) = call(
            &router,
            "GET",
            &format!("/api/sessions/{id}"),
            Some(&alice),
            None,
        )
        .await;
        if s["state"] == "error" {
            assert!(s["reason"].as_str().unwrap().contains("Private"));
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let (_, denied) = call(
        &router,
        "GET",
        &format!("/api/sessions/{id}"),
        Some(&alice),
        None,
    )
    .await;
    assert_eq!(denied["state"], "error", "{denied}");
    let mut allowed = config(
        moleapi_server::sqlite_database_url(&tmp.path().join("allowed.db")).unwrap(),
        true,
    );
    allowed.allow_private_network = true;
    let router = hosted(allowed).await.unwrap();
    let token = register(&router, "graphql-auth").await;
    let mut d = graphql_data(&format!("{url}/auth"), "{hello(name:\"a\")}");
    d["collections"][0]["requests"][0]["auth"] =
        json!({"kind":"bearer","token":"endpoint-token","username":"","password":""});
    let w = workspace(&router, Some(&token), d).await;
    let (_, result) = call(
        &router,
        "POST",
        "/api/graphql/introspect",
        Some(&token),
        Some(json!({"workspace_id":"w","request":w["data"]["collections"][0]["requests"][0]})),
    )
    .await;
    assert_eq!(result["response"]["status"], 401);
    assert_eq!(
        call(&router, "GET", "/api/workspaces/w", Some(&token), None)
            .await
            .0,
        StatusCode::OK
    );
    server.abort();
}
#[tokio::test]
async fn redirects_strip_http_and_websocket_credentials_across_origins() {
    use axum::extract::ws::WebSocketUpgrade;
    let target_fixture = Router::new().route("/http", post(redirected_http)).route(
        "/wire",
        axum::routing::get(
            |headers: axum::http::HeaderMap, upgrade: WebSocketUpgrade| async move {
                assert!(!headers.contains_key("authorization"));
                assert!(!headers.contains_key("cookie"));
                upgrade
                    .protocols(["graphql-transport-ws"])
                    .on_upgrade(redirected_socket)
            },
        ),
    );
    let (target, target_server) = serve(target_fixture).await;
    let http_target = format!("{target}/http");
    let ws_target = format!("{target}/wire");
    let (url, server) = serve(
        Router::new()
            .route(
                "/http",
                post(move || {
                    let target = http_target.clone();
                    async move { axum::response::Redirect::temporary(&target) }
                }),
            )
            .route(
                "/wire",
                axum::routing::get(move || {
                    let target = ws_target.clone();
                    async move { axum::response::Redirect::temporary(&target) }
                }),
            ),
    )
    .await;
    let tmp = tempfile::tempdir().unwrap();
    let router = local(&tmp.path().join("redirect.db")).await.unwrap();
    let mut d = graphql_data(&format!("{url}/http"), "{one}");
    let r = &mut d["collections"][0]["requests"][0];
    r["auth"] = json!({"kind":"bearer","token":"cross-origin-secret","username":"","password":""});
    r["headers"] = json!([{"id":"cookie","key":"cookie","value":"session=cross-origin-cookie","enabled":true}]);
    let w = workspace(&router, None, d).await;
    let base = w["data"]["collections"][0]["requests"][0].clone();
    let (status, response) = execute(&router, &w, base.clone()).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], 200);
    let mut r = base;
    r["protocol"]["document"] = json!("subscription {ticks}");
    r["protocol"]["connection_params"] = json!({"token":"cross-origin-connection-secret"});
    r["protocol"]["subscription_url"] = json!(format!("{}/wire", url.replacen("http", "ws", 1)));
    let (status, s) = call(
        &router,
        "POST",
        "/api/sessions",
        None,
        Some(json!({"workspace_id":"w","request":r})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{s}");
    let id = s["id"].as_str().unwrap();
    wait(&router, id, "closed").await;
    let (_, events) = call(
        &router,
        "GET",
        &format!("/api/sessions/{id}/events"),
        None,
        None,
    )
    .await;
    assert!(
        events["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["message"]["payload"]["data"]["redirected"] == true),
        "{events}"
    );
    target_server.abort();
    server.abort();
}
#[tokio::test]
async fn logout_cancels_graphql_and_fences_inflight_preparation() {
    use axum::extract::ws::WebSocketUpgrade;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let connections = Arc::new(AtomicUsize::new(0));
    let count = connections.clone();
    let (url, server) = serve(Router::new().route(
        "/wire",
        axum::routing::get(move |upgrade: WebSocketUpgrade| {
            let count = count.clone();
            async move {
                upgrade.protocols(["graphql-transport-ws"]).on_upgrade(
                    move |mut socket| async move {
                        count.fetch_add(1, Ordering::SeqCst);
                        let _ = socket.recv().await;
                        socket
                            .send(axum::extract::ws::Message::Text(
                                "{\"type\":\"connection_ack\"}".into(),
                            ))
                            .await
                            .unwrap();
                        while socket.recv().await.is_some() {}
                    },
                )
            }
        }),
    ))
    .await;
    let tmp = tempfile::tempdir().unwrap();
    let router = local(&tmp.path().join("logout.db")).await.unwrap();
    let w = workspace(
        &router,
        None,
        graphql_data(&format!("{url}/wire"), "subscription {waiting}"),
    )
    .await;
    let base = w["data"]["collections"][0]["requests"][0].clone();
    let (_, s) = call(
        &router,
        "POST",
        "/api/sessions",
        None,
        Some(json!({"workspace_id":"w","request":base})),
    )
    .await;
    let id = s["id"].as_str().unwrap();
    wait(&router, id, "open").await;
    let (_, closed) = call(&router, "POST", "/api/auth/logout", None, None).await;
    assert!(closed.get("error").is_none(), "{closed}");
    assert_eq!(
        call(&router, "GET", &format!("/api/sessions/{id}"), None, None)
            .await
            .1["state"],
        "closed"
    );
    let mut r = base;
    r["pre_request_script"] = json!("const until=Date.now()+150; while(Date.now()<until){};");
    let other = router.clone();
    let before = connections.load(Ordering::SeqCst);
    let creating = tokio::spawn(async move {
        call(
            &other,
            "POST",
            "/api/sessions",
            None,
            Some(json!({"workspace_id":"w","request":r})),
        )
        .await
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    call(&router, "POST", "/api/auth/logout", None, None).await;
    let (status, error) = creating.await.unwrap();
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{error}");
    assert_eq!(connections.load(Ordering::SeqCst), before);
    server.abort();
}

#[tokio::test]
async fn websocket_auth_rejection_and_legacy_protocol_are_explicit() {
    use axum::extract::ws::{CloseFrame, Message, WebSocketUpgrade};
    let fixture = Router::new()
        .route(
            "/reject",
            axum::routing::get(|upgrade: WebSocketUpgrade| async move {
                upgrade
                    .protocols(["graphql-transport-ws"])
                    .on_upgrade(|mut socket| async move {
                        let _ = socket.recv().await;
                        socket
                            .send(Message::Close(Some(CloseFrame {
                                code: 4401,
                                reason: "Target authentication required".into(),
                            })))
                            .await
                            .unwrap();
                        let _ = socket.recv().await;
                    })
            }),
        )
        .route(
            "/legacy",
            axum::routing::get(|upgrade: WebSocketUpgrade| async move {
                upgrade
                    .protocols(["graphql-ws"])
                    .on_upgrade(|mut socket| async move { while socket.recv().await.is_some() {} })
            }),
        );
    let (url, server) = serve(fixture).await;
    let tmp = tempfile::tempdir().unwrap();
    let router = local(&tmp.path().join("ws-auth.db")).await.unwrap();
    let w = workspace(
        &router,
        None,
        graphql_data(&format!("{url}/reject"), "subscription {waiting}"),
    )
    .await;
    let base = w["data"]["collections"][0]["requests"][0].clone();
    for (path, expected) in [("reject", "4401"), ("legacy", "graphql-transport-ws")] {
        let mut r = base.clone();
        r["url"] = json!(format!("{url}/{path}"));
        let (status, s) = call(
            &router,
            "POST",
            "/api/sessions",
            None,
            Some(json!({"workspace_id":"w","request":r})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{s}");
        let id = s["id"].as_str().unwrap();
        let error = wait(&router, id, "error").await;
        assert!(
            error["reason"].as_str().unwrap().contains(expected),
            "{error}"
        );
        assert_eq!(error["handshake"]["status"], 101);
        if path == "reject" {
            let (_, events) = call(
                &router,
                "GET",
                &format!("/api/sessions/{id}/events"),
                None,
                None,
            )
            .await;
            assert!(
                events["events"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|e| e["message"]["kind"] == "close" && e["message"]["code"] == 4401),
                "{events}"
            );
        }
        assert_eq!(
            call(&router, "GET", "/api/workspaces/w", None, None)
                .await
                .0,
            StatusCode::OK
        );
    }
    server.abort();
}

#[tokio::test]
async fn subscription_url_credentials_are_captured_before_script_failure_and_redirect() {
    use axum::extract::ws::{CloseFrame, Message, WebSocketUpgrade};
    let secret = "subscription-url-private-credential";
    let fixture = Router::new().route(
        "/wire",
        axum::routing::get(move |upgrade: WebSocketUpgrade| async move {
            upgrade
                .protocols(["graphql-transport-ws"])
                .on_upgrade(move |mut socket| async move {
                    let _ = socket.recv().await;
                    socket
                        .send(Message::Close(Some(CloseFrame {
                            code: 4401,
                            reason: format!("Target rejected {secret}").into(),
                        })))
                        .await
                        .unwrap();
                    let _ = socket.recv().await;
                })
        }),
    );
    let (target, target_server) = serve(fixture).await;
    let destination = format!("{target}/wire?api_key={secret}");
    let (url, redirect_server) = serve(Router::new().route(
        "/hop",
        axum::routing::get(move || {
            let destination = destination.clone();
            async move { axum::response::Redirect::temporary(&destination) }
        }),
    ))
    .await;
    let tmp = tempfile::tempdir().unwrap();
    let router = local(&tmp.path().join("subscription-url-privacy.db"))
        .await
        .unwrap();
    let mut d = graphql_data("https://example.com/graphql", "subscription {waiting}");
    d["global_variables"] = json!([{ "id":"alias","key":"alias","value":"api_key","enabled":true },{"id":"base","key":"base","value":url.replacen("http","ws",1),"enabled":true}]);
    d["collections"][0]["requests"][0]["protocol"]["subscription_url"] =
        json!(format!("{{{{base}}}}/hop?{{{{alias}}}}={secret}"));
    let w = workspace(&router, None, d).await;
    let base = w["data"]["collections"][0]["requests"][0].clone();
    let mut failed = base.clone();
    failed["pre_request_script"] = json!(format!("throw new Error('{secret}');"));
    let (status, error) = call(
        &router,
        "POST",
        "/api/sessions",
        None,
        Some(json!({"workspace_id":"w","request":failed})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(!error.to_string().contains(secret), "{error}");
    assert!(error["error"].as_str().unwrap().contains("REDACTED"));
    let mut r = base;
    r["pre_request_script"] = json!(format!("console.log('{secret}');"));
    let (status, session) = call(
        &router,
        "POST",
        "/api/sessions",
        None,
        Some(json!({"workspace_id":"w","request":r})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{session}");
    assert!(!session.to_string().contains(secret), "{session}");
    let id = session["id"].as_str().unwrap();
    let ended = wait(&router, id, "error").await;
    assert!(!ended.to_string().contains(secret), "{ended}");
    let (_, events) = call(
        &router,
        "GET",
        &format!("/api/sessions/{id}/events"),
        None,
        None,
    )
    .await;
    assert!(!events.to_string().contains(secret), "{events}");
    assert!(
        events["events"].as_array().unwrap().iter().any(
            |e| e["message"]["kind"] == "script_log" && e["message"]["message"] == "[REDACTED]"
        )
    );
    // A scope alias may temporarily become sensitive and return to public before throw.
    let (_, current) = call(&router, "GET", "/api/workspaces/w", None, None).await;
    let mut updated = current["data"].clone();
    updated["global_variables"][0]["value"] = json!("opaque");
    let (status, saved) = call(
        &router,
        "PUT",
        "/api/workspaces/w",
        None,
        Some(json!({"name":"GraphQL","expected_revision":current["revision"],"data":updated})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let mut changed = saved["data"]["collections"][0]["requests"][0].clone();
    changed["pre_request_script"] = json!(format!(
        "pm.globals.set('alias','api_key'); pm.globals.set('alias','opaque'); throw new Error('{secret}');"
    ));
    let (status, error) = call(
        &router,
        "POST",
        "/api/sessions",
        None,
        Some(json!({"workspace_id":"w","request":changed})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(!error.to_string().contains(secret), "{error}");
    assert!(error["error"].as_str().unwrap().contains("REDACTED"));
    redirect_server.abort();
    target_server.abort();
}
#[tokio::test]
async fn deeply_nested_sdl_save_rejects_and_api_stays_available() {
    let tmp = tempfile::tempdir().unwrap();
    let router = local(&tmp.path().join("schema-depth.db")).await.unwrap();
    let mut d = data();
    d["specifications"] = json!([{ "id":"s","name":"Deep SDL","kind":"graphql-sdl","source":format!("type Query {{ value:{}String{} }}","[".repeat(4000),"]".repeat(4000)),"dialect":"graphql"}]);
    let (status, error) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"deep","name":"Deep","data":d})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
    assert!(error["error"].as_str().unwrap().contains("nesting"));
    assert_eq!(
        call(&router, "GET", "/api/health", None, None).await.0,
        StatusCode::OK
    );
    workspace(
        &router,
        None,
        graphql_data("https://example.com/graphql", "{value}"),
    )
    .await;
}

#[tokio::test]
async fn aliased_subscription_ids_never_route_active_next_error_or_complete() {
    use axum::extract::ws::{Message, WebSocketUpgrade};
    for alias in ["01", "+1"] {
        for kind in ["next", "error", "complete"] {
            let fixture=Router::new().route("/wire",axum::routing::get(move |upgrade:WebSocketUpgrade|async move {
                upgrade.protocols(["graphql-transport-ws"]).on_upgrade(move |mut socket|async move {
                    let _=socket.recv().await;
                    socket.send(Message::Text(json!({"type":"connection_ack"}).to_string().into())).await.unwrap();
                    let subscribe:Value=serde_json::from_str(socket.recv().await.unwrap().unwrap().to_text().unwrap()).unwrap();assert_eq!(subscribe["id"],"1");
                    let event=match kind {
                        "next"=>json!({"type":"next","id":alias,"payload":{"data":{"aliased":true}}}),
                        "error"=>json!({"type":"error","id":alias,"payload":[{"message":"aliased operation error"}]}),
                        _=>json!({"type":"complete","id":alias}),
                    };
                    socket.send(Message::Text(event.to_string().into())).await.unwrap();
                    let _=socket.recv().await;
                })
            }));
            let (url, server) = serve(fixture).await;
            let tmp = tempfile::tempdir().unwrap();
            let router = local(&tmp.path().join("alias-id.db")).await.unwrap();
            let w = workspace(
                &router,
                None,
                graphql_data(&format!("{url}/wire"), "subscription {waiting}"),
            )
            .await;
            let (status,session)=call(&router,"POST","/api/sessions",None,Some(json!({"workspace_id":"w","request":w["data"]["collections"][0]["requests"][0]}))).await;
            assert_eq!(status, StatusCode::OK, "{session}");
            let id = session["id"].as_str().unwrap();
            for _ in 0..100 {
                let (_, summary) =
                    call(&router, "GET", &format!("/api/sessions/{id}"), None, None).await;
                if summary["state"] != "connecting" && summary["state"] != "open" {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            let (_, events) = call(
                &router,
                "GET",
                &format!("/api/sessions/{id}/events"),
                None,
                None,
            )
            .await;
            assert!(
                !events["events"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|event| event["message"]["kind"]
                        .as_str()
                        .is_some_and(|kind| kind.starts_with("graphql_"))),
                "alias {alias} {kind} reached active operation: {events}"
            );
            let (_, summary) =
                call(&router, "GET", &format!("/api/sessions/{id}"), None, None).await;
            assert_eq!(summary["state"], "error", "alias {alias} {kind}: {summary}");
            assert!(
                summary["reason"]
                    .as_str()
                    .unwrap()
                    .contains("unknown subscription")
            );
            server.abort();
        }
    }
}
#[tokio::test]
async fn graphql_js16_introspection_http_fixture_preserves_source_and_schema() {
    let source = include_str!("../../core/tests/fixtures/graphql-js16-introspection.json");
    let fixture = Router::new().route(
        "/graphql",
        post(move |Json(request): Json<Value>| async move {
            let generated = moleapi_core::introspection_payload();
            assert_eq!(request["query"], generated.query);
            assert!(request["variables"].is_object());
            ([("content-type", "application/json")], source)
        }),
    );
    let (url, server) = serve(fixture).await;
    let tmp = tempfile::tempdir().unwrap();
    let router = local(&tmp.path().join("js16-introspection.db"))
        .await
        .unwrap();
    let w = workspace(
        &router,
        None,
        graphql_data(&format!("{url}/graphql"), "{hello(name:\"World\")}"),
    )
    .await;
    let (status, result) = call(
        &router,
        "POST",
        "/api/graphql/introspect",
        None,
        Some(json!({"workspace_id":"w","request":w["data"]["collections"][0]["requests"][0]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["response"]["status"], 200);
    assert!(result.get("error").is_none(), "{result}");
    assert_eq!(result["specification"]["source"], source);
    let sdl = result["sdl"].as_str().unwrap();
    assert!(sdl.contains("type Mutation"));
    assert!(sdl.contains("type Subscription"));
    let mut d = w["data"].clone();
    d["specifications"] = json!([result["specification"].clone()]);
    let (status, saved) = call(
        &router,
        "PUT",
        "/api/workspaces/w",
        None,
        Some(json!({"name":"GraphQL","expected_revision":w["revision"],"data":d})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (status, schema) = call(
        &router,
        "POST",
        "/api/graphql/schema",
        None,
        Some(json!({"workspace_id":"w","specification_id":result["specification"]["id"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{schema}");
    assert_eq!(schema["specification"]["source"], source);
    assert_eq!(schema["sdl"], sdl);
    server.abort();
}
