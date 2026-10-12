mod common;
use common::*;
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

async fn wait_schedule(router: &Router, token: &str, id: &str, terminal: bool) -> Value {
    tokio::time::timeout(std::time::Duration::from_secs(8), async {
        loop {
            let (_, rows) = call(
                router,
                "GET",
                "/api/workspaces/w/schedules",
                Some(token),
                None,
            )
            .await;
            let schedule = rows
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["id"] == id)
                .unwrap()
                .clone();
            if if terminal {
                schedule["last_run"].is_object()
            } else {
                schedule["running"]["runner_task_id"].is_string()
            } {
                return schedule;
            }
            tokio::time::sleep(std::time::Duration::from_millis(40)).await;
        }
    })
    .await
    .expect("Schedule state did not advance")
}

#[tokio::test]
async fn scheduled_runner_dispatch_is_atomic_across_instances_and_reconciles_report_once() {
    let temporary = tempfile::tempdir().unwrap();
    let url = format!(
        "sqlite://{}?mode=rwc",
        temporary.path().join("scheduled.db").display()
    );
    let first = hosted(config(url.clone(), true)).await.unwrap();
    let second = hosted(config(url, true)).await.unwrap();
    let token = register(&first, "scheduledowner").await;
    let (_, workspace) = call(
        &first,
        "POST",
        "/api/workspaces",
        Some(&token),
        Some(json!({"id":"w","name":"Scheduled workspace","data":example_data()})),
    )
    .await;
    let (_, runner) = call(
        &first,
        "POST",
        "/api/runners",
        Some(&token),
        Some(json!({"name":"LAN"})),
    )
    .await;
    let definition = json!({"name":"LAN schedule","cron":"0 9 * * *","timezone":"UTC","enabled":false,"collection_id":"c","runner_id":runner["id"]});
    let (status, schedule) = call(
        &first,
        "POST",
        "/api/workspaces/w/schedules",
        Some(&token),
        Some(definition),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{schedule}");
    let id = schedule["id"].as_str().unwrap();
    let path = format!("/api/workspaces/w/schedules/{id}/run");
    assert_eq!(
        call(&first, "POST", &path, Some(&token), None).await.0,
        StatusCode::OK
    );
    let running = wait_schedule(&first, &token, id, false).await;
    let task_id = running["running"]["runner_task_id"].as_str().unwrap();
    let (_, tasks) = call(
        &second,
        "GET",
        "/api/workspaces/w/runner-tasks",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(tasks.as_array().unwrap().len(), 1);
    assert_eq!(tasks[0]["id"], task_id);
    assert_eq!(tasks[0]["source_revision"], workspace["revision"]);
    assert_eq!(tasks[0]["max_attempts"], 1);
    let claim_path = format!("/api/runners/{}/claim", runner["id"].as_str().unwrap());
    let (left, right) = tokio::join!(
        call(&first, "POST", &claim_path, Some(&token), None),
        call(&second, "POST", &claim_path, Some(&token), None)
    );
    assert!(left.1.is_null() != right.1.is_null());
    let claim = if left.1.is_null() { right.1 } else { left.1 };
    let complete_path = format!(
        "/api/runners/{}/tasks/{task_id}/complete",
        runner["id"].as_str().unwrap()
    );
    let report = json!({"results":[{"request_id":"r","request_name":"Echo","response":{"status":200,"elapsed_ms":1,"size_bytes":2,"tests":[]}}],"passed":1,"failed":0,"skipped":0,"iteration_count":1});
    let (status, completed) = call(
        &second,
        "POST",
        &complete_path,
        Some(&token),
        Some(json!({"lease_token":claim["lease_token"],"report":report})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{completed}");
    let finished = wait_schedule(&first, &token, id, true).await;
    assert_eq!(finished["last_run"]["status"], "passed", "{finished}");
    assert_eq!(finished["last_run"]["report_id"], completed["report_id"]);
    assert_eq!(finished["last_run"]["passed"], 1);
    let (_, history) = call(
        &second,
        "GET",
        &format!("/api/workspaces/w/schedules/{id}/runs"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(history.as_array().unwrap().len(), 1);
    assert!(finished["running"].is_null());
}

#[tokio::test]
async fn scheduled_runner_cancel_and_delete_fence_tasks_and_reject_unavailable_targets() {
    let temporary = tempfile::tempdir().unwrap();
    let router = hosted(config(
        format!(
            "sqlite://{}?mode=rwc",
            temporary.path().join("cancel.db").display()
        ),
        true,
    ))
    .await
    .unwrap();
    let token = register(&router, "schedulecancel").await;
    let foreign = register(&router, "scheduleforeign").await;
    call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&token),
        Some(json!({"id":"w","name":"Workspace","data":example_data()})),
    )
    .await;
    let (_, runner) = call(
        &router,
        "POST",
        "/api/runners",
        Some(&token),
        Some(json!({"name":"LAN"})),
    )
    .await;
    let mut definition = json!({"name":"Task","cron":"0 9 * * *","timezone":"UTC","enabled":false,"collection_id":"c","runner_id":runner["id"]});
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces/w/schedules",
            Some(&foreign),
            Some(definition.clone())
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (_, schedule) = call(
        &router,
        "POST",
        "/api/workspaces/w/schedules",
        Some(&token),
        Some(definition.clone()),
    )
    .await;
    let id = schedule["id"].as_str().unwrap();
    let base = format!("/api/workspaces/w/schedules/{id}");
    call(&router, "POST", &format!("{base}/run"), Some(&token), None).await;
    let active = wait_schedule(&router, &token, id, false).await;
    call(
        &router,
        "POST",
        &format!("{base}/cancel"),
        Some(&token),
        None,
    )
    .await;
    let (_, tasks) = call(
        &router,
        "GET",
        "/api/workspaces/w/runner-tasks",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(
        tasks
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == active["running"]["runner_task_id"])
            .unwrap()["status"],
        "cancelled"
    );
    assert_eq!(
        wait_schedule(&router, &token, id, true).await["last_run"]["status"],
        "cancelled"
    );
    call(&router, "POST", &format!("{base}/run"), Some(&token), None).await;
    wait_schedule(&router, &token, id, false).await;
    assert_eq!(
        call(
            &router,
            "DELETE",
            &base,
            Some(&token),
            Some(json!({"expected_revision":1}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, tasks) = call(
        &router,
        "GET",
        "/api/workspaces/w/runner-tasks",
        Some(&token),
        None,
    )
    .await;
    assert!(
        tasks
            .as_array()
            .unwrap()
            .iter()
            .all(|task| task["status"] == "cancelled")
    );
    let runner_path = format!("/api/runners/{}", runner["id"].as_str().unwrap());
    assert_eq!(
        call(
            &router,
            "PATCH",
            &runner_path,
            Some(&token),
            Some(json!({"name":"LAN","enabled":false,"expected_revision":1}))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces/w/schedules",
            Some(&token),
            Some(definition.clone())
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    definition["runner_id"] = "missing".into();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces/w/schedules",
            Some(&token),
            Some(definition.clone())
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let local_router = local(&temporary.path().join("local.db")).await.unwrap();
    call(
        &local_router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"Local","data":example_data()})),
    )
    .await;
    assert_eq!(
        call(
            &local_router,
            "POST",
            "/api/workspaces/w/schedules",
            None,
            Some(definition)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn dispatched_jobs_survive_restart_and_expired_leases_fail_without_replay() {
    let temporary = tempfile::tempdir().unwrap();
    let url = format!(
        "sqlite://{}?mode=rwc",
        temporary.path().join("restart.db").display()
    );
    let router = hosted(config(url.clone(), true)).await.unwrap();
    let token = register(&router, "schedulerestart").await;
    call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&token),
        Some(json!({"id":"w","name":"Workspace","data":example_data()})),
    )
    .await;
    let (_, runner) = call(
        &router,
        "POST",
        "/api/runners",
        Some(&token),
        Some(json!({"name":"LAN"})),
    )
    .await;
    let definition = json!({"name":"Task","cron":"0 9 * * *","timezone":"UTC","enabled":false,"collection_id":"c","runner_id":runner["id"]});
    let (_, schedule) = call(
        &router,
        "POST",
        "/api/workspaces/w/schedules",
        Some(&token),
        Some(definition),
    )
    .await;
    let id = schedule["id"].as_str().unwrap();
    call(
        &router,
        "POST",
        &format!("/api/workspaces/w/schedules/{id}/run"),
        Some(&token),
        None,
    )
    .await;
    let dispatched = wait_schedule(&router, &token, id, false).await;
    let task_id = dispatched["running"]["runner_task_id"].as_str().unwrap();
    drop(router);
    let restarted = hosted(config(url.clone(), true)).await.unwrap();
    let claim_path = format!("/api/runners/{}/claim", runner["id"].as_str().unwrap());
    let (_, claim) = call(&restarted, "POST", &claim_path, Some(&token), None).await;
    assert_eq!(claim["task"]["id"], task_id);
    let db = Database::connect(&url).await.unwrap();
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT payload FROM documents WHERE id = ?",
            [task_id.into()],
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
        [payload.to_string().into(), task_id.into()],
    ))
    .await
    .unwrap();
    let terminal = wait_schedule(&restarted, &token, id, true).await;
    assert_eq!(terminal["last_run"]["status"], "failed", "{terminal}");
    assert_eq!(
        call(&restarted, "POST", &claim_path, Some(&token), None)
            .await
            .1,
        Value::Null
    );
    let (_, tasks) = call(
        &restarted,
        "GET",
        "/api/workspaces/w/runner-tasks",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(tasks.as_array().unwrap().len(), 1);
    assert_eq!(tasks[0]["status"], "failed");
    assert_eq!(tasks[0]["attempt"], 1);
    let heartbeat = format!(
        "/api/runners/{}/tasks/{task_id}/heartbeat",
        runner["id"].as_str().unwrap()
    );
    assert_eq!(
        call(
            &restarted,
            "POST",
            &heartbeat,
            Some(&token),
            Some(json!({"lease_token":claim["lease_token"]}))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn cron_dispatches_remotely_and_edits_atomically_cancel_leased_work() {
    let temporary = tempfile::tempdir().unwrap();
    let router = hosted(config(
        format!(
            "sqlite://{}?mode=rwc",
            temporary.path().join("cron.db").display()
        ),
        true,
    ))
    .await
    .unwrap();
    let token = register(&router, "schedulecron").await;
    let stranger = register(&router, "foreignrunner").await;
    call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&token),
        Some(json!({"id":"w","name":"Workspace","data":example_data()})),
    )
    .await;
    let (_, runner) = call(
        &router,
        "POST",
        "/api/runners",
        Some(&token),
        Some(json!({"name":"LAN"})),
    )
    .await;
    let (_, foreign_runner) = call(
        &router,
        "POST",
        "/api/runners",
        Some(&stranger),
        Some(json!({"name":"Foreign LAN"})),
    )
    .await;
    let mut definition = json!({"name":"Cron LAN","cron":"* * * * * *","timezone":"UTC","enabled":true,"collection_id":"c","runner_id":foreign_runner["id"]});
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces/w/schedules",
            Some(&token),
            Some(definition.clone())
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    definition["runner_id"] = runner["id"].clone();
    let (status, schedule) = call(
        &router,
        "POST",
        "/api/workspaces/w/schedules",
        Some(&token),
        Some(definition.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let id = schedule["id"].as_str().unwrap();
    let active = wait_schedule(&router, &token, id, false).await;
    assert_eq!(active["running"]["manual"], false);
    let claim_path = format!("/api/runners/{}/claim", runner["id"].as_str().unwrap());
    let (_, claim) = call(&router, "POST", &claim_path, Some(&token), None).await;
    assert_eq!(claim["task"]["id"], active["running"]["runner_task_id"]);
    definition["enabled"] = false.into();
    definition["runner_id"] = Value::Null;
    let (status, edited) = call(
        &router,
        "PUT",
        &format!("/api/workspaces/w/schedules/{id}"),
        Some(&token),
        Some(json!({"expected_revision":1,"definition":definition})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{edited}");
    let (_, tasks) = call(
        &router,
        "GET",
        "/api/workspaces/w/runner-tasks",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(tasks[0]["status"], "cancelled");
    let complete_path = format!(
        "/api/runners/{}/tasks/{}/complete",
        runner["id"].as_str().unwrap(),
        claim["task"]["id"].as_str().unwrap()
    );
    let (_, completion) = call(
        &router,
        "POST",
        &complete_path,
        Some(&token),
        Some(json!({"lease_token":claim["lease_token"],"status":"failed"})),
    )
    .await;
    assert_eq!(completion["status"], "cancelled");
    let terminal = wait_schedule(&router, &token, id, true).await;
    assert_eq!(terminal["last_run"]["status"], "cancelled");
    assert_eq!(terminal["last_run"]["config_revision"], 1);
    assert_eq!(terminal["revision"], 2);
}
