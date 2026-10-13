mod common;
use common::*;
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

#[tokio::test]
async fn scoped_credentials_are_owned_one_time_and_workspace_bound() {
    let temporary = tempfile::tempdir().unwrap();
    let url = format!(
        "sqlite://{}?mode=rwc",
        temporary.path().join("runner-credentials.db").display()
    );
    let first = hosted(config(url.clone(), true)).await.unwrap();
    let second = hosted(config(url.clone(), true)).await.unwrap();
    let owner = register(&first, "agentowner").await;
    let stranger = register(&first, "agentstranger").await;
    let (status, _) = call(
        &first,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"workspace","name":"Workspace","data":example_data()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, runner) = call(
        &first,
        "POST",
        "/api/runners",
        Some(&owner),
        Some(json!({"name":"LAN"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let runner_id = runner["id"].as_str().unwrap();
    let path = format!("/api/runners/{runner_id}/credentials");

    assert_eq!(
        call(
            &first,
            "POST",
            &path,
            Some(&stranger),
            Some(json!({"workspace_id":"workspace","name":"Foreign"}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&first, "GET", &path, Some(&stranger), None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &first,
            "POST",
            "/api/workspaces",
            Some(&stranger),
            Some(json!({"id":"foreign","name":"Foreign","data":example_data()}))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &first,
            "POST",
            &path,
            Some(&owner),
            Some(json!({"workspace_id":"foreign","name":"Foreign workspace"}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &first,
            "POST",
            &path,
            Some(&owner),
            Some(json!({"workspace_id":"missing","name":"Missing"}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, created) = call(
        &first,
        "POST",
        &path,
        Some(&owner),
        Some(json!({"workspace_id":"workspace","name":"LAN credential","expires_in_days":7})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let token = created["token"].as_str().unwrap();
    assert!(token.starts_with("moleapi_runner_"));
    assert_eq!(token.len(), 79);
    let (status, another_runner) = call(
        &first,
        "POST",
        "/api/runners",
        Some(&owner),
        Some(json!({"name":"Other runner"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{another_runner}");
    let another_claim = format!(
        "/api/runners/{}/claim",
        another_runner["id"].as_str().unwrap()
    );
    assert_eq!(
        call(&second, "POST", &another_claim, Some(token), None)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (_, listed) = call(&second, "GET", &path, Some(&owner), None).await;
    assert_eq!(listed[0]["id"], created["id"]);
    assert_eq!(listed[0]["workspace_id"], "workspace");
    assert!(!listed.to_string().contains(token));
    assert!(listed[0].get("digest").is_none());
    assert_eq!(
        call(
            &second,
            "GET",
            "/api/workspaces/workspace",
            Some(token),
            None
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(&second, "GET", &path, Some(token), None).await.0,
        StatusCode::FORBIDDEN
    );
    let (status, replacement) = call(
        &first,
        "POST",
        &path,
        Some(&owner),
        Some(json!({"workspace_id":"workspace","name":"Replacement"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{replacement}");
    let replacement_token = replacement["token"].as_str().unwrap();

    let (status, _) = call(
        &first,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"other","name":"Other","data":example_data()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, other_workspace) =
        call(&first, "GET", "/api/workspaces/other", Some(&owner), None).await;
    let (_, workspace) = call(
        &first,
        "GET",
        "/api/workspaces/workspace",
        Some(&owner),
        None,
    )
    .await;
    for (id, revision) in [
        ("other", &other_workspace["revision"]),
        ("workspace", &workspace["revision"]),
    ] {
        let (status, task) = call(&first, "POST", &format!("/api/workspaces/{id}/runner-tasks"), Some(&owner), Some(json!({"runner_id":runner_id,"expected_revision":revision,"selection":{"collection_id":"c"}}))).await;
        assert_eq!(status, StatusCode::OK, "{task}");
    }
    let claim_path = format!("/api/runners/{runner_id}/claim");
    assert_eq!(
        call(&second, "POST", &claim_path, Some(&owner), None)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, pat) = call(
        &first,
        "POST",
        "/api/auth/tokens",
        Some(&owner),
        Some(json!({"name":"Automation"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{pat}");
    let pat = pat["token"].as_str().unwrap();
    assert_eq!(
        call(&second, "POST", &claim_path, Some(pat), None).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(&second, "GET", &path, Some(pat), None).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &second,
            "POST",
            &path,
            Some(pat),
            Some(json!({"workspace_id":"workspace","name":"Escalation"}))
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (status, claim) = call(&second, "POST", &claim_path, Some(token), None).await;
    assert_eq!(status, StatusCode::OK, "{claim}");
    assert_eq!(claim["task"]["workspace_id"], "workspace");
    assert_eq!(
        call(&second, "POST", &claim_path, Some(token), None)
            .await
            .1,
        Value::Null
    );
    let task_id = claim["task"]["id"].as_str().unwrap();
    let heartbeat = format!("/api/runners/{runner_id}/tasks/{task_id}/heartbeat");
    assert_eq!(
        call(
            &second,
            "POST",
            &heartbeat,
            Some(token),
            Some(json!({"lease_token":claim["lease_token"]}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let complete = format!("/api/runners/{runner_id}/tasks/{task_id}/complete");
    assert_eq!(
        call(
            &second,
            "POST",
            &heartbeat,
            Some(&owner),
            Some(json!({"lease_token":claim["lease_token"]}))
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &second,
            "POST",
            &complete,
            Some(pat),
            Some(json!({"lease_token":claim["lease_token"],"status":"failed"}))
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &second,
            "POST",
            &heartbeat,
            Some(replacement_token),
            Some(json!({"lease_token":claim["lease_token"]}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &second,
            "POST",
            &complete,
            Some(replacement_token),
            Some(json!({"lease_token":claim["lease_token"],"status":"failed"}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );

    let database = Database::connect(&url).await.unwrap();
    let row = database
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT id, payload, ref_id FROM documents WHERE kind = 'runner-credential'",
            [],
        ))
        .await
        .unwrap()
        .unwrap();
    let id: String = row.try_get("", "id").unwrap();
    let payload: String = row.try_get("", "payload").unwrap();
    assert_eq!(row.try_get::<String>("", "ref_id").unwrap(), "workspace");
    assert!(!id.contains(token));
    assert!(!payload.contains(token));
    assert!(!payload.contains(&id));

    let revoke = format!("{path}/{}", created["id"].as_str().unwrap());
    assert_eq!(
        call(&second, "DELETE", &revoke, Some(&stranger), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&second, "DELETE", &revoke, Some(pat), None).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(&second, "DELETE", &revoke, Some(&owner), None).await.0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &first,
            "POST",
            &heartbeat,
            Some(token),
            Some(json!({"lease_token":claim["lease_token"]}))
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &first,
            "POST",
            &complete,
            Some(replacement_token),
            Some(json!({"lease_token":claim["lease_token"],"status":"failed"}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (_, tasks) = call(
        &first,
        "GET",
        "/api/workspaces/workspace/runner-tasks",
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(tasks[0]["status"], "cancelled");
    let (_, other_tasks) = call(
        &first,
        "GET",
        "/api/workspaces/other/runner-tasks",
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(other_tasks[0]["status"], "queued");
    let (_, workspace) = call(
        &first,
        "GET",
        "/api/workspaces/workspace",
        Some(&owner),
        None,
    )
    .await;
    let (status, next_task) = call(&first, "POST", "/api/workspaces/workspace/runner-tasks", Some(&owner), Some(json!({"runner_id":runner_id,"expected_revision":workspace["revision"],"selection":{"collection_id":"c"}}))).await;
    assert_eq!(status, StatusCode::OK, "{next_task}");
    let (status, next_claim) =
        call(&second, "POST", &claim_path, Some(replacement_token), None).await;
    assert_eq!(status, StatusCode::OK, "{next_claim}");
    assert_eq!(next_claim["task"]["id"], next_task["id"]);
    assert_eq!(
        call(&second, "POST", &claim_path, Some(token), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn scoped_credentials_enforce_expiry_quota_and_workspace_deletion() {
    let temporary = tempfile::tempdir().unwrap();
    let url = format!(
        "sqlite://{}?mode=rwc",
        temporary.path().join("credential-lifecycle.db").display()
    );
    let first = hosted(config(url.clone(), true)).await.unwrap();
    let second = hosted(config(url.clone(), true)).await.unwrap();
    let owner = register(&first, "credentiallifecycle").await;
    let (status, workspace) = call(
        &first,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Workspace","data":example_data()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{workspace}");
    let (status, runner) = call(
        &first,
        "POST",
        "/api/runners",
        Some(&owner),
        Some(json!({"name":"LAN"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{runner}");
    let path = format!(
        "/api/runners/{}/credentials",
        runner["id"].as_str().unwrap()
    );
    for invalid in [
        json!({"workspace_id":"w","name":""}),
        json!({"workspace_id":"w","name":" padded "}),
        json!({"workspace_id":"w","name":"x".repeat(81)}),
        json!({"workspace_id":"w","name":"valid","expires_in_days":0}),
        json!({"workspace_id":"w","name":"valid","expires_in_days":366}),
    ] {
        assert_eq!(
            call(&first, "POST", &path, Some(&owner), Some(invalid))
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    let (status, created) = call(
        &first,
        "POST",
        &path,
        Some(&owner),
        Some(json!({"workspace_id":"w","name":"Expires"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let bearer = created["token"].as_str().unwrap();
    let claim = format!("/api/runners/{}/claim", runner["id"].as_str().unwrap());
    for malformed in [
        format!("{bearer}x"),
        bearer.to_uppercase(),
        "moleapi_runner_zz".to_owned(),
    ] {
        let (status, error) = call(&second, "POST", &claim, Some(&malformed), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{error}");
        assert!(!error.to_string().contains(&malformed));
    }
    assert_eq!(
        call(&second, "GET", &claim, Some(bearer), None).await.0,
        StatusCode::FORBIDDEN
    );
    let database = Database::connect(&url).await.unwrap();
    let row = database
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT id,payload FROM documents WHERE kind = 'runner-credential'",
            [],
        ))
        .await
        .unwrap()
        .unwrap();
    let row_id: String = row.try_get("", "id").unwrap();
    let mut payload: Value =
        serde_json::from_str(&row.try_get::<String>("", "payload").unwrap()).unwrap();
    payload["expires_at"] = json!(0);
    database
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE documents SET payload = ? WHERE id = ?",
            [payload.to_string().into(), row_id.into()],
        ))
        .await
        .unwrap();
    assert_eq!(
        call(&second, "POST", &claim, Some(bearer), None).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (_, listed) = call(&first, "GET", &path, Some(&owner), None).await;
    assert_eq!(listed[0]["expired"], true);
    for number in 1..100 {
        let (status, result) = call(
            &first,
            "POST",
            &path,
            Some(&owner),
            Some(json!({"workspace_id":"w","name":format!("Agent {number}")})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{result}");
    }
    let (_, listed) = call(&second, "GET", &path, Some(&owner), None).await;
    assert_eq!(listed.as_array().unwrap().len(), 100);
    assert!(!listed.to_string().contains(bearer));
    assert_eq!(
        call(
            &first,
            "POST",
            &path,
            Some(&owner),
            Some(json!({"workspace_id":"w","name":"Overflow"}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &first,
            "DELETE",
            "/api/workspaces/w",
            Some(&owner),
            Some(json!({"expected_revision":workspace["revision"]}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, listed) = call(&second, "GET", &path, Some(&owner), None).await;
    assert_eq!(listed, json!([]));
    assert_eq!(
        call(&second, "POST", &claim, Some(bearer), None).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (status, recreated) = call(
        &second,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Recreated","data":example_data()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{recreated}");
    assert_eq!(
        call(&first, "POST", &claim, Some(bearer), None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &first,
            "POST",
            &path,
            Some(&owner),
            Some(json!({"workspace_id":"w","name":"Fresh"}))
        )
        .await
        .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn concurrent_claim_and_revoke_cannot_leave_a_live_lease() {
    let temporary = tempfile::tempdir().unwrap();
    let url = format!(
        "sqlite://{}?mode=rwc",
        temporary.path().join("claim-revoke.db").display()
    );
    let first = hosted(config(url.clone(), true)).await.unwrap();
    let second = hosted(config(url, true)).await.unwrap();
    let owner = register(&first, "claimrevoke").await;
    let (status, workspace) = call(
        &first,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Workspace","data":example_data()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{workspace}");
    let (status, runner) = call(
        &first,
        "POST",
        "/api/runners",
        Some(&owner),
        Some(json!({"name":"LAN"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{runner}");
    let runner_id = runner["id"].as_str().unwrap();
    let credential_path = format!("/api/runners/{runner_id}/credentials");
    let (status, credential) = call(
        &first,
        "POST",
        &credential_path,
        Some(&owner),
        Some(json!({"workspace_id":"w","name":"Race agent"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{credential}");
    let (status, task) = call(&first, "POST", "/api/workspaces/w/runner-tasks", Some(&owner), Some(json!({"runner_id":runner_id,"expected_revision":workspace["revision"],"selection":{"collection_id":"c"}}))).await;
    assert_eq!(status, StatusCode::OK, "{task}");
    let claim_path = format!("/api/runners/{runner_id}/claim");
    let revoke_path = format!("{credential_path}/{}", credential["id"].as_str().unwrap());
    let (claimed, revoked) = tokio::join!(
        call(
            &first,
            "POST",
            &claim_path,
            Some(credential["token"].as_str().unwrap()),
            None
        ),
        call(&second, "DELETE", &revoke_path, Some(&owner), None),
    );
    assert_eq!(revoked.0, StatusCode::OK, "{}", revoked.1);
    assert!(
        matches!(claimed.0, StatusCode::OK | StatusCode::UNAUTHORIZED),
        "{}",
        claimed.1
    );
    let (_, tasks) = call(
        &second,
        "GET",
        "/api/workspaces/w/runner-tasks",
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(tasks[0]["id"], task["id"]);
    assert!(
        matches!(tasks[0]["status"].as_str(), Some("queued" | "cancelled")),
        "{tasks}"
    );
    if claimed.0 == StatusCode::OK && claimed.1.is_object() {
        assert_eq!(
            tasks[0]["status"], "cancelled",
            "claim={claimed:?} revoke={revoked:?} tasks={tasks}"
        );
    }
    assert_eq!(
        call(
            &second,
            "POST",
            &claim_path,
            Some(credential["token"].as_str().unwrap()),
            None
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
}
