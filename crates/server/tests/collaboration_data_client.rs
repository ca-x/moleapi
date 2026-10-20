mod common;
use base64::{Engine, engine::general_purpose::STANDARD};
use common::*;
use std::time::Duration;

async fn events_until(router: &Router, base: &str, token: &str, kind: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let (status, batch) = call(
                router,
                "GET",
                &format!("{base}/events?after=0"),
                Some(token),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{batch}");
            if batch["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["message"]["kind"] == kind)
            {
                return batch;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn shared_data_sessions_use_saved_owner_source_scripts_and_actor_fences() {
    let dir = tempfile::tempdir().unwrap();
    let router = moleapi_server::hosted_with_worker(
        config(
            moleapi_server::sqlite_database_url(&dir.path().join("data.db")).unwrap(),
            true,
        ),
        std::path::Path::new(env!("CARGO_BIN_EXE_moleapi-server")),
    )
    .await
    .unwrap();
    let owner = register(&router, "dataowner").await;
    let editor = register(&router, "dataeditor").await;
    let viewer = register(&router, "dataviewer").await;
    let other = register(&router, "dataother").await;
    let mut data = example_data();
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = "".into();
    request["protocol"] = json!({"kind":"data","source":"local_file","file_name":"rows.csv","file_format":"csv","file_base64":STANDARD.encode(b"id,name\n1,owner-first\n2,owner-second\n"),"table_name":"data","sql":"SELECT name FROM data","query_pre_script":"pm.request.body.update('SELECT name FROM data WHERE id=2');"});
    request["post_response_script"] =
        "pm.test('shared postprocessing',()=>pm.expect(pm.response.code).to.equal(200));".into();
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Data","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let mut base = String::new();
    for (token, role) in [(&editor, "editor"), (&viewer, "viewer")] {
        let (_, invitation) = call(
            &router,
            "POST",
            "/api/workspaces/w/invitations",
            Some(&owner),
            Some(json!({"role":role,"expires_in_days":1})),
        )
        .await;
        let (status, accepted) = call(
            &router,
            "POST",
            "/api/workspace-invitations/accept",
            Some(token),
            Some(json!({"token":invitation["token"]})),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        base = format!(
            "/api/owners/{}/workspaces/w",
            accepted["owner_id"].as_str().unwrap()
        );
    }
    let endpoint = format!("{base}/sessions");
    let input = json!({"request_id":"r","expected_revision":saved["revision"]});
    assert_eq!(
        call(
            &router,
            "POST",
            &endpoint,
            Some(&viewer),
            Some(input.clone())
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (status, session) = call(
        &router,
        "POST",
        &endpoint,
        Some(&editor),
        Some(input.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{session}");
    assert_eq!(session["protocol"], "data");
    let id = session["id"].as_str().unwrap();
    let path = format!("{endpoint}/{id}");
    events_until(&router, &path, &editor, "data_ready").await;
    for actor in [&viewer, &other, &owner] {
        assert!(
            call(&router, "GET", &path, Some(actor), None)
                .await
                .0
                .is_client_error()
        );
    }
    assert_eq!(
        call(
            &router,
            "GET",
            &format!("/api/sessions/{id}"),
            Some(&editor),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &router,
            "POST",
            &format!("{path}/send"),
            Some(&editor),
            Some(json!({"kind":"text","text":"no"}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (status,result)=call(&router,"POST",&format!("{path}/send"),Some(&editor),Some(json!({"kind":"data_query","query_id":"member-query","sql":"SELECT name FROM data","read_only":true}))).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let batch = events_until(&router, &path, &editor, "data_processed").await;
    let serialized = batch.to_string();
    assert!(serialized.contains("owner-second"), "{batch}");
    assert!(!serialized.contains("owner-first"));
    assert!(serialized.contains("shared postprocessing"));
    assert!(batch["events"].as_array().unwrap().iter().any(|e|e["message"]["kind"]=="data_prepared"&&e["message"]["sql_changed"]==true));
    let (_, history) = call(
        &router,
        "GET",
        &format!("{base}/history"),
        Some(&editor),
        None,
    )
    .await;
    assert!(!history.as_array().unwrap().is_empty());
    assert!(!history.to_string().contains("owner-second"));
    assert_eq!(
        call(
            &router,
            "POST",
            &format!("{path}/send"),
            Some(&editor),
            Some(json!({"kind":"data_schema_refresh"}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, audit) = call(&router, "GET", &format!("{base}/audit"), Some(&owner), None).await;
    assert!(audit.to_string().contains("protocol.data.query"));
    assert!(!audit.to_string().contains("SELECT name"));
    assert!(!audit.to_string().contains("owner-second"));
    let (_, current) = call(&router, "GET", "/api/workspaces/w", Some(&owner), None).await;
    assert_eq!(current["data"], saved["data"]);
    assert_eq!(call(&router,"PUT","/api/workspaces/w",Some(&owner),Some(json!({"name":"Changed","expected_revision":current["revision"],"data":current["data"]}))).await.0,StatusCode::OK);
    assert!(
        call(&router, "GET", &path, Some(&editor), None)
            .await
            .0
            .is_client_error()
    );
    assert!(
        call(
            &router,
            "POST",
            &format!("{path}/send"),
            Some(&editor),
            Some(json!({"kind":"data_query","query_id":"stale","sql":"SELECT 1"}))
        )
        .await
        .0
        .is_client_error()
    );
    assert_eq!(
        call(&router, "POST", &endpoint, Some(&editor), Some(input))
            .await
            .0,
        StatusCode::CONFLICT
    );
    let (_, mut current) = call(&router, "GET", "/api/workspaces/w", Some(&owner), None).await;
    current["data"]["collections"][0]["requests"][0]["protocol"]["query_pre_script"] = "".into();
    let (status, updated) = call(&router, "PUT", "/api/workspaces/w", Some(&owner), Some(json!({"name":"Data cancellation","expected_revision":current["revision"],"data":current["data"]}))).await;
    assert_eq!(status, StatusCode::OK);
    let (status, session) = call(
        &router,
        "POST",
        &endpoint,
        Some(&editor),
        Some(json!({"request_id":"r","expected_revision":updated["revision"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{session}");
    let second_path = format!("{endpoint}/{}", session["id"].as_str().unwrap());
    events_until(&router, &second_path, &editor, "data_ready").await;
    assert_eq!(call(&router,"POST",&format!("{second_path}/send"),Some(&editor),Some(json!({"kind":"data_query","query_id":"cancel-query","sql":"SELECT sum(i) FROM range(10000000000) t(i)","read_only":true}))).await.0,StatusCode::OK);
    events_until(&router, &second_path, &editor, "data_started").await;
    assert_eq!(
        call(
            &router,
            "POST",
            &format!("{second_path}/send"),
            Some(&editor),
            Some(json!({"kind":"data_cancel","query_id":"cancel-query"}))
        )
        .await
        .0,
        StatusCode::OK
    );
    events_until(&router, &second_path, &editor, "data_cancelled").await;
    assert_eq!(
        call(&router, "POST", "/api/auth/logout", Some(&editor), None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&router, "GET", &second_path, Some(&editor), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn member_completion_waiting_for_database_writer_cannot_insert_after_demotion() {
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement, TransactionTrait};
    let dir = tempfile::tempdir().unwrap();
    let database_url =
        moleapi_server::sqlite_database_url(&dir.path().join("history-race.db")).unwrap();
    let mut settings = config(database_url.clone(), true);
    settings.allow_private_network = true;
    let router = moleapi_server::hosted_with_worker(
        settings,
        std::path::Path::new(env!("CARGO_BIN_EXE_moleapi-server")),
    )
    .await
    .unwrap();
    let owner = register(&router, "raceowner").await;
    let editor = register(&router, "raceeditor").await;
    let mut data = example_data();
    data["script_mode"] = "v2".into();
    let r = &mut data["collections"][0]["requests"][0];
    r["url"] = "".into();
    r["protocol"] = json!({"kind":"data","source":"local_file","file_name":"rows.csv","file_format":"csv","file_base64":STANDARD.encode(b"id\n1\n"),"sql":"SELECT id FROM data"});
    r["post_response_script"] = "const until=Date.now()+100;while(Date.now()<until){};".into();
    let (_, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Race","data":data})),
    )
    .await;
    let (_, invite) = call(
        &router,
        "POST",
        "/api/workspaces/w/invitations",
        Some(&owner),
        Some(json!({"role":"editor","expires_in_days":1})),
    )
    .await;
    let (_, accepted) = call(
        &router,
        "POST",
        "/api/workspace-invitations/accept",
        Some(&editor),
        Some(json!({"token":invite["token"]})),
    )
    .await;
    let base = format!(
        "/api/owners/{}/workspaces/w",
        accepted["owner_id"].as_str().unwrap()
    );
    let (status, session) = call(
        &router,
        "POST",
        &format!("{base}/sessions"),
        Some(&editor),
        Some(json!({"request_id":"r","expected_revision":saved["revision"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{session}");
    let path = format!("{base}/sessions/{}", session["id"].as_str().unwrap());
    events_until(&router, &path, &editor, "data_ready").await;
    assert_eq!(call(&router,"POST",&format!("{path}/send"),Some(&editor),Some(json!({"kind":"data_query","query_id":"race","sql":"SELECT id FROM data","read_only":true}))).await.0,StatusCode::OK);
    events_until(&router, &path, &editor, "data_finished").await;
    let db = Database::connect(database_url).await.unwrap();
    let tx = db.begin().await.unwrap();
    tx.execute(Statement::from_string(
        DbBackend::Sqlite,
        "UPDATE documents SET revision=revision WHERE kind='workspace'".to_owned(),
    ))
    .await
    .unwrap();
    tx.execute(Statement::from_string(
        DbBackend::Sqlite,
        "UPDATE workspace_memberships SET role='viewer',revision=revision+1".to_owned(),
    ))
    .await
    .unwrap();
    // Old code reads the previously committed membership and blocks only at INSERT.
    tokio::time::sleep(Duration::from_millis(300)).await;
    tx.commit().await.unwrap();
    tokio::time::sleep(Duration::from_millis(400)).await;
    let (status, history) = call(
        &router,
        "GET",
        "/api/workspaces/w/history",
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{history}");
    assert_eq!(
        history,
        json!([]),
        "completion must linearize with demotion before writing history"
    );
}
