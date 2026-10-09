mod common;
use common::*;
use std::sync::{Arc, Mutex};
async fn events(router: &Router, count: usize) -> Vec<Value> {
    tokio::time::timeout(std::time::Duration::from_secs(6), async {
        loop {
            let (_, rows) = call(
                router,
                "GET",
                "/api/workspaces/w/notification-deliveries",
                None,
                None,
            )
            .await;
            let sent = rows
                .as_array()
                .unwrap()
                .iter()
                .filter(|row| row["status"] == "sent")
                .cloned()
                .collect::<Vec<_>>();
            if sent.len() >= count {
                return sent;
            }
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        }
    })
    .await
    .unwrap()
}
#[tokio::test]
async fn scenario_defaults_interactive_override_and_ci_events_reuse_owned_report_commit() {
    let captured = Arc::new(Mutex::new(Vec::<Value>::new()));
    let receiver = captured.clone();
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
            let captured = receiver.clone();
            async move {
                captured.lock().unwrap().push(body);
                StatusCode::NO_CONTENT
            }
        }),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("run-events.db")).await.unwrap();
    let target_body = json!({"settings":{"name":"Runs","kind":"webhook","enabled":true,"statuses":["passed","failed","cancelled"],"changes_only":false,"language":"en"},"credentials":{"endpoint":url}});
    let mut data = example_data();
    data["collections"][0]["requests"][0]["pre_request_script"] =
        "pm.execution.skipRequest();".into();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"Events","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, target) = call(
        &router,
        "POST",
        "/api/workspaces/w/notifications",
        None,
        Some(target_body),
    )
    .await;
    let target = target["id"].as_str().unwrap();
    data["scenarios"] = json!([{"id":"scenario","name":"Flow","collection_id":"c","notification_ids":[target],"steps":[{"id":"step","request_id":"r"}]}]);
    assert_eq!(
        call(
            &router,
            "PUT",
            "/api/workspaces/w",
            None,
            Some(json!({"name":"Events","data":data,"expected_revision":1}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, interactive) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        None,
        Some(json!({"collection_id":"c","scenario_id":"scenario"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{interactive}");
    events(&router, 1).await;
    let message = captured.lock().unwrap()[0].clone();
    assert_eq!(message["event"], "RUN_COMPLETED");
    assert_eq!(message["data"]["report_id"], interactive["report_id"]);
    let (_, suppressed) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        None,
        Some(json!({"collection_id":"c","scenario_id":"scenario","notification_ids":[]})),
    )
    .await;
    assert!(suppressed["report_id"].is_string());
    let (_, rows) = call(
        &router,
        "GET",
        "/api/workspaces/w/notification-deliveries",
        None,
        None,
    )
    .await;
    assert_eq!(rows.as_array().unwrap().len(), 1);
    let (_, ci) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        None,
        Some(json!({"collection_id":"c","notification_ids":[target],"run_origin":"ci"})),
    )
    .await;
    events(&router, 2).await;
    let messages = captured.lock().unwrap();
    let ci_message = messages
        .iter()
        .find(|message| message["event"] == "CI_RUN_COMPLETED")
        .unwrap();
    assert_eq!(ci_message["data"]["report_id"], ci["report_id"]);
    drop(messages);
    fixture.abort();
}
#[tokio::test]
async fn notification_target_validation_rejects_wrong_owner_before_network_and_failed_persistence_sends_nothing()
 {
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
    let requests = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = requests.clone();
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::get(move || {
            let requests = counter.clone();
            async move {
                requests.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                "ok"
            }
        }),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("owned.db");
    let mut config = config(format!("sqlite://{}?mode=rwc", path.display()), true);
    config.allow_private_network = true;
    let router = hosted(config).await.unwrap();
    let owner = register(&router, "event-owner").await;
    let other = register(&router, "event-other").await;
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = format!("{url}/").into();
    for token in [&owner, &other] {
        assert_eq!(
            call(
                &router,
                "POST",
                "/api/workspaces",
                Some(token),
                Some(json!({"id":"w","name":"Owned","data":data}))
            )
            .await
            .0,
            StatusCode::OK
        );
    }
    let(_,target)=call(&router,"POST","/api/workspaces/w/notifications",Some(&other),Some(json!({"settings":{"name":"Other","kind":"webhook","enabled":true,"statuses":["passed"],"changes_only":false,"language":"en"},"credentials":{"endpoint":url}}))).await;
    let id = target["id"].as_str().unwrap();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces/w/run",
            Some(&owner),
            Some(json!({"collection_id":"c","notification_ids":[id]}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(requests.load(std::sync::atomic::Ordering::SeqCst), 0);
    let db = Database::connect(format!("sqlite://{}?mode=rwc", path.display()))
        .await
        .unwrap();
    db.execute(Statement::from_string(DbBackend::Sqlite,"CREATE TRIGGER fail_report BEFORE INSERT ON documents WHEN NEW.kind='run-report' BEGIN SELECT RAISE(ABORT,'synthetic'); END;")).await.unwrap();
    let (_, run) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        Some(&other),
        Some(json!({"collection_id":"c","notification_ids":[id]})),
    )
    .await;
    assert!(run.get("report_save_error").is_some());
    let (_, rows) = call(
        &router,
        "GET",
        "/api/workspaces/w/notification-deliveries",
        Some(&other),
        None,
    )
    .await;
    assert!(rows.as_array().unwrap().is_empty());
    fixture.abort();
}
#[tokio::test]
async fn scheduled_scenarios_with_defaults_emit_only_the_scheduled_event() {
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::post(|| async { StatusCode::NO_CONTENT }),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("scheduled.db")).await.unwrap();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["pre_request_script"] =
        "pm.execution.skipRequest();".into();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"Scheduled","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let(_,target)=call(&router,"POST","/api/workspaces/w/notifications",None,Some(json!({"settings":{"name":"Runs","kind":"webhook","enabled":true,"statuses":["passed"],"changes_only":false,"language":"en"},"credentials":{"endpoint":url}}))).await;
    let id = target["id"].as_str().unwrap();
    data["scenarios"] = json!([{"id":"s","name":"Scenario","collection_id":"c","notification_ids":[id],"steps":[{"id":"a","request_id":"r"}]}]);
    assert_eq!(
        call(
            &router,
            "PUT",
            "/api/workspaces/w",
            None,
            Some(json!({"name":"Scheduled","data":data,"expected_revision":1}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let(_,schedule)=call(&router,"POST","/api/workspaces/w/schedules",None,Some(json!({"name":"Schedule","cron":"0 9 * * *","timezone":"UTC","enabled":false,"collection_id":"c","scenario_id":"s","notification_ids":[id]}))).await;
    assert_eq!(
        call(
            &router,
            "POST",
            &format!(
                "/api/workspaces/w/schedules/{}/run",
                schedule["id"].as_str().unwrap()
            ),
            None,
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    let deliveries = events(&router, 1).await;
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0]["event"]["kind"], "SCHEDULE_RUN_COMPLETED");
    fixture.abort();
}
#[tokio::test]
async fn run_notifications_use_runtime_privacy_projected_names_instead_of_raw_workspace_metadata() {
    let bodies = Arc::new(Mutex::new(Vec::<Value>::new()));
    let captured = bodies.clone();
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
            let captured = captured.clone();
            async move {
                captured.lock().unwrap().push(body);
                StatusCode::NO_CONTENT
            }
        }),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("privacy.db")).await.unwrap();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["pre_request_script"] =
        "pm.variables.set('issued','event-runtime-private');pm.execution.skipRequest();".into();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"event-runtime-private","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let(_,target)=call(&router,"POST","/api/workspaces/w/notifications",None,Some(json!({"settings":{"name":"Runtime","kind":"webhook","enabled":true,"statuses":["passed"],"changes_only":false,"language":"en"},"credentials":{"endpoint":url}}))).await;
    let (_, run) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        None,
        Some(json!({"collection_id":"c","notification_ids":[target["id"]]})),
    )
    .await;
    assert!(run["report_id"].is_string());
    events(&router, 1).await;
    assert!(
        !bodies.lock().unwrap()[0]
            .to_string()
            .contains("event-runtime-private")
    );
    fixture.abort();
}
