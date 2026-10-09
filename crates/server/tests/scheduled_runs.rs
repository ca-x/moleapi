mod common;
use common::*;
use std::sync::{Arc, Mutex};
fn definition(enabled: bool) -> Value {
    json!({"name":"Scheduled","cron":"* * * * * *","timezone":"UTC","enabled":enabled,"collection_id":"c","scenario_id":null,"environment_id":null,"dataset_id":null,"iterations":null})
}
async fn completed(router: &Router, id: &str) -> Value {
    let observed = Arc::new(Mutex::new(json!(null)));
    let capture = observed.clone();
    tokio::time::timeout(std::time::Duration::from_secs(8), async {
        loop {
            let (_, list) = call(router, "GET", "/api/workspaces/w/schedules", None, None).await;
            let schedule = list
                .as_array()
                .unwrap()
                .iter()
                .find(|schedule| schedule["id"] == id)
                .unwrap()
                .clone();
            *capture.lock().unwrap() = schedule.clone();
            if schedule["last_run"].is_object() {
                return schedule;
            }
            tokio::time::sleep(std::time::Duration::from_millis(40)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("Schedule did not complete: {}", observed.lock().unwrap()))
}
#[tokio::test]
async fn cron_executes_saved_scenario_environment_dataset_and_saves_a_private_report() {
    let observed = Arc::new(Mutex::new(Vec::<String>::new()));
    let calls = observed.clone();
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::get(
            move |headers: axum::http::HeaderMap,
                  axum::extract::Query(query): axum::extract::Query<
                std::collections::BTreeMap<String, String>,
            >| {
                let calls = calls.clone();
                async move {
                    assert_eq!(headers.get("x-token").unwrap(), "scheduled-private");
                    calls.lock().unwrap().push(query["value"].clone());
                    axum::Json(json!(query))
                }
            },
        ),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("cron.db")).await.unwrap();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = format!("{url}/?value={{{{value}}}}").into();
    data["collections"][0]["requests"][0]["headers"] =
        json!([{"id":"token","key":"X-Token","value":"{{token}}","enabled":true}]);
    data["environments"] = json!([{"id":"dev","name":"Dev","variables":[{"id":"token","key":"token","value":"scheduled-private","secret":true,"enabled":true}]}]);
    data["datasets"] = json!([{"id":"data","name":"Rows","source":{"format":"json","source":"[{\"value\":\"first\"},{\"value\":\"second\"}]"}}]);
    data["scenarios"] = json!([{"id":"s","name":"Scenario","collection_id":"c","steps":[{"id":"step","request_id":"r"}]}]);
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"Cron","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let mut input = definition(true);
    input["scenario_id"] = "s".into();
    input["environment_id"] = "dev".into();
    input["dataset_id"] = "data".into();
    let (status, schedule) = call(
        &router,
        "POST",
        "/api/workspaces/w/schedules",
        None,
        Some(input.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{schedule}");
    let id = schedule["id"].as_str().unwrap();
    let done = completed(&router, id).await;
    assert_eq!(done["last_run"]["status"], "passed", "{done}");
    assert_eq!(done["last_run"]["passed"], 2);
    assert!(!done.to_string().contains("scheduled-private"));
    input["enabled"] = false.into();
    assert_eq!(
        call(
            &router,
            "PUT",
            &format!("/api/workspaces/w/schedules/{id}"),
            None,
            Some(json!({"definition":input,"expected_revision":1}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let report = done["last_run"]["report_id"].as_str().unwrap();
    let (status, saved) = call(
        &router,
        "GET",
        &format!("/api/workspaces/w/reports/{report}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved["summary"]["iteration_count"], 2);
    assert_eq!(&observed.lock().unwrap()[..2], &["first", "second"]);
    fixture.abort();
}
#[tokio::test]
async fn competing_engines_claim_a_disabled_manual_run_once_and_editor_revision_is_independent() {
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = calls.clone();
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::get(move || {
            let calls = observed.clone();
            async move {
                calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                "ok"
            }
        }),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("shared.db");
    let router = local(&path).await.unwrap();
    let other = local(&path).await.unwrap();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = format!("{url}/").into();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"Manual","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, schedule) = call(
        &router,
        "POST",
        "/api/workspaces/w/schedules",
        None,
        Some(definition(false)),
    )
    .await;
    let id = schedule["id"].as_str().unwrap();
    assert_eq!(
        call(
            &router,
            "POST",
            &format!("/api/workspaces/w/schedules/{id}/run"),
            None,
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    let done = completed(&router, id).await;
    assert_eq!(done["last_run"]["manual"], true);
    assert_eq!(done["revision"], 1);
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    let (_, history) = call(
        &other,
        "GET",
        &format!("/api/workspaces/w/schedules/{id}/runs"),
        None,
        None,
    )
    .await;
    assert_eq!(history.as_array().unwrap().len(), 1);
    assert_eq!(history[0]["status"], "passed");
    let (status, _) = call(
        &router,
        "PUT",
        &format!("/api/workspaces/w/schedules/{id}"),
        None,
        Some(json!({"definition":definition(false),"expected_revision":1})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        call(
            &router,
            "PUT",
            &format!("/api/workspaces/w/schedules/{id}"),
            None,
            Some(json!({"definition":definition(false),"expected_revision":1}))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    drop(other);
    fixture.abort();
}
#[tokio::test]
async fn owned_schedule_preview_cancellation_and_workspace_cascade() {
    let temp = tempfile::tempdir().unwrap();
    let router = hosted(config(
        format!(
            "sqlite://{}?mode=rwc",
            temp.path().join("owned.db").display()
        ),
        true,
    ))
    .await
    .unwrap();
    let owner = register(&router, "schedule-owner").await;
    let other = register(&router, "schedule-other").await;
    let mut data = example_data();
    data["collections"][0]["requests"][0]["pre_request_script"] =
        "pm.execution.skipRequest();".into();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            Some(&owner),
            Some(json!({"id":"w","name":"Owned","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, schedule) = call(
        &router,
        "POST",
        "/api/workspaces/w/schedules",
        Some(&owner),
        Some(definition(false)),
    )
    .await;
    let id = schedule["id"].as_str().unwrap();
    for (method, path, body) in [
        ("GET", "".to_string(), None),
        ("POST", format!("/{id}/run"), None),
        ("POST", format!("/{id}/cancel"), None),
        (
            "PUT",
            format!("/{id}"),
            Some(json!({"definition":definition(false),"expected_revision":1})),
        ),
    ] {
        assert_eq!(
            call(
                &router,
                method,
                &format!("/api/workspaces/w/schedules{path}"),
                Some(&other),
                body
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
    }
    let (status, preview) = call(
        &router,
        "POST",
        "/api/workspaces/w/schedules/preview",
        Some(&owner),
        Some(json!({"cron":"0 9 * * *","timezone":"Asia/Shanghai"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["next"].as_array().unwrap().len(), 5);
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces/w/schedules/preview",
            Some(&owner),
            Some(json!({"cron":"invalid","timezone":"UTC"}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &router,
            "POST",
            &format!("/api/workspaces/w/schedules/{id}/run"),
            Some(&owner),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &router,
            "POST",
            &format!("/api/workspaces/w/schedules/{id}/cancel"),
            Some(&owner),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, history) = call(
        &router,
        "GET",
        &format!("/api/workspaces/w/schedules/{id}/runs"),
        Some(&owner),
        None,
    )
    .await;
    assert!(
        history
            .as_array()
            .unwrap()
            .iter()
            .any(|run| run["status"] == "cancelled")
    );
    assert_eq!(
        call(
            &router,
            "DELETE",
            "/api/workspaces/w",
            Some(&owner),
            Some(json!({"expected_revision":1}))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            Some(&owner),
            Some(json!({"id":"w","name":"Recreated","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, schedules) = call(
        &router,
        "GET",
        "/api/workspaces/w/schedules",
        Some(&owner),
        None,
    )
    .await;
    assert!(schedules.as_array().unwrap().is_empty());
}
#[tokio::test]
async fn queued_runs_survive_restart_and_expired_leases_are_recorded_without_replaying_old_jobs() {
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = calls.clone();
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::get(move || {
            let calls = observed.clone();
            async move {
                calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                "ok"
            }
        }),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("restart.db");
    let router = local(&path).await.unwrap();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = format!("{url}/").into();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"Restart","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, mut schedule) = call(
        &router,
        "POST",
        "/api/workspaces/w/schedules",
        None,
        Some(definition(false)),
    )
    .await;
    let id = schedule["id"].as_str().unwrap().to_string();
    assert_eq!(
        call(
            &router,
            "POST",
            &format!("/api/workspaces/w/schedules/{id}/run"),
            None,
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    drop(router);
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    let restarted = local(&path).await.unwrap();
    let done = completed(&restarted, &id).await;
    assert_eq!(done["last_run"]["status"], "passed");
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    drop(restarted);
    let now = chrono::Utc::now();
    let expired = uuid::Uuid::new_v4().to_string();
    let resumed = uuid::Uuid::new_v4().to_string();
    schedule["running"] = json!({"id":expired,"started_at":(now-chrono::Duration::seconds(900)).to_rfc3339(),"lease_until":(now-chrono::Duration::seconds(1)).to_rfc3339(),"config_revision":1,"manual":true,"cancel_requested":false});
    schedule["queued"] = json!({"id":resumed,"requested_at":now.to_rfc3339()});
    let db = Database::connect(format!("sqlite://{}?mode=rwc", path.display()))
        .await
        .unwrap();
    db.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "UPDATE documents SET payload = ?, revision = ? WHERE id = ?",
        [
            schedule.to_string().into(),
            (now.timestamp_millis() - 1000).into(),
            id.clone().into(),
        ],
    ))
    .await
    .unwrap();
    let restarted = local(&path).await.unwrap();
    let done = completed(&restarted, &id).await;
    assert_eq!(done["last_run"]["id"], resumed, "{done}");
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    let (_, history) = call(
        &restarted,
        "GET",
        &format!("/api/workspaces/w/schedules/{id}/runs"),
        None,
        None,
    )
    .await;
    assert!(
        history
            .as_array()
            .unwrap()
            .iter()
            .any(|run| run["id"] == expired && run["status"] == "interrupted")
    );
    assert!(
        !history
            .as_array()
            .unwrap()
            .iter()
            .any(|run| run["id"] == expired && run["status"] == "passed")
    );
    fixture.abort();
}
#[tokio::test]
async fn an_inflight_scheduled_request_can_be_cancelled_without_disabling_the_definition() {
    let started = Arc::new(tokio::sync::Notify::new());
    let signal = started.clone();
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::get(move || {
            let signal = signal.clone();
            async move {
                signal.notify_one();
                std::future::pending::<String>().await
            }
        }),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("cancel.db")).await.unwrap();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = format!("{url}/").into();
    data["collections"][0]["requests"][0]["timeout_ms"] = 10000.into();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"Cancel","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, schedule) = call(
        &router,
        "POST",
        "/api/workspaces/w/schedules",
        None,
        Some(definition(false)),
    )
    .await;
    let id = schedule["id"].as_str().unwrap();
    assert_eq!(
        call(
            &router,
            "POST",
            &format!("/api/workspaces/w/schedules/{id}/run"),
            None,
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    tokio::time::timeout(std::time::Duration::from_secs(3), started.notified())
        .await
        .unwrap();
    assert_eq!(
        call(
            &router,
            "POST",
            &format!("/api/workspaces/w/schedules/{id}/cancel"),
            None,
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    let done = completed(&router, id).await;
    assert_eq!(done["last_run"]["status"], "cancelled", "{done}");
    assert_eq!(done["definition"]["enabled"], false);
    assert_eq!(done["revision"], 1);
    fixture.abort();
}
#[tokio::test]
async fn hosted_enabled_tasks_continue_after_logout_and_check_owner_at_each_occurrence() {
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = calls.clone();
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::get(move || {
            let calls = observed.clone();
            async move {
                calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                "ok"
            }
        }),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let mut settings = config(
        format!(
            "sqlite://{}?mode=rwc",
            temp.path().join("logout.db").display()
        ),
        true,
    );
    settings.allow_private_network = true;
    let router = hosted(settings).await.unwrap();
    let owner = register(&router, "schedule-background").await;
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = format!("{url}/").into();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            Some(&owner),
            Some(json!({"id":"w","name":"Background","data":data}))
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
            Some(&owner),
            Some(definition(true))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&router, "POST", "/api/auth/logout", Some(&owner), None)
            .await
            .0,
        StatusCode::OK
    );
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while calls.load(std::sync::atomic::Ordering::SeqCst) == 0 {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    drop(router);
    fixture.abort();
}
