mod common;
use common::*;
use std::sync::{Arc, Mutex};
#[tokio::test]
async fn skipped_live_pre_scripts_cannot_open_a_connection() {
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = calls.clone();
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::get(move || {
            let count = count.clone();
            async move {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                "data: unexpected\n\n"
            }
        }),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("live-skip.db")).await.unwrap();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = format!("{url}/").into();
    data["collections"][0]["requests"][0]["protocol"] = json!({"kind":"sse"});
    data["collections"][0]["requests"][0]["pre_request_script"] =
        "pm.execution.skipRequest();".into();
    let (status, w) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"Live skip","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, result) = call(
        &router,
        "POST",
        "/api/sessions",
        None,
        Some(json!({"workspace_id":"w","request":w["data"]["collections"][0]["requests"][0]})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{result}");
    assert!(result.to_string().contains("skipped"));
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    fixture.abort();
}
#[tokio::test]
async fn unresolved_or_ambiguous_jump_targets_stop_without_echoing_private_values() {
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("targets.db")).await.unwrap();
    for (workspace, target, reason) in [
        ("missing", "private-target-name", "next_request_missing"),
        ("ambiguous", "Shared", "next_request_ambiguous"),
    ] {
        let mut data = example_data();
        let mut first = data["collections"][0]["requests"][0].clone();
        first["id"] = "first".into();
        first["pre_request_script"] =
            format!("pm.execution.setNextRequest('{target}');pm.execution.skipRequest();").into();
        let mut second = first.clone();
        second["id"] = "second".into();
        second["name"] = "Shared".into();
        let mut third = second.clone();
        third["id"] = "third".into();
        data["collections"][0]["requests"] = json!([first, second, third]);
        assert_eq!(
            call(
                &router,
                "POST",
                "/api/workspaces",
                None,
                Some(json!({"id":workspace,"name":"Targets","data":data}))
            )
            .await
            .0,
            StatusCode::OK
        );
        let (status, result) = call(
            &router,
            "POST",
            &format!("/api/workspaces/{workspace}/run"),
            None,
            Some(json!({"collection_id":"c"})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{result}");
        assert_eq!(result["stopped_reason"], reason);
        assert_eq!(result["skipped"], 1);
        assert_eq!(result["results"].as_array().unwrap().len(), 1);
        if workspace == "missing" {
            assert!(!result.to_string().contains(target));
        }
    }
}
#[tokio::test]
async fn infinite_skip_loops_are_stopped_by_the_shared_step_budget() {
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("quota.db")).await.unwrap();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["pre_request_script"] =
        "pm.execution.setNextRequest(pm.execution.location.current);pm.execution.skipRequest();"
            .into();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"Quota","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, result) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        None,
        Some(json!({"collection_id":"c"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{status}");
    assert_eq!(result["stopped_reason"], "step_limit");
    assert_eq!(result["skipped"], 1000);
    assert_eq!(result["passed"], 0);
    assert_eq!(result["executed_steps"], 1000);
    assert_eq!(result["results"].as_array().unwrap().len(), 1000);
}
#[tokio::test]
async fn scripts_jump_loop_skip_and_stop_without_sending_skipped_requests() {
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let log = seen.clone();
    let (url, fixture) = serve(Router::new().route(
        "/{name}",
        axum::routing::get(
            move |axum::extract::Path(name): axum::extract::Path<String>| {
                let log = log.clone();
                async move {
                    log.lock().unwrap().push(name);
                    "ok"
                }
            },
        ),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("flow.db")).await.unwrap();
    let mut data = example_data();
    let original = data["collections"][0]["requests"][0].clone();
    let make = |id: &str, name: &str, pre: &str, post: &str| {
        let mut request = original.clone();
        request["id"] = id.into();
        request["name"] = name.into();
        request["url"] = format!("{url}/{id}").into();
        request["pre_request_script"] = pre.into();
        request["post_response_script"] = post.into();
        request
    };
    data["collections"][0]["requests"] = json!([
        make("start", "Start", "", "pm.execution.setNextRequest('Loop');"),
        make("ignored", "Ignored", "", ""),
        make(
            "loop",
            "Loop",
            "",
            "let n=Number(pm.variables.get('n')||0)+1;pm.variables.set('n',n);pm.execution.setNextRequest(n<3?'loop':'skip');"
        ),
        make(
            "skip",
            "Skip",
            "pm.variables.set('fromSkip','yes');pm.execution.setNextRequest('end');pm.execution.skipRequest();",
            "throw new Error('must not run');"
        ),
        make(
            "end",
            "End",
            "pm.test('skip updates',()=>pm.expect(pm.variables.get('fromSkip')).to.equal('yes'));",
            "pm.execution.setNextRequest(null);"
        )
    ]);
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"Flow","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, result) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        None,
        Some(json!({"collection_id":"c","iterations":2})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["passed"], 8);
    assert_eq!(result["skipped"], 2);
    assert_eq!(result["failed"], 0);
    assert_eq!(result["completed_iterations"], 2);
    assert_eq!(result["iterations"][0]["script_stopped"], true);
    assert_eq!(result["iterations"][1]["script_stopped"], true);
    assert_eq!(
        *seen.lock().unwrap(),
        vec![
            "start", "loop", "loop", "loop", "end", "start", "loop", "end"
        ]
    );
    let (_, history) = call(&router, "GET", "/api/workspaces/w/history", None, None).await;
    assert_eq!(history.as_array().unwrap().len(), 8);
    assert_eq!(result["results"][4]["response"]["skipped"], true);
    fixture.abort();
}
