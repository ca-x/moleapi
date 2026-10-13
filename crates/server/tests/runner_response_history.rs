mod common;
use common::*;

fn response_entry(body: &str) -> Value {
    json!({
        "id":"local-history", "workspace_id":"w", "request_id":"r", "request_name":"Request",
        "method":"GET", "url":"https://example.com/", "status":200, "elapsed_ms":1,
        "size_bytes":body.len(), "created_at":"2026-10-13T00:00:00Z",
        "response":{"status":200,"status_text":"OK","headers":[],"body":body,"elapsed_ms":1,
            "size_bytes":body.len(),"truncated":false,"url":"https://example.com/","tests":[]}
    })
}

#[tokio::test]
async fn hosted_completion_reprojects_uploaded_history_and_commits_one_owned_link() {
    let temporary = tempfile::tempdir().unwrap();
    let url = format!(
        "sqlite://{}?mode=rwc",
        temporary.path().join("history.db").display()
    );
    let first = hosted(config(url.clone(), true)).await.unwrap();
    let second = hosted(config(url, true)).await.unwrap();
    let owner = register(&first, "historyuploadowner").await;
    let stranger = register(&first, "historyuploadstranger").await;
    let mut data = example_data();
    data["global_variables"] = json!([{"id":"secret","key":"secret","value":"server-secret-value","enabled":true,"secret":true}]);
    let (_, workspace) = call(
        &first,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Workspace","data":data})),
    )
    .await;
    let (_, runner) = call(
        &first,
        "POST",
        "/api/runners",
        Some(&owner),
        Some(json!({"name":"LAN"})),
    )
    .await;
    let runner_id = runner["id"].as_str().unwrap();
    let agent = runner_credential(&first, &owner, runner_id, "w").await;
    let (status, task) = call(&first, "POST", "/api/workspaces/w/runner-tasks", Some(&owner), Some(json!({"runner_id":runner_id,"expected_revision":workspace["revision"],"selection":{"collection_id":"c"}}))).await;
    assert_eq!(status, StatusCode::OK, "{task}");
    let (_, claim) = call(
        &second,
        "POST",
        &format!("/api/runners/{runner_id}/claim"),
        Some(&agent),
        None,
    )
    .await;
    let complete = format!(
        "/api/runners/{runner_id}/tasks/{}/complete",
        task["id"].as_str().unwrap()
    );
    let report = json!({"results":[{"request_id":"r","request_name":"Request","method":"GET","status":200,"elapsed_ms":1,"history_id":"local-history","response":{"status":200,"elapsed_ms":1,"size_bytes":19,"tests":[]}}],"passed":1,"failed":0,"skipped":0,"iteration_count":1,"history_uploads":[{"position":0,"entry":response_entry("server-secret-value")}]});
    for invalid in [
        {
            let mut value = report.clone();
            value["history_uploads"][0]["entry"]["workspace_id"] = json!("foreign");
            value
        },
        {
            let mut value = report.clone();
            value["history_uploads"][0]["entry"]["request_id"] = json!("outside");
            value
        },
        {
            let mut value = report.clone();
            value["history_uploads"][0]["position"] = json!(1);
            value
        },
        {
            let mut value = report.clone();
            let duplicate = value["history_uploads"][0].clone();
            value["history_uploads"]
                .as_array_mut()
                .unwrap()
                .push(duplicate);
            value
        },
        {
            let mut value = report.clone();
            value["history_uploads"][0]["entry"]["response"]["body"] =
                json!("x".repeat(8 * 1024 * 1024));
            value
        },
    ] {
        let (status, error) = call(
            &first,
            "POST",
            &complete,
            Some(&agent),
            Some(json!({"lease_token":claim["lease_token"],"report":invalid})),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
    }
    let (_, before) = call(
        &second,
        "GET",
        "/api/workspaces/w/history",
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(before, json!([]));
    let (status, completed) = call(
        &first,
        "POST",
        &complete,
        Some(&agent),
        Some(json!({"lease_token":claim["lease_token"],"report":report})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{completed}");
    let report_id = completed["report_id"].as_str().unwrap();
    let report_path = format!("/api/workspaces/w/reports/{report_id}");
    let (_, saved) = call(&second, "GET", &report_path, Some(&owner), None).await;
    let hosted_id = saved["results"][0]["history_id"].as_str().unwrap();
    assert_ne!(hosted_id, "local-history");
    let response_path = format!("{report_path}/steps/0/response");
    let (status, response) = call(&second, "GET", &response_path, Some(&owner), None).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["id"], hosted_id);
    assert!(!response.to_string().contains("server-secret-value"));
    assert!(
        response["response"]["body"]
            .as_str()
            .unwrap()
            .contains("[REDACTED]")
    );
    assert_eq!(
        call(&second, "GET", &response_path, Some(&stranger), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (status, again) = call(
        &second,
        "POST",
        &complete,
        Some(&agent),
        Some(json!({"lease_token":claim["lease_token"],"report":json!({"results":[]})})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(again["report_id"], completed["report_id"]);
    let (_, histories) = call(
        &first,
        "GET",
        "/api/workspaces/w/history",
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(histories.as_array().unwrap().len(), 1);
    assert_eq!(
        call(
            &first,
            "DELETE",
            "/api/workspaces/w/history",
            Some(&owner),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&second, "GET", &response_path, Some(&owner), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&second, "GET", &report_path, Some(&owner), None)
            .await
            .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn old_or_omitted_history_ids_do_not_survive_a_runner_completion() {
    let temporary = tempfile::tempdir().unwrap();
    let url = format!(
        "sqlite://{}?mode=rwc",
        temporary.path().join("omitted.db").display()
    );
    let router = hosted(config(url, true)).await.unwrap();
    let owner = register(&router, "historyomissionowner").await;
    let (_, workspace) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Workspace","data":example_data()})),
    )
    .await;
    let (_, runner) = call(
        &router,
        "POST",
        "/api/runners",
        Some(&owner),
        Some(json!({"name":"LAN"})),
    )
    .await;
    let runner_id = runner["id"].as_str().unwrap();
    let agent = runner_credential(&router, &owner, runner_id, "w").await;
    let (_, task) = call(&router, "POST", "/api/workspaces/w/runner-tasks", Some(&owner), Some(json!({"runner_id":runner_id,"expected_revision":workspace["revision"],"selection":{"collection_id":"c"}}))).await;
    let (_, claim) = call(
        &router,
        "POST",
        &format!("/api/runners/{runner_id}/claim"),
        Some(&agent),
        None,
    )
    .await;
    let report = json!({"results":[{"request_id":"r","history_id":"deleted-local-history","response":{"status":200,"elapsed_ms":1,"size_bytes":1,"tests":[]}}],"passed":1,"failed":0,"skipped":0,"iteration_count":1});
    let complete = format!(
        "/api/runners/{runner_id}/tasks/{}/complete",
        task["id"].as_str().unwrap()
    );
    let (status, completed) = call(
        &router,
        "POST",
        &complete,
        Some(&agent),
        Some(json!({"lease_token":claim["lease_token"],"report":report})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{completed}");
    let report_id = completed["report_id"].as_str().unwrap();
    let (_, saved) = call(
        &router,
        "GET",
        &format!("/api/workspaces/w/reports/{report_id}"),
        Some(&owner),
        None,
    )
    .await;
    assert!(saved["results"][0]["history_id"].is_null());
    assert_eq!(
        call(
            &router,
            "GET",
            &format!("/api/workspaces/w/reports/{report_id}/steps/0/response"),
            Some(&owner),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}
