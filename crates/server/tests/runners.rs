mod common;
use common::*;
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
async fn setup() -> (
    tempfile::TempDir,
    String,
    Router,
    Router,
    String,
    String,
    String,
    Value,
) {
    let temporary = tempfile::tempdir().unwrap();
    let url = format!(
        "sqlite://{}?mode=rwc",
        temporary.path().join("runners.db").display()
    );
    let a = hosted(config(url.clone(), true)).await.unwrap();
    let b = hosted(config(url.clone(), true)).await.unwrap();
    let owner = register(&a, "runnerowner").await;
    let stranger = register(&a, "runnerstranger").await;
    let mut data = example_data();
    data["global_variables"] = json!([{"id":"secret","key":"secret","value":"saved-runner-secret","enabled":true,"secret":true}]);
    let (status, workspace) = call(
        &a,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Runner workspace","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{workspace}");
    let (status, runner) = call(
        &a,
        "POST",
        "/api/runners",
        Some(&owner),
        Some(json!({"name":"Private LAN"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{runner}");
    (
        temporary,
        url,
        a,
        b,
        owner,
        stranger,
        runner["id"].as_str().unwrap().into(),
        workspace,
    )
}
async fn queue(
    router: &Router,
    owner: &str,
    runner: &str,
    revision: &Value,
    attempts: usize,
) -> Value {
    let (status, task) = call(router,"POST","/api/workspaces/w/runner-tasks",Some(owner),Some(json!({"runner_id":runner,"expected_revision":revision,"selection":{"collection_id":"c"},"max_attempts":attempts}))).await;
    assert_eq!(status, StatusCode::OK, "{task}");
    task
}
async fn expire(db: &sea_orm::DatabaseConnection, id: &str) {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT payload FROM documents WHERE id = ?",
            [id.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    let mut payload: Value =
        serde_json::from_str(&row.try_get::<String>("", "payload").unwrap()).unwrap();
    payload["summary"]["lease_until"] = json!("2000-01-01T00:00:00Z");
    db.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "UPDATE documents SET revision = 0, payload = ? WHERE id = ?",
        [payload.to_string().into(), id.into()],
    ))
    .await
    .unwrap();
}
#[tokio::test]
async fn claims_are_exclusive_owned_private_and_completion_is_atomic_idempotent() {
    let (_temp, _url, a, b, owner, stranger, runner, w) = setup().await;
    let task = queue(&a, &owner, &runner, &w["revision"], 1).await;
    assert!(!task.to_string().contains("saved-runner-secret"));
    assert_eq!(
        call(
            &a,
            "POST",
            &format!("/api/runners/{runner}/claim"),
            Some(&stranger),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let claim_path = format!("/api/runners/{runner}/claim");
    let (x, y) = tokio::join!(
        call(&a, "POST", &claim_path, Some(&owner), None),
        call(&b, "POST", &claim_path, Some(&owner), None)
    );
    assert_eq!(x.0, StatusCode::OK);
    assert_eq!(y.0, StatusCode::OK);
    assert!(x.1.is_null() != y.1.is_null());
    let claim = if x.1.is_null() { y.1 } else { x.1 };
    assert!(
        claim["workspace"]
            .to_string()
            .contains("saved-runner-secret")
    );
    let (_, metadata) = call(
        &a,
        "GET",
        "/api/workspaces/w/runner-tasks",
        Some(&owner),
        None,
    )
    .await;
    assert!(!metadata.to_string().contains("saved-runner-secret"));
    assert!(
        !metadata
            .to_string()
            .contains(claim["lease_token"].as_str().unwrap())
    );
    let task_id = task["id"].as_str().unwrap();
    let heartbeat = format!("/api/runners/{runner}/tasks/{task_id}/heartbeat");
    assert_eq!(
        call(
            &b,
            "POST",
            &heartbeat,
            Some(&owner),
            Some(json!({"lease_token":"bad"}))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(
            &b,
            "POST",
            &heartbeat,
            Some(&owner),
            Some(json!({"lease_token":claim["lease_token"]}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let complete_path = format!("/api/runners/{runner}/tasks/{task_id}/complete");
    let completion = json!({"lease_token":claim["lease_token"],"report":{"passed":1,"failed":0,"skipped":0,"executed_steps":1,"results":[{"request_id":"r","request_name":"saved-runner-secret","history_id":"worker-local-history","status":200,"passed":true,"tests_passed":1,"tests_failed":0,"response":{"tests":[{"id":"a","name":"saved-runner-secret","passed":true,"actual":"yes","expected":"yes"}]}}]}});
    let (status, completed) = call(
        &b,
        "POST",
        &complete_path,
        Some(&owner),
        Some(completion.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{completed}");
    assert_eq!(completed["status"], "completed");
    let (status, replayed) = call(&a, "POST", &complete_path, Some(&owner), Some(completion)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(completed, replayed);
    let (_, reports) = call(&a, "GET", "/api/workspaces/w/reports", Some(&owner), None).await;
    assert_eq!(reports["items"].as_array().unwrap().len(), 1, "{reports}");
    let (_, report) = call(
        &a,
        "GET",
        &format!(
            "/api/workspaces/w/reports/{}",
            completed["report_id"].as_str().unwrap()
        ),
        Some(&owner),
        None,
    )
    .await;
    assert!(!report.to_string().contains("saved-runner-secret"));
    assert!(!report.to_string().contains("worker-local-history"));
    assert_eq!(
        call(
            &a,
            "GET",
            "/api/workspaces/w/runner-tasks",
            Some(&stranger),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}
#[tokio::test]
async fn leases_retry_only_when_configured_and_cancel_disable_or_revoke_stops_work() {
    let (_temp, url, a, b, owner, _stranger, runner, w) = setup().await;
    let task = queue(&a, &owner, &runner, &w["revision"], 2).await;
    let path = format!("/api/runners/{runner}/claim");
    let (_, first) = call(&a, "POST", &path, Some(&owner), None).await;
    let id = task["id"].as_str().unwrap();
    let db = Database::connect(&url).await.unwrap();
    expire(&db, id).await;
    let (_, second) = call(&b, "POST", &path, Some(&owner), None).await;
    assert_eq!(second["task"]["attempt"], 2);
    assert_ne!(first["lease_token"], second["lease_token"]);
    let heartbeat = format!("/api/runners/{runner}/tasks/{id}/heartbeat");
    assert_eq!(
        call(
            &a,
            "POST",
            &heartbeat,
            Some(&owner),
            Some(json!({"lease_token":first["lease_token"]}))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(
            &a,
            "POST",
            &format!("/api/workspaces/w/runner-tasks/{id}/cancel"),
            Some(&owner),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &b,
            "POST",
            &heartbeat,
            Some(&owner),
            Some(json!({"lease_token":second["lease_token"]}))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let default_task = queue(&a, &owner, &runner, &w["revision"], 1).await;
    call(&a, "POST", &path, Some(&owner), None).await;
    expire(&db, default_task["id"].as_str().unwrap()).await;
    let (_, empty) = call(&b, "POST", &path, Some(&owner), None).await;
    assert!(empty.is_null());
    let (_, tasks) = call(
        &a,
        "GET",
        "/api/workspaces/w/runner-tasks",
        Some(&owner),
        None,
    )
    .await;
    assert!(
        tasks
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == default_task["id"] && t["status"] == "failed")
    );
    let queued = queue(&a, &owner, &runner, &w["revision"], 1).await;
    let (_, token) = call(
        &a,
        "POST",
        "/api/auth/tokens",
        Some(&owner),
        Some(json!({"name":"Revoked"})),
    )
    .await;
    assert_eq!(
        call(
            &a,
            "DELETE",
            &format!("/api/auth/tokens/{}", token["id"].as_str().unwrap()),
            Some(&owner),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, empty) = call(&b, "POST", &path, Some(&owner), None).await;
    assert!(empty.is_null());
    let (_, tasks) = call(
        &a,
        "GET",
        "/api/workspaces/w/runner-tasks",
        Some(&owner),
        None,
    )
    .await;
    assert!(
        tasks
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == queued["id"] && t["status"] == "cancelled")
    );
    let queued = queue(&a, &owner, &runner, &w["revision"], 1).await;
    let (_, claimed) = call(&b, "POST", &path, Some(&owner), None).await;
    assert_eq!(
        call(
            &a,
            "PATCH",
            &format!("/api/runners/{runner}"),
            Some(&owner),
            Some(json!({"name":"Disabled","enabled":false,"expected_revision":1}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let heartbeat = format!(
        "/api/runners/{runner}/tasks/{}/heartbeat",
        queued["id"].as_str().unwrap()
    );
    assert_eq!(
        call(
            &b,
            "POST",
            &heartbeat,
            Some(&owner),
            Some(json!({"lease_token":claimed["lease_token"]}))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn invalid_selection_source_changes_and_invalid_reports_leave_no_partial_records() {
    let (_temp, _url, a, _b, owner, _stranger, runner, w) = setup().await;
    for selection in [
        json!({"collection_id":"missing"}),
        json!({"collection_id":"c","environment_id":"missing"}),
        json!({"collection_id":"c","dataset_id":"missing"}),
        json!({"collection_id":"c","request_ids":["r","r"]}),
        json!({"collection_id":"c","request_ids":["outside"]}),
        json!({"collection_id":"c","iterations":101}),
    ] {
        let (status, _) = call(
            &a,
            "POST",
            "/api/workspaces/w/runner-tasks",
            Some(&owner),
            Some(
                json!({"runner_id":runner,"expected_revision":w["revision"],"selection":selection}),
            ),
        )
        .await;
        assert!(matches!(
            status,
            StatusCode::BAD_REQUEST | StatusCode::NOT_FOUND
        ));
    }
    let (_, tasks) = call(
        &a,
        "GET",
        "/api/workspaces/w/runner-tasks",
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(tasks, json!([]));
    let old = queue(&a, &owner, &runner, &w["revision"], 1).await;
    let (status, edited) = call(
        &a,
        "PUT",
        "/api/workspaces/w",
        Some(&owner),
        Some(json!({"name":"Edited","data":w["data"],"expected_revision":w["revision"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{edited}");
    let claim_path = format!("/api/runners/{runner}/claim");
    let (_, empty) = call(&a, "POST", &claim_path, Some(&owner), None).await;
    assert!(empty.is_null());
    let (_, tasks) = call(
        &a,
        "GET",
        "/api/workspaces/w/runner-tasks",
        Some(&owner),
        None,
    )
    .await;
    assert!(
        tasks
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == old["id"] && t["status"] == "cancelled")
    );
    let new = queue(&a, &owner, &runner, &edited["revision"], 1).await;
    let (_, claimed) = call(&a, "POST", &claim_path, Some(&owner), None).await;
    let complete = format!(
        "/api/runners/{runner}/tasks/{}/complete",
        new["id"].as_str().unwrap()
    );
    for report in [
        json!({"results":[{"request_id":"outside"}]}),
        json!({"results":[null]}),
        json!({"results":[{"request_id":"r","response":{"tests":[{"passed":true}]}}]}),
    ] {
        let (status, error) = call(
            &a,
            "POST",
            &complete,
            Some(&owner),
            Some(json!({"lease_token":claimed["lease_token"],"report":report})),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
    }
    let (_, reports) = call(&a, "GET", "/api/workspaces/w/reports", Some(&owner), None).await;
    assert_eq!(reports["items"], json!([]));
    let (status, finished) = call(
        &a,
        "POST",
        &complete,
        Some(&owner),
        Some(json!({"lease_token":claimed["lease_token"],"status":"failed"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{finished}");
    assert_eq!(finished["status"], "failed");
}

#[tokio::test]
async fn cancelling_a_task_prunes_terminal_metadata_to_the_latest_hundred() {
    let (_temp, url, a, _b, owner, _stranger, runner, w) = setup().await;
    let task = queue(&a, &owner, &runner, &w["revision"], 1).await;
    let db = Database::connect(&url).await.unwrap();
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT owner, payload FROM documents WHERE id = ?",
            [task["id"].as_str().unwrap().into()],
        ))
        .await
        .unwrap()
        .unwrap();
    let owner_id: String = row.try_get("", "owner").unwrap();
    let mut payload: Value =
        serde_json::from_str(&row.try_get::<String>("", "payload").unwrap()).unwrap();
    payload["summary"]["status"] = json!("cancelled");
    payload["summary"]["finished_at"] = json!("2000-01-01T00:00:00Z");
    for number in 0..100 {
        let id = format!("old-runner-task-{number}");
        payload["summary"]["id"] = json!(id);
        db.execute(Statement::from_sql_and_values(DbBackend::Sqlite,"INSERT INTO documents (id,owner,kind,ref_id,revision,payload,updated_at) VALUES (?,?,?,?,?,?,?)",[id.into(),owner_id.clone().into(),"runner-task".into(),"w".into(),i64::MAX.into(),payload.to_string().into(),"2000-01-01T00:00:00Z".into()])).await.unwrap();
    }
    let (status, _) = call(
        &a,
        "POST",
        &format!(
            "/api/workspaces/w/runner-tasks/{}/cancel",
            task["id"].as_str().unwrap()
        ),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, tasks) = call(
        &a,
        "GET",
        "/api/workspaces/w/runner-tasks",
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(tasks.as_array().unwrap().len(), 100);
    assert!(
        tasks
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == task["id"])
    );
}
