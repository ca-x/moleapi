mod common;
use base64::Engine;
use common::*;

#[tokio::test]
async fn v2_dataset_queries_wrap_real_request_and_keep_private_rows_out_of_history() {
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("script-datasets.db")).await.unwrap();
    let (url, server) = serve(Router::new().route(
        "/",
        axum::routing::get(|headers: axum::http::HeaderMap| async move {
            assert_eq!(headers["x-dataset"], "private-row");
            "response"
        }),
    ))
    .await;
    let mut data = example_data();
    data["script_mode"] = json!("v2");
    data["datasets"] = json!([{"id":"saved","name":"Saved","table_name":"rows","secret":true,"source":{"format":"json","source":"[{\"token\":\"private-row\",\"flag\":true}]"},"views":[{"id":"flags","name":"Flags","sql":"SELECT flag FROM rows WHERE token = ?"}]}]);
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = json!(format!("{url}/"));
    request["pre_request_script"] = json!(
        "const result=await pm.datasets('saved').executeQuery('SELECT token FROM rows WHERE flag = ?', ['1']); for await (const row of result.rows) pm.request.headers.upsert({key:'x-dataset',value:row.token});"
    );
    request["post_response_script"] = json!(
        "const result=await pm.datasets('saved').executeView('flags',['private-row']); for await (const row of result.rows) pm.test('boolean row',()=>pm.expect(row.flag).to.equal(true));"
    );
    let request = request.clone();
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
    let (status, response) = call(
        &router,
        "POST",
        "/api/execute",
        None,
        Some(json!({"workspace_id":"w","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert!(
        response["tests"]
            .as_array()
            .unwrap()
            .iter()
            .any(|test| test["name"] == "boolean row" && test["passed"] == true),
        "{response}"
    );
    let (_, history) = call(&router, "GET", "/api/workspaces/w/history", None, None).await;
    assert!(!history.to_string().contains("private-row"), "{history}");
    let mut rejected = request.clone();
    rejected["pre_request_script"] = json!(
        "const result=await pm.datasets('saved').executeQuery('SELECT token FROM rows'); for await (const row of result.rows) throw new Error(row.token);"
    );
    let (status, error) = call(
        &router,
        "POST",
        "/api/execute",
        None,
        Some(json!({"workspace_id":"w","request":rejected})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
    assert!(!error.to_string().contains("private-row"), "{error}");
    server.abort();
}

#[tokio::test]
async fn dataset_script_snapshot_is_owned_and_missing_sources_fail_before_request() {
    let dir = tempfile::tempdir().unwrap();
    let (url, server) = serve(Router::new().route(
        "/",
        axum::routing::get(|headers: axum::http::HeaderMap| async move {
            assert_eq!(headers["x-owner-dataset"], "private-owner");
            "ok"
        }),
    ))
    .await;
    let mut cfg = config(
        format!(
            "sqlite://{}?mode=rwc",
            dir.path().join("hosted.db").display()
        ),
        true,
    );
    cfg.allow_private_network = true;
    let hosted = hosted(cfg).await.unwrap();
    let alice = register(&hosted, "datasetalice").await;
    let bob = register(&hosted, "datasetbobby").await;
    let mut data = example_data();
    data["script_mode"] = json!("v2");
    data["datasets"] = json!([{"id":"saved","name":"Private","secret":true,"source":{"format":"json","source":"[{\"token\":\"private-owner\"}]"}}]);
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = json!(format!("{url}/"));
    request["pre_request_script"] = json!(
        "const result=await pm.datasets('saved').executeQuery('SELECT token FROM data'); for await (const row of result.rows) pm.request.headers.upsert({key:'x-owner-dataset',value:row.token});"
    );
    let request = request.clone();
    let (status, result) = call(
        &hosted,
        "POST",
        "/api/workspaces",
        Some(&alice),
        Some(json!({"id":"w","name":"W","data":data.clone()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(
        call(
            &hosted,
            "POST",
            "/api/execute",
            Some(&bob),
            Some(json!({"workspace_id":"w","request":request}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, result) = call(
        &hosted,
        "POST",
        "/api/execute",
        Some(&alice),
        Some(json!({"workspace_id":"w","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    data["datasets"][0]
        .as_object_mut()
        .unwrap()
        .remove("source");
    let (status, result) = call(
        &hosted,
        "PUT",
        "/api/workspaces/w",
        Some(&alice),
        Some(json!({"name":"W","data":data,"expected_revision":1})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let (status, error) = call(
        &hosted,
        "POST",
        "/api/execute",
        Some(&alice),
        Some(json!({"workspace_id":"w","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
    assert!(
        error.to_string().contains("snapshot unavailable"),
        "{error}"
    );
    server.abort();
}

#[tokio::test]
async fn v2_scripts_do_not_block_a_separate_query_backed_runner_dataset() {
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::get(
            |axum::extract::Query(values): axum::extract::Query<
                std::collections::HashMap<String, String>,
            >| async move { axum::Json(json!(values)) },
        ),
    ))
    .await;
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("mixed-sources.db")).await.unwrap();
    let mut data = example_data();
    data["script_mode"] = json!("v2");
    data["collections"][0]["requests"][0]["url"] = json!(format!("{url}/?value={{{{value}}}}"));
    data["collections"][0]["requests"][0]["pre_request_script"] =
        json!("pm.test('v2',()=>pm.expect(pm.compatibilityVersion).to.equal('moleapi-pm/2'));");
    let mut query = data["collections"][0]["requests"][0].clone();
    query["id"] = json!("data-source");
    query["url"] = json!("");
    query["pre_request_script"] = json!("");
    query["protocol"] = json!({"kind":"data","source":"local_file","file_name":"rows.csv","file_format":"csv","file_base64":base64::engine::general_purpose::STANDARD.encode(b"value\nfirst\nsecond\n"),"table_name":"data","sql":"SELECT value FROM data","read_only":true});
    data["collections"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"sources","name":"Sources","description":"","requests":[query]}));
    data["datasets"] = json!([{"id":"live","name":"Live","query":{"request_id":"data-source"}}]);
    let (status, result) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"W","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let (status, report) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        None,
        Some(json!({"collection_id":"c","dataset_id":"live"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["passed"], 2, "{report}");
    fixture.abort();
}

#[tokio::test]
async fn live_script_dataset_loads_on_demand_before_the_real_http_request() {
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::get(|headers: axum::http::HeaderMap| async move {
            assert_eq!(headers["x-live"], "live-private");
            "ok"
        }),
    ))
    .await;
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("script-live.db")).await.unwrap();
    let mut data = example_data();
    data["script_mode"] = json!("v2");
    data["global_variables"] =
        json!([{"id":"target","key":"target","value":"'wrong'","enabled":true,"secret":true}]);
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = json!(format!("{url}/"));
    request["pre_request_script"] = json!(
        "pm.variables.set('target',\"'live-private'\"); const result=await pm.datasets('live').executeView('by-value',['live-private']); for await(const row of result.rows) pm.request.headers.upsert({key:'x-live',value:row.value});"
    );
    let request = request.clone();
    let mut source = request.clone();
    source["id"] = json!("data-query");
    source["url"] = json!("");
    source["pre_request_script"] = json!("");
    source["protocol"] = json!({"kind":"data","source":"local_file","file_name":"rows.csv","file_format":"csv","file_base64":base64::engine::general_purpose::STANDARD.encode(b"value\nwrong\nlive-private\n"),"table_name":"data","sql":"SELECT value FROM data WHERE value = {{target}}","read_only":true});
    let mut unused = source.clone();
    unused["id"] = json!("unused-query");
    unused["url"] = json!("http://127.0.0.1:1/unreachable");
    unused["protocol"] = json!({"kind":"data","source":"remote_file","file_name":"missing.csv","file_format":"csv","table_name":"data","sql":"SELECT value FROM data","read_only":true});
    data["collections"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"sources","name":"Sources","description":"","requests":[source,unused]}));
    data["datasets"] = json!([{"id":"live","name":"Live","query":{"request_id":"data-query"},"views":[{"id":"by-value","name":"By value","sql":"SELECT value FROM data WHERE value = ?"}]},{"id":"unused","name":"Unrelated","query":{"request_id":"unused-query"}}]);
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"W","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (status, response) = call(
        &router,
        "POST",
        "/api/execute",
        None,
        Some(json!({"workspace_id":"w","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], 200);
    let (_, history) = call(&router, "GET", "/api/workspaces/w/history", None, None).await;
    assert!(!history.to_string().contains("live-private"), "{history}");
    fixture.abort();
}

#[tokio::test]
async fn edited_workspace_cannot_send_after_caught_live_dataset_refresh_failure() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let started = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let calls = Arc::new(AtomicUsize::new(0));
    let source_started = started.clone();
    let source_release = release.clone();
    let destination_calls = calls.clone();
    let (url, fixture) = serve(
        Router::new()
            .route(
                "/source",
                axum::routing::get(move || {
                    let started = source_started.clone();
                    let release = source_release.clone();
                    async move {
                        started.notify_one();
                        release.notified().await;
                        "value\nlive-private\n"
                    }
                }),
            )
            .route(
                "/destination",
                axum::routing::get(move || {
                    let calls = destination_calls.clone();
                    async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        "sent"
                    }
                }),
            ),
    )
    .await;
    let dir = tempfile::tempdir().unwrap();
    let router = local(&dir.path().join("script-live-revision.db"))
        .await
        .unwrap();
    let mut data = example_data();
    data["script_mode"] = json!("v2");
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = json!(format!("{url}/destination"));
    request["pre_request_script"] = json!(
        "try { await pm.datasets('live').executeQuery('SELECT value FROM data'); } catch (error) { pm.test('caught',()=>pm.expect(error.message).to.equal('Live dataset refresh failed')); }"
    );
    let request = request.clone();
    let mut source = request.clone();
    source["id"] = json!("data-query");
    source["url"] = json!(format!("{url}/source"));
    source["pre_request_script"] = json!("");
    source["protocol"] = json!({"kind":"data","source":"remote_file","file_name":"rows.csv","file_format":"csv","table_name":"data","sql":"SELECT value FROM data","read_only":true});
    data["collections"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"sources","name":"Sources","description":"","requests":[source]}));
    data["datasets"] = json!([{"id":"live","name":"Live","query":{"request_id":"data-query"}}]);
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
    let running = tokio::spawn(async move {
        call(
            &running_router,
            "POST",
            "/api/execute",
            None,
            Some(json!({"workspace_id":"w","request":request})),
        )
        .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(2), started.notified())
        .await
        .unwrap();
    let (status, updated) = call(
        &router,
        "PUT",
        "/api/workspaces/w",
        None,
        Some(json!({"name":"Updated","expected_revision":saved["revision"],"data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    release.notify_one();
    let (status, response) = running.await.unwrap();
    assert_eq!(status, StatusCode::CONFLICT, "{response}");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    fixture.abort();
}

#[cfg(unix)]
#[tokio::test]
async fn edited_literal_dataset_cannot_send_after_script_worker_starts() {
    use std::{
        os::unix::fs::PermissionsExt,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let destination_calls = calls.clone();
    let (url, fixture) = serve(Router::new().route(
        "/destination",
        axum::routing::get(move || {
            let calls = destination_calls.clone();
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                "sent"
            }
        }),
    ))
    .await;
    let dir = tempfile::tempdir().unwrap();
    let ready = dir.path().join("worker-ready");
    let release = dir.path().join("worker-release");
    let wrapper = dir.path().join("worker-wrapper");
    let real_worker = env!("CARGO_BIN_EXE_moleapi-server");
    std::fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\ntouch '{}'\nwhile [ ! -f '{}' ]; do sleep 0.01; done\nexec '{}' \"$@\"\n",
            ready.display(),
            release.display(),
            real_worker
        ),
    )
    .unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700)).unwrap();
    let router =
        moleapi_server::local_with_worker(&dir.path().join("literal-revision.db"), &wrapper)
            .await
            .unwrap();
    let mut data = example_data();
    data["script_mode"] = json!("v2");
    data["datasets"] = json!([{"id":"saved","name":"Rows","source":{"format":"json","source":"[{\"token\":\"old\"}]"}}]);
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = json!(format!("{url}/destination"));
    request["pre_request_script"] = json!(
        "const result=await pm.datasets('saved').executeQuery('SELECT token FROM data');for await(const row of result.rows)pm.request.headers.upsert({key:'x-dataset',value:row.token});"
    );
    let request = request.clone();
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
    let running = tokio::spawn(async move {
        call(
            &running_router,
            "POST",
            "/api/execute",
            None,
            Some(json!({"workspace_id":"w","request":request})),
        )
        .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while !ready.exists() {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    data["datasets"][0]["source"]["source"] = json!("[{\"token\":\"new\"}]");
    let (status, updated) = call(
        &router,
        "PUT",
        "/api/workspaces/w",
        None,
        Some(json!({"name":"W","expected_revision":saved["revision"],"data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    std::fs::write(release, b"").unwrap();
    let (status, response) = running.await.unwrap();
    assert_eq!(status, StatusCode::CONFLICT, "{response}");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    fixture.abort();
}
