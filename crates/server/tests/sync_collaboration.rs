mod common;
use common::*;

struct Fixture {
    _dir: tempfile::TempDir,
    local: Router,
    hosted: Router,
    owner: String,
    base: String,
    session: Value,
    server: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}
async fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let mut settings = config(
        moleapi_server::sqlite_database_url(&dir.path().join("hosted.db")).unwrap(),
        true,
    );
    settings.allow_private_network = true;
    let hosted = hosted(settings).await.unwrap();
    let owner = register(&hosted, "nativeowner").await;
    register(&hosted, "nativemember").await;
    let (_, saved) = call(
        &hosted,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Canonical","data":example_data()})),
    )
    .await;
    assert_eq!(saved["revision"], 1);
    let (url, server) = serve(hosted.clone()).await;
    let local = local(&dir.path().join("native.db")).await.unwrap();
    let (status, session) = call(
        &local,
        "POST",
        "/api/sync/connect",
        None,
        Some(json!({"server_url":url,"username":"nativemember","password":"goodpassword123"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{session}");
    let (_, invite) = call(
        &hosted,
        "POST",
        "/api/workspaces/w/invitations",
        Some(&owner),
        Some(json!({"role":"editor","expires_in_days":1})),
    )
    .await;
    let (status, accepted) = bridge(
        &local,
        &session,
        "POST",
        "/api/workspace-invitations/accept",
        Some(json!({"token":invite["token"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{accepted}");
    assert_eq!(accepted["status"], 200, "{accepted}");
    let base = format!(
        "/api/owners/{}/workspaces/w",
        accepted["body"]["owner_id"].as_str().unwrap()
    );
    call(
        &local,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"Local collision","data":example_data()})),
    )
    .await;
    Fixture {
        _dir: dir,
        local,
        hosted,
        owner,
        base,
        session,
        server,
    }
}
async fn bridge(
    local: &Router,
    session: &Value,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut payload = json!({"session_id":session["session_id"],"method":method,"path":path});
    if let Some(body) = body {
        payload["body"] = body;
    }
    call(
        local,
        "POST",
        "/api/sync/collaboration",
        None,
        Some(payload),
    )
    .await
}
#[tokio::test]
async fn native_members_use_canonical_hosted_data_and_cas_without_copying_local_workspace() {
    let f = fixture().await;
    let (status, list) = bridge(&f.local, &f.session, "GET", "/api/workspaces/shared", None).await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert_eq!(list["status"], 200);
    assert_eq!(list["body"]["items"][0]["role"], "editor");
    let (_, read) = bridge(&f.local, &f.session, "GET", &f.base, None).await;
    assert_eq!(read["body"]["name"], "Canonical");
    let mut w = read["body"].clone();
    w["name"] = json!("Member change");
    let (_, updated) = bridge(
        &f.local,
        &f.session,
        "PUT",
        &f.base,
        Some(json!({"expected_revision":w["revision"],"name":w["name"],"data":w["data"]})),
    )
    .await;
    assert_eq!(updated["status"], 200, "{updated}");
    assert_eq!(updated["body"]["revision"], 2);
    let (_, stale) = bridge(
        &f.local,
        &f.session,
        "PUT",
        &f.base,
        Some(json!({"expected_revision":1,"name":"stale","data":w["data"]})),
    )
    .await;
    assert_eq!(stale["status"], 409, "{stale}");
    assert_eq!(
        call(&f.local, "GET", "/api/workspaces/w", None, None)
            .await
            .1["name"],
        "Local collision"
    );
    assert_eq!(
        call(&f.hosted, "GET", "/api/workspaces/w", Some(&f.owner), None)
            .await
            .1["name"],
        "Member change"
    );
    let (status, _) = call(
        &f.hosted,
        "POST",
        "/api/sync/collaboration",
        Some(&f.owner),
        Some(json!({"session_id":"x","method":"GET","path":"/api/workspaces/shared"})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(f.session.get("token").is_none());
    assert!(!updated.to_string().contains(&f.owner));
}
#[tokio::test]
async fn native_member_bridge_denies_unscoped_paths_overrides_methods_and_old_sessions() {
    let f = fixture().await;
    for (method, path) in [
        ("GET", "https://example.test/api/workspaces/shared"),
        ("GET", "//evil.test/api/workspaces/shared"),
        ("GET", "/api/auth/me"),
        ("GET", "/api/workspaces/w"),
        ("DELETE", &f.base),
        ("GET", "/api/owners/a/workspaces/../auth/me"),
        ("GET", "/api/owners/a/workspaces/%2e%2e/members"),
        ("GET", "/api/owners/a/workspaces/w%2fmembers"),
        ("GET", "/api/workspaces/shared?token=secret"),
        ("GET", "/api/workspaces/shared?cursor=a&cursor=b"),
        ("POST", "/api/owners/a/workspaces/w/mock-shares"),
    ] {
        let (status, value) = bridge(&f.local, &f.session, method, path, None).await;
        assert!(status.is_client_error(), "{method} {path}: {value}");
    }
    let (status,value)=call(&f.local,"POST","/api/sync/collaboration",None,Some(json!({"session_id":f.session["session_id"],"method":"GET","path":f.base,"authorization":"override"}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{value}");
    let (status, value) = bridge(
        &f.local,
        &json!({"session_id":"stale"}),
        "GET",
        &f.base,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{value}");
    call(&f.local, "DELETE", "/api/sync/connect", None, None).await;
    let (status, value) = bridge(&f.local, &f.session, "GET", &f.base, None).await;
    assert_eq!(status, StatusCode::CONFLICT, "{value}");
}

#[tokio::test]
async fn native_member_executes_remote_request_and_keeps_mock_401_inert() {
    let f = fixture().await;
    let (wire, worker) =
        serve(Router::new().route("/echo", axum::routing::get(|| async { "remote wire" }))).await;
    let (_, mut w) = call(&f.hosted, "GET", "/api/workspaces/w", Some(&f.owner), None).await;
    w["data"]["collections"][0]["requests"][0]["url"] = json!(format!("{wire}/echo"));
    w["data"]["collections"][0]["requests"][0]["examples"] = json!([{"id":"e","name":"Inert","status":401,"body":"<script>never execute</script>","headers":[{"id":"h","key":"Set-Cookie","value":"native-mock=1","enabled":true}]}]);
    let (status, w) = call(
        &f.hosted,
        "PUT",
        "/api/workspaces/w",
        Some(&f.owner),
        Some(json!({"expected_revision":w["revision"],"name":w["name"],"data":w["data"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    let request = w["data"]["collections"][0]["requests"][0]["id"]
        .as_str()
        .unwrap();
    let (_, executed) = bridge(
        &f.local,
        &f.session,
        "POST",
        &format!("{}/requests/{request}/execute", f.base),
        Some(json!({})),
    )
    .await;
    assert_eq!(executed["status"], 200, "{executed}");
    assert_eq!(executed["body"]["body"], "remote wire");
    let path = format!("{}/mock/{request}/e", f.base);
    let (outer,preview)=call(&f.local,"POST","/api/sync/collaboration",None,Some(json!({"session_id":f.session["session_id"],"method":"GET","path":path,"mode":"mock_preview"}))).await;
    assert_eq!(outer, StatusCode::OK, "{preview}");
    assert_eq!(preview["status"], 401);
    assert_eq!(preview["mock_preview"], true);
    assert_eq!(preview["body"], "<script>never execute</script>");
    let (_,route)=bridge(&f.local,&f.session,"POST",&format!("{}/mock/route-preview",f.base),Some(json!({"expected_revision":w["revision"],"method":"GET","path":"/echo","body":"","query":[],"headers":[]}))).await;
    assert_eq!(route["status"], 200, "{route}");
    assert_eq!(route["body"]["status"], 401);
    let (_, members) = call(
        &f.hosted,
        "GET",
        &format!("{}/members", f.base),
        Some(&f.owner),
        None,
    )
    .await;
    let member = members
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["username"] == "nativemember")
        .unwrap();
    let (status, value) = call(
        &f.hosted,
        "PATCH",
        &format!(
            "{}/members/{}",
            f.base,
            member["member_id"].as_str().unwrap()
        ),
        Some(&f.owner),
        Some(json!({"role":"viewer","expected_revision":member["revision"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    let (_, denied) = bridge(
        &f.local,
        &f.session,
        "POST",
        &format!("{}/requests/{request}/execute", f.base),
        Some(json!({})),
    )
    .await;
    assert_eq!(denied["status"], 403, "{denied}");
    let (_, foreign) = bridge(
        &f.local,
        &f.session,
        "GET",
        &format!("{}/missing", f.base.rsplit_once('/').unwrap().0),
        None,
    )
    .await;
    assert_eq!(foreign["status"], 404);
    worker.abort();
}

#[tokio::test]
async fn native_connection_changes_cancel_held_headers_and_body_without_blocking_disconnect() {
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Notify;
    for body_delay in [false, true] {
        for change in ["disconnect", "reconnect", "account", "server"] {
            let entered = Arc::new(Notify::new());
            let signal = entered.clone();
            let hosted = Router::new()
                .route(
                    "/api/auth/login",
                    axum::routing::post(|| async {
                        axum::Json(json!({"token":"server-side-fixture-token"}))
                    }),
                )
                .route(
                    "/api/workspaces/shared",
                    axum::routing::get(move || {
                        let signal = signal.clone();
                        async move {
                            if body_delay {
                                axum::response::Response::new(Body::from_stream(
                                    futures_util::stream::once(async move {
                                        signal.notify_one();
                                        tokio::time::sleep(Duration::from_secs(30)).await;
                                        Ok::<_, std::io::Error>(axum::body::Bytes::from_static(
                                            b"{\"items\":[]}",
                                        ))
                                    }),
                                ))
                            } else {
                                signal.notify_one();
                                tokio::time::sleep(Duration::from_secs(30)).await;
                                axum::response::Response::new(Body::from("{\"items\":[]}"))
                            }
                        }
                    }),
                );
            let (url, server) = serve(hosted.clone()).await;
            let dir = tempfile::tempdir().unwrap();
            let local = local(&dir.path().join("local.db")).await.unwrap();
            let (_, session) = call(
                &local,
                "POST",
                "/api/sync/connect",
                None,
                Some(json!({"server_url":url,"username":"member","password":"test"})),
            )
            .await;
            let request = {
                let local = local.clone();
                let session = session.clone();
                tokio::spawn(async move {
                    bridge(&local, &session, "GET", "/api/workspaces/shared", None).await
                })
            };
            tokio::time::timeout(Duration::from_secs(3), entered.notified())
                .await
                .unwrap();
            assert!(!request.is_finished());
            let mut second_server = None;
            let response = if change == "disconnect" {
                tokio::time::timeout(
                    Duration::from_secs(2),
                    call(&local, "DELETE", "/api/sync/connect", None, None),
                )
                .await
                .unwrap()
            } else {
                let target = if change == "server" {
                    let (next, task) = serve(hosted).await;
                    second_server = Some(task);
                    next
                } else {
                    url.clone()
                };
                tokio::time::timeout(Duration::from_secs(2),call(&local,"POST","/api/sync/connect",None,Some(json!({"server_url":target,"username":if change=="account" {"another"} else {"member"},"password":"test"})))).await.unwrap()
            };
            assert_eq!(
                response.0,
                StatusCode::OK,
                "{change} body={body_delay}: {}",
                response.1
            );
            let (status, value) = tokio::time::timeout(Duration::from_secs(2), request)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                status,
                StatusCode::CONFLICT,
                "{change} body={body_delay}: {value}"
            );
            assert!(!value.to_string().contains("server-side-fixture-token"));
            server.abort();
            if let Some(server) = second_server {
                server.abort();
            }
        }
    }
}

#[tokio::test]
async fn native_member_bridge_uses_only_stored_credentials_and_bounds_untrusted_replies() {
    use axum::{extract::Query, http::HeaderMap, response::IntoResponse};
    use std::collections::HashMap;
    let router = Router::new()
        .route(
            "/api/auth/login",
            axum::routing::post(|| async {
                axum::Json(json!({"token":"stored-native-fixture-token"}))
            }),
        )
        .route(
            "/api/workspaces/shared",
            axum::routing::get(
                |headers: HeaderMap, Query(query): Query<HashMap<String, String>>| async move {
                    assert_eq!(
                        headers.get("authorization").unwrap(),
                        "Bearer stored-native-fixture-token"
                    );
                    assert!(!headers.contains_key("cookie"));
                    assert!(!headers.contains_key("x-ambient"));
                    match query.get("cursor").map(String::as_str) {
                        Some("redirect") => (
                            StatusCode::FOUND,
                            [("location", "/api/auth/me"), ("set-cookie", "ambient=1")],
                            "never follow",
                        )
                            .into_response(),
                        Some("html") => (
                            StatusCode::OK,
                            [("content-type", "text/html")],
                            "<script>untrusted source</script>",
                        )
                            .into_response(),
                        Some("reflect") => axum::Json(
                            json!({"items":[],"echo":"Bearer stored-native-fixture-token"}),
                        )
                        .into_response(),
                        Some("escaped-token") => (
                            [("content-type", "application/json")],
                            r#"{"items":[],"echo":"\u0073tored-native-fixture-token"}"#,
                        )
                            .into_response(),
                        Some("escaped-key") => (
                            [("content-type", "application/json")],
                            r#"{"\u0073tored-native-fixture-token":"copied key"}"#,
                        )
                            .into_response(),
                        Some("json-text") => axum::Json(
                            json!({"body":r#"{"echo":"\u0073tored-native-fixture-token"}"#}),
                        )
                        .into_response(),
                        Some("oversize") => axum::response::Response::new(Body::from_stream(
                            futures_util::stream::once(async {
                                Ok::<_, std::io::Error>(vec![b'x'; 26 * 1024 * 1024])
                            }),
                        )),
                        _ => axum::Json(json!({"items":[],"next_cursor":null})).into_response(),
                    }
                },
            ),
        );
    let router = router.route(
        "/api/owners/o/workspaces/w/mock/r/e",
        axum::routing::get(|| async {
            (
                [("x-moleapi-mock-preview", "saved-example")],
                r#"{"echo":"\u0073tored-native-fixture-token"}"#,
            )
        }),
    );
    let (url, server) = serve(router).await;
    let dir = tempfile::tempdir().unwrap();
    let local = local(&dir.path().join("local.db")).await.unwrap();
    let (_, session) = call(
        &local,
        "POST",
        "/api/sync/connect",
        None,
        Some(json!({"server_url":url,"username":"member","password":"test"})),
    )
    .await;
    let response=local.clone().oneshot(Request::builder().method("POST").uri("/api/sync/collaboration").header("content-type","application/json").header("authorization","Bearer ambient-actor").header("cookie","outer=secret").header("x-ambient","never forward").body(Body::from(json!({"session_id":session["session_id"],"method":"GET","path":"/api/workspaces/shared"}).to_string())).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!response.headers().contains_key("set-cookie"));
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("stored-native-fixture-token"));
    let (_, redirect) = bridge(
        &local,
        &session,
        "GET",
        "/api/workspaces/shared?cursor=redirect",
        None,
    )
    .await;
    assert_eq!(redirect["status"], 302, "{redirect}");
    assert!(redirect.get("location").is_none());
    for cursor in [
        "html",
        "oversize",
        "reflect",
        "escaped-token",
        "escaped-key",
        "json-text",
    ] {
        let (status, value) = bridge(
            &local,
            &session,
            "GET",
            &format!("/api/workspaces/shared?cursor={cursor}"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{cursor}: {value}");
        assert!(!value.to_string().contains("untrusted source"));
        assert!(!value.to_string().contains("stored-native-fixture-token"));
    }
    let(status,value)=call(&local,"POST","/api/sync/collaboration",None,Some(json!({"session_id":session["session_id"],"method":"GET","path":"/api/owners/o/workspaces/w/mock/r/e","mode":"mock_preview"}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{value}");
    assert!(!value.to_string().contains("stored-native-fixture-token"));
    server.abort();
}

#[tokio::test]
async fn native_save_with_lost_reply_has_unknown_outcome_and_never_updates_local_owned_data() {
    use std::{sync::Arc, time::Duration};
    let mut f = fixture().await;
    let entered = Arc::new(tokio::sync::Notify::new());
    let signal = entered.clone();
    let base = f.base.clone();
    let wrapped = f.hosted.clone().layer(axum::middleware::from_fn(
        move |request: Request<Body>, next: axum::middleware::Next| {
            let signal = signal.clone();
            let base = base.clone();
            async move {
                let delay = request.method() == "PUT" && request.uri().path() == base;
                let response = next.run(request).await;
                if !delay {
                    return response;
                }
                assert_eq!(response.status(), StatusCode::OK);
                signal.notify_one();
                let (mut parts, body) = response.into_parts();
                parts.headers.remove("content-length");
                axum::response::Response::from_parts(
                    parts,
                    Body::from_stream(futures_util::stream::once(async move {
                        tokio::time::sleep(Duration::from_secs(30)).await;
                        to_bytes(body, 25 * 1024 * 1024)
                            .await
                            .map_err(std::io::Error::other)
                    })),
                )
            }
        },
    ));
    let (url, server) = serve(wrapped).await;
    let (_, session) = call(
        &f.local,
        "POST",
        "/api/sync/connect",
        None,
        Some(json!({"server_url":url,"username":"nativemember","password":"goodpassword123"})),
    )
    .await;
    f.session = session;
    let (_, w) = bridge(&f.local, &f.session, "GET", &f.base, None).await;
    let request = {
        let local = f.local.clone();
        let session = f.session.clone();
        let base = f.base.clone();
        tokio::spawn(async move {
            bridge(&local,&session,"PUT",&base,Some(json!({"expected_revision":w["body"]["revision"],"name":"Committed remote only","data":w["body"]["data"]}))).await
        })
    };
    tokio::time::timeout(Duration::from_secs(3), entered.notified())
        .await
        .unwrap();
    assert_eq!(
        call(&f.local, "DELETE", "/api/sync/connect", None, None)
            .await
            .0,
        StatusCode::OK
    );
    let (status, value) = tokio::time::timeout(Duration::from_secs(2), request)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status, StatusCode::CONFLICT, "{value}");
    let (_, canonical) = call(&f.hosted, "GET", "/api/workspaces/w", Some(&f.owner), None).await;
    assert_eq!(canonical["name"], "Committed remote only");
    assert_eq!(canonical["revision"], 2);
    let (_, local) = call(&f.local, "GET", "/api/workspaces/w", None, None).await;
    assert_eq!(local["name"], "Local collision");
    assert_eq!(local["revision"], 1);
    server.abort();
}

#[tokio::test]
async fn native_disconnect_and_reconnect_revoke_the_previous_hosted_login() {
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
    for reconnect in [false, true] {
        let f = fixture().await;
        // Inspect test SQLite directly: the stored bearer is never a frontend field.
        let db = Database::connect(
            moleapi_server::sqlite_database_url(&f._dir.path().join("native.db")).unwrap(),
        )
        .await
        .unwrap();
        let row = db
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT payload FROM documents WHERE id = 'native-connection'".to_owned(),
            ))
            .await
            .unwrap()
            .unwrap();
        let connection: Value =
            serde_json::from_str(&row.try_get::<String>("", "payload").unwrap()).unwrap();
        let old = connection["token"].as_str().unwrap();
        assert_eq!(
            call(&f.hosted, "GET", "/api/auth/me", Some(old), None)
                .await
                .0,
            StatusCode::OK
        );
        if reconnect {
            assert_eq!(call(&f.local,"POST","/api/sync/connect",None,Some(json!({"server_url":connection["server_url"],"username":"nativemember","password":"goodpassword123"}))).await.0,StatusCode::OK);
        } else {
            assert_eq!(
                call(&f.local, "DELETE", "/api/sync/connect", None, None)
                    .await
                    .0,
                StatusCode::OK
            );
        }
        let (status, value) = call(&f.hosted, "GET", "/api/auth/me", Some(old), None).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "old hosted login remains active after reconnect={reconnect}: {value}"
        );
    }
}

#[tokio::test]
async fn native_member_paths_preserve_supported_encoded_workspace_identifiers() {
    let f = fixture().await;
    let id = "w ?# space";
    let encoded = "w%20%3F%23%20space";
    let (status, value) = call(
        &f.hosted,
        "POST",
        "/api/workspaces",
        Some(&f.owner),
        Some(json!({"id":id,"name":"Encoded canonical","data":example_data()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    let (_, invite) = call(
        &f.hosted,
        "POST",
        &format!("/api/workspaces/{encoded}/invitations"),
        Some(&f.owner),
        Some(json!({"role":"editor","expires_in_days":1})),
    )
    .await;
    let (_, accepted) = bridge(
        &f.local,
        &f.session,
        "POST",
        "/api/workspace-invitations/accept",
        Some(json!({"token":invite["token"]})),
    )
    .await;
    assert_eq!(accepted["status"], 200, "{accepted}");
    let path = format!("{}/{encoded}", f.base.strip_suffix("/w").unwrap());
    let (status, value) = bridge(&f.local, &f.session, "GET", &path, None).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(value["status"], 200, "{value}");
    assert_eq!(value["body"]["id"], id);
    assert_eq!(
        bridge(
            &f.local,
            &f.session,
            "GET",
            "/api/workspaces/shared#fragment",
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}
