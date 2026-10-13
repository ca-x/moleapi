mod common;
use common::*;

async fn queue_label(
    router: &Router,
    token: &str,
    revision: &Value,
    label: &str,
) -> (StatusCode, Value) {
    call(
        router,
        "POST",
        "/api/workspaces/w/runner-tasks",
        Some(token),
        Some(json!({"runner_label":label,"expected_revision":revision,"selection":{"collection_id":"c"}})),
    )
    .await
}

#[tokio::test]
async fn label_group_selects_least_loaded_owned_runner_and_disabling_cancels_bound_tasks() {
    let temporary = tempfile::tempdir().unwrap();
    let url = format!(
        "sqlite://{}?mode=rwc",
        temporary.path().join("label-targets.db").display()
    );
    let first = hosted(config(url.clone(), true)).await.unwrap();
    let second = hosted(config(url, true)).await.unwrap();
    let owner = register(&first, "labeltargetowner").await;
    let stranger = register(&first, "labeltargetstranger").await;
    let (status, workspace) = call(
        &first,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Workspace","data":example_data()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{workspace}");
    let (_, foreign) = call(
        &first,
        "POST",
        "/api/runners",
        Some(&stranger),
        Some(json!({"name":"Foreign","labels":["lan"]})),
    )
    .await;
    let mut runners = Vec::new();
    for name in ["First", "Second"] {
        let (status, runner) = call(
            &first,
            "POST",
            "/api/runners",
            Some(&owner),
            Some(json!({"name":name,"labels":["lan"]})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{runner}");
        runners.push(runner);
    }
    runners.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    let path = "/api/workspaces/w/runner-tasks";
    for body in [
        json!({"expected_revision":workspace["revision"],"selection":{"collection_id":"c"}}),
        json!({"runner_id":runners[0]["id"],"runner_label":"lan","expected_revision":workspace["revision"],"selection":{"collection_id":"c"}}),
        json!({"runner_label":"LAN","expected_revision":workspace["revision"],"selection":{"collection_id":"c"}}),
        json!({"runner_label":"missing","expected_revision":workspace["revision"],"selection":{"collection_id":"c"}}),
    ] {
        assert_eq!(
            call(&first, "POST", path, Some(&owner), Some(body)).await.0,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(call(&first, "POST", path, Some(&owner), Some(json!({"runner_id":foreign["id"],"expected_revision":workspace["revision"],"selection":{"collection_id":"c"}}))).await.0, StatusCode::NOT_FOUND);
    let (left, right) = tokio::join!(
        queue_label(&first, &owner, &workspace["revision"], "lan"),
        queue_label(&second, &owner, &workspace["revision"], "lan")
    );
    assert_eq!(left.0, StatusCode::OK, "{}", left.1);
    assert_eq!(right.0, StatusCode::OK, "{}", right.1);
    let mut chosen = [
        left.1["runner_id"].as_str().unwrap(),
        right.1["runner_id"].as_str().unwrap(),
    ];
    chosen.sort();
    assert_eq!(
        chosen,
        [
            runners[0]["id"].as_str().unwrap(),
            runners[1]["id"].as_str().unwrap()
        ]
    );
    assert_eq!(left.1["target_label"], "lan");
    assert_eq!(right.1["target_label"], "lan");
    let (status, third) = queue_label(&first, &owner, &workspace["revision"], "lan").await;
    assert_eq!(status, StatusCode::OK, "{third}");
    assert_eq!(third["runner_id"], runners[0]["id"]);
    let first_id = runners[0]["id"].as_str().unwrap();
    let credential = runner_credential(&first, &owner, first_id, "w").await;
    let (status, claim) = call(
        &second,
        "POST",
        &format!("/api/runners/{first_id}/claim"),
        Some(&credential),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{claim}");
    assert_eq!(claim["task"]["runner_id"], first_id);
    assert_eq!(claim["task"]["target_label"], "lan");
    let (status, _) = call(
        &first,
        "PATCH",
        &format!("/api/runners/{first_id}"),
        Some(&owner),
        Some(json!({"name":runners[0]["name"],"enabled":false,"expected_revision":1})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, tasks) = call(&second, "GET", path, Some(&owner), None).await;
    let rows = tasks.as_array().unwrap();
    assert_eq!(
        rows.iter()
            .filter(|task| task["runner_id"] == first_id && task["status"] == "cancelled")
            .count(),
        2
    );
    assert_eq!(
        rows.iter()
            .filter(|task| task["runner_id"] == runners[1]["id"] && task["status"] == "queued")
            .count(),
        1
    );
    let (status, next) = queue_label(&second, &owner, &workspace["revision"], "lan").await;
    assert_eq!(status, StatusCode::OK, "{next}");
    assert_eq!(next["runner_id"], runners[1]["id"]);
}

#[tokio::test]
async fn schedule_label_targets_stay_remote_and_missing_groups_never_fall_back() {
    let temporary = tempfile::tempdir().unwrap();
    let url = format!(
        "sqlite://{}?mode=rwc",
        temporary.path().join("label-schedule.db").display()
    );
    let first = hosted(config(url.clone(), true)).await.unwrap();
    let second = hosted(config(url, true)).await.unwrap();
    let owner = register(&first, "labelscheduleowner").await;
    call(
        &first,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Workspace","data":example_data()})),
    )
    .await;
    let (_, runner) = call(
        &first,
        "POST",
        "/api/runners",
        Some(&owner),
        Some(json!({"name":"LAN","labels":["lan"]})),
    )
    .await;
    let definition = json!({"name":"Group schedule","cron":"0 9 * * *","timezone":"UTC","enabled":false,"collection_id":"c","runner_label":"lan"});
    let (status, schedule) = call(
        &first,
        "POST",
        "/api/workspaces/w/schedules",
        Some(&owner),
        Some(definition.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{schedule}");
    assert_eq!(schedule["definition"]["runner_label"], "lan");
    assert_eq!(schedule["definition"]["runner_id"], Value::Null);
    let mut conflicting = definition;
    conflicting["runner_id"] = runner["id"].clone();
    assert_eq!(
        call(
            &first,
            "POST",
            "/api/workspaces/w/schedules",
            Some(&owner),
            Some(conflicting)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let id = schedule["id"].as_str().unwrap();
    let run_path = format!("/api/workspaces/w/schedules/{id}/run");
    assert_eq!(
        call(&first, "POST", &run_path, Some(&owner), None).await.0,
        StatusCode::OK
    );
    let task = tokio::time::timeout(std::time::Duration::from_secs(8), async {
        loop {
            let (_, tasks) = call(
                &second,
                "GET",
                "/api/workspaces/w/runner-tasks",
                Some(&owner),
                None,
            )
            .await;
            if let Some(task) = tasks.as_array().and_then(|rows| rows.first()) {
                return task.clone();
            }
            tokio::time::sleep(std::time::Duration::from_millis(40)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(task["runner_id"], runner["id"]);
    assert_eq!(task["target_label"], "lan");
    let runner_id = runner["id"].as_str().unwrap();
    assert_eq!(
        call(
            &second,
            "PATCH",
            &format!("/api/runners/{runner_id}"),
            Some(&owner),
            Some(json!({"name":"LAN","enabled":false,"expected_revision":1}))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        queue_label(&second, &owner, &json!(1), "lan").await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(&first, "POST", &run_path, Some(&owner), None).await.0,
        StatusCode::BAD_REQUEST
    );
    let (_, tasks) = call(
        &first,
        "GET",
        "/api/workspaces/w/runner-tasks",
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(tasks.as_array().unwrap().len(), 1);
    assert_ne!(tasks[0]["status"], "completed");
}
