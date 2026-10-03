mod common;
use common::*;
#[tokio::test]
async fn native_sync_persists_bases_and_conflicts_without_overwriting() {
    let dir = tempfile::tempdir().unwrap();
    let hosted = common::hosted(config(
        format!(
            "sqlite://{}?mode=rwc",
            dir.path().join("remote.db").display()
        ),
        true,
    ))
    .await
    .unwrap();
    let token = register(&hosted, "syncuser").await;
    let (url, server) = serve(hosted.clone()).await;
    let path = dir.path().join("native.db");
    let native = common::local(&path).await.unwrap();
    let (status, connected) = call(
        &native,
        "POST",
        "/api/sync/connect",
        None,
        Some(json!({"server_url":url,"username":"syncuser","password":"goodpassword123"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{connected}");
    assert!(connected.get("token").is_none());
    call(
        &native,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"sync-workspace","name":"Start","data":data()})),
    )
    .await;
    let (status, result) = call(
        &native,
        "POST",
        "/api/workspaces/sync-workspace/sync",
        None,
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["status"], "synced");
    rename(&native, None, "Local edit", 1).await;
    let (_, result) = call(
        &native,
        "POST",
        "/api/workspaces/sync-workspace/sync",
        None,
        Some(json!({})),
    )
    .await;
    assert_eq!(result["status"], "synced");
    assert_eq!(
        call(
            &hosted,
            "GET",
            "/api/workspaces/sync-workspace",
            Some(&token),
            None
        )
        .await
        .1["name"],
        "Local edit"
    );
    rename(&hosted, Some(&token), "Remote edit", 2).await;
    // A fresh native router proves connection and revision bases survive restarts.
    let native = common::local(&path).await.unwrap();
    let (_, result) = call(
        &native,
        "POST",
        "/api/workspaces/sync-workspace/sync",
        None,
        Some(json!({})),
    )
    .await;
    assert_eq!(result["status"], "synced");
    assert_eq!(result["workspace"]["name"], "Remote edit");
    assert_eq!(result["workspace"]["revision"], 3);
    rename(&native, None, "Diverged local", 3).await;
    rename(&hosted, Some(&token), "Diverged remote", 3).await;
    let (_, result) = call(
        &native,
        "POST",
        "/api/workspaces/sync-workspace/sync",
        None,
        Some(json!({})),
    )
    .await;
    assert_eq!(result["status"], "conflict");
    assert_eq!(result["workspace"]["name"], "Diverged local");
    assert_eq!(result["remote"]["name"], "Diverged remote");
    let (_, result) = call(
        &native,
        "POST",
        "/api/workspaces/sync-workspace/sync",
        None,
        Some(json!({"resolution":"pull"})),
    )
    .await;
    assert_eq!(result["workspace"]["name"], "Diverged remote");
    assert_eq!(result["workspace"]["revision"], 5);
    let (_, versions) = call(
        &native,
        "GET",
        "/api/workspaces/sync-workspace/versions",
        None,
        None,
    )
    .await;
    assert_eq!(versions[0]["name"], "Diverged local");
    rename(&native, None, "Force local", 5).await;
    rename(&hosted, Some(&token), "Other remote", 4).await;
    let (_, result) = call(
        &native,
        "POST",
        "/api/workspaces/sync-workspace/sync",
        None,
        Some(json!({"resolution":"push"})),
    )
    .await;
    assert_eq!(result["status"], "synced");
    assert_eq!(
        call(
            &hosted,
            "GET",
            "/api/workspaces/sync-workspace",
            Some(&token),
            None
        )
        .await
        .1["name"],
        "Force local"
    );
    let (_, result) = call(&native, "DELETE", "/api/sync/connect", None, None).await;
    assert_eq!(result, json!({"connected":false}));
    assert_eq!(
        call(&native, "GET", "/api/workspaces/sync-workspace", None, None)
            .await
            .0,
        StatusCode::OK
    );
    server.abort();
}
#[tokio::test]
async fn native_sync_rejects_local_edits_during_network_wait() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let dir = tempfile::tempdir().unwrap();
    let hosted = common::hosted(config(
        format!(
            "sqlite://{}?mode=rwc",
            dir.path().join("remote.db").display()
        ),
        true,
    ))
    .await
    .unwrap();
    let token = register(&hosted, "syncuser").await;
    call(
        &hosted,
        "POST",
        "/api/workspaces",
        Some(&token),
        Some(json!({"id":"sync-workspace","name":"Start","data":data()})),
    )
    .await;
    let waiting = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let enabled = Arc::new(AtomicBool::new(false));
    let router = hosted.clone().layer(axum::middleware::from_fn({
        let waiting = waiting.clone();
        let release = release.clone();
        let enabled = enabled.clone();
        move |req: axum::extract::Request, next: axum::middleware::Next| {
            let waiting = waiting.clone();
            let release = release.clone();
            let enabled = enabled.clone();
            async move {
                if enabled.load(Ordering::SeqCst)
                    && req.method() == "GET"
                    && req.uri().path() == "/api/workspaces/sync-workspace"
                {
                    waiting.notify_one();
                    release.notified().await;
                }
                next.run(req).await
            }
        }
    }));
    let (url, server) = serve(router).await;
    let native = common::local(&dir.path().join("native.db")).await.unwrap();
    call(
        &native,
        "POST",
        "/api/sync/connect",
        None,
        Some(json!({"server_url":url,"username":"syncuser","password":"goodpassword123"})),
    )
    .await;
    call(
        &native,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"sync-workspace","name":"Start","data":data()})),
    )
    .await;
    enabled.store(true, Ordering::SeqCst);
    let operation = tokio::spawn({
        let native = native.clone();
        async move {
            call(
                &native,
                "POST",
                "/api/workspaces/sync-workspace/sync",
                None,
                Some(json!({})),
            )
            .await
        }
    });
    tokio::time::timeout(std::time::Duration::from_secs(3), waiting.notified())
        .await
        .unwrap();
    rename(&native, None, "During wait", 1).await;
    release.notify_one();
    let (status, result) = operation.await.unwrap();
    assert_eq!(status, StatusCode::CONFLICT, "{result}");
    assert_eq!(
        call(&native, "GET", "/api/workspaces/sync-workspace", None, None)
            .await
            .1["name"],
        "During wait"
    );
    server.abort();
}
#[tokio::test]
async fn sync_credentials_never_follow_redirects_or_non_loopback_http() {
    let dir = tempfile::tempdir().unwrap();
    let native = common::local(&dir.path().join("native.db")).await.unwrap();
    assert_eq!(call(&native,"POST","/api/sync/connect",None,Some(json!({"server_url":"http://192.0.2.1","username":"user","password":"goodpassword123"}))).await.0,StatusCode::BAD_REQUEST);
    let (url, server) = serve(Router::new().route(
        "/api/auth/login",
        axum::routing::post(|| async {
            (
                StatusCode::TEMPORARY_REDIRECT,
                [("location", "http://other.invalid/api/auth/login")],
            )
        }),
    ))
    .await;
    assert_eq!(
        call(
            &native,
            "POST",
            "/api/sync/connect",
            None,
            Some(json!({"server_url":url,"username":"user","password":"goodpassword123"}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    server.abort();
}

async fn sync_fixture(
    id: &str,
) -> (
    tempfile::TempDir,
    Router,
    Router,
    String,
    tokio::task::JoinHandle<()>,
) {
    let dir = tempfile::tempdir().unwrap();
    let hosted = common::hosted(config(
        format!(
            "sqlite://{}?mode=rwc",
            dir.path().join("remote.db").display()
        ),
        true,
    ))
    .await
    .unwrap();
    let token = register(&hosted, "syncuser").await;
    let (url, server) = serve(hosted.clone()).await;
    let local = common::local(&dir.path().join("native.db")).await.unwrap();
    assert_eq!(
        call(
            &local,
            "POST",
            "/api/sync/connect",
            None,
            Some(json!({"server_url":url,"username":"syncuser","password":"goodpassword123"}))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &local,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":id,"name":"Original","data":data()}))
        )
        .await
        .0,
        StatusCode::OK
    );
    (dir, local, hosted, token, server)
}
#[tokio::test]
async fn sync_encodes_workspace_path_segments_without_form_plus_substitution() {
    let (_dir, local, _hosted, _token, server) = sync_fixture("my workspace").await;
    for _ in 0..2 {
        let (status, result) = call(
            &local,
            "POST",
            "/api/workspaces/my%20workspace/sync",
            None,
            Some(json!({})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{result}");
        assert_eq!(result["status"], "synced", "{result}");
    }
    server.abort();
}
#[tokio::test]
async fn recreated_remote_with_reused_revision_is_a_conflict_if_local_changed() {
    let (_dir, local, hosted, token, server) = sync_fixture("sync-workspace").await;
    assert_eq!(
        call(
            &local,
            "POST",
            "/api/workspaces/sync-workspace/sync",
            None,
            Some(json!({}))
        )
        .await
        .1["status"],
        "synced"
    );
    assert_eq!(
        call(
            &hosted,
            "DELETE",
            "/api/workspaces/sync-workspace",
            Some(&token),
            Some(json!({"expected_revision":1}))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &hosted,
            "POST",
            "/api/workspaces",
            Some(&token),
            Some(json!({"id":"sync-workspace","name":"Recreated remote","data":data()}))
        )
        .await
        .0,
        StatusCode::OK
    );
    rename(&local, None, "Independent local edit", 1).await;
    let (status, result) = call(
        &local,
        "POST",
        "/api/workspaces/sync-workspace/sync",
        None,
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["status"], "conflict", "{result}");
    assert_eq!(
        call(
            &hosted,
            "GET",
            "/api/workspaces/sync-workspace",
            Some(&token),
            None
        )
        .await
        .1["name"],
        "Recreated remote"
    );
    server.abort();
}
#[tokio::test]
async fn ambiguous_dot_workspace_ids_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let local = common::local(&dir.path().join("dots.db")).await.unwrap();
    for id in [".", ".."] {
        assert_eq!(
            call(
                &local,
                "POST",
                "/api/workspaces",
                None,
                Some(json!({"id":id,"name":"Invalid","data":data()}))
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
}
#[tokio::test]
async fn cloud_push_scrubs_local_values_and_pull_preserves_stable_native_identity() {
    let (_dir, local, hosted, token, server) = sync_fixture("sync-workspace").await;
    let mut original = data();
    original["global_variables"] = json!([{"id":"project","key":"p","value":"shared-project","local_value":"native-project","enabled":true}]);
    original["environments"] = json!([{"id":"env","name":"Env","variables":[{"id":"var","key":"key","value":"shared-env","local_value":"native-env","enabled":true,"secret":true}]}]);
    let (status, _) = call(
        &local,
        "PUT",
        "/api/workspaces/sync-workspace",
        None,
        Some(json!({"name":"Original","data":original,"expected_revision":1})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, pushed) = call(
        &local,
        "POST",
        "/api/workspaces/sync-workspace/sync",
        None,
        Some(json!({})),
    )
    .await;
    assert_eq!(pushed["status"], "synced", "{pushed}");
    let (_, remote) = call(
        &hosted,
        "GET",
        "/api/workspaces/sync-workspace",
        Some(&token),
        None,
    )
    .await;
    assert!(!remote.to_string().contains("native-project"));
    assert!(!remote.to_string().contains("native-env"));
    assert_eq!(
        remote["data"]["environments"][0]["variables"][0]["value"],
        "shared-env"
    );
    let (_, noop) = call(
        &local,
        "POST",
        "/api/workspaces/sync-workspace/sync",
        None,
        Some(json!({})),
    )
    .await;
    assert_eq!(noop["status"], "synced");
    assert_eq!(noop["message"], "Workspaces already match");
    let mut next = remote["data"].clone();
    next["environments"][0]["variables"][0]["value"] = json!("remote-edit");
    call(
        &hosted,
        "PUT",
        "/api/workspaces/sync-workspace",
        Some(&token),
        Some(json!({"name":"Remote edit","data":next,"expected_revision":1})),
    )
    .await;
    let (_, pulled) = call(
        &local,
        "POST",
        "/api/workspaces/sync-workspace/sync",
        None,
        Some(json!({})),
    )
    .await;
    assert_eq!(pulled["status"], "synced", "{pulled}");
    let saved = &pulled["workspace"]["data"];
    assert_eq!(
        saved["global_variables"][0]["local_value"],
        "native-project"
    );
    assert_eq!(
        saved["environments"][0]["variables"][0]["local_value"],
        "native-env"
    );
    assert_eq!(
        saved["environments"][0]["variables"][0]["value"],
        "remote-edit"
    );
    server.abort();
}

#[tokio::test]
async fn old_server_dropping_live_protocol_cannot_acknowledge_sync_baseline() {
    use axum::{
        Json,
        routing::{get, post},
    };
    use std::sync::{Arc, Mutex};
    let stored = Arc::new(Mutex::new(None::<Value>));
    let read = stored.clone();
    let write = stored.clone();
    let remote=Router::new().route("/api/auth/login",post(||async{Json(json!({"token":"fixture-token"}))}))
    .route("/api/workspaces/sync-workspace",get(move||{let read=read.clone();async move{
        match read.lock().unwrap().clone() {Some(w)=>(StatusCode::OK,Json(w)),None=>(StatusCode::NOT_FOUND,Json(json!({"error":"missing"}))) }
    }})).route("/api/workspaces",post(move|Json(mut body):Json<Value>|{let write=write.clone();async move{
        // Mimic the previous server's serde schema, which ignores additive protocol fields.
        for collection in body["data"]["collections"].as_array_mut().unwrap() {
            for request in collection["requests"].as_array_mut().unwrap() { request.as_object_mut().unwrap().remove("protocol"); }
        }
        let w=json!({"id":body["id"],"name":body["name"],"data":body["data"],"revision":1,"updated_at":"2026-10-03T00:00:00Z"});
        *write.lock().unwrap()=Some(w.clone());Json(w)
    }}));
    let (url, server) = serve(remote).await;
    let dir = tempfile::tempdir().unwrap();
    let native = common::local(&dir.path().join("native.db")).await.unwrap();
    let (status, body) = call(
        &native,
        "POST",
        "/api/sync/connect",
        None,
        Some(json!({"server_url":url,"username":"old-server","password":"password"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let mut d = example_data();
    d["collections"][0]["requests"][0]["protocol"] = json!({"kind":"sse"});
    assert_eq!(
        call(
            &native,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"sync-workspace","name":"SSE workspace","data":d}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, error) = call(
        &native,
        "POST",
        "/api/workspaces/sync-workspace/sync",
        None,
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        error["error"]
            .as_str()
            .unwrap()
            .contains("did not preserve")
    );
    // Without a committed baseline, the divergent remote triggers a conflict on retry.
    let (status, result) = call(
        &native,
        "POST",
        "/api/workspaces/sync-workspace/sync",
        None,
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["status"], "conflict");
    assert_eq!(
        result["workspace"]["data"]["collections"][0]["requests"][0]["protocol"]["kind"],
        "sse"
    );
    assert_eq!(result["workspace"]["revision"], 1);
    server.abort();
}
