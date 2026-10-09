mod common;
use common::*;
#[tokio::test]
async fn reports_preserve_real_outcomes_privacy_exports_and_survive_restart() {
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::get(|| async {
            axum::Json(json!({"token":"report-live-private-token","answer":1}))
        }),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("reports.db");
    let router = local(&path).await.unwrap();
    let mut data = example_data();
    data["environments"] = json!([{"id":"dev","name":"Dev","variables":[]},{"id":"prod","name":"Prod","variables":[{"id":"private","key":"token","value":"unused-environment-private","secret":true,"enabled":true}]}]);
    let first = &mut data["collections"][0]["requests"][0];
    first["url"] = format!("{url}/").into();
    first["name"] = "=SUM(1), 猫 <script>oops</script> canonical-auth-private".into();
    first["auth"] =
        json!({"kind":"bearer","token":"canonical-auth-private","username":"","password":""});
    first["post_response_script"]="pm.environment.set('token',pm.response.json().token);pm.test('Unicode 猫 <& check',()=>pm.expect(pm.response.json().answer).to.equal(2));".into();
    let mut skipped = first.clone();
    skipped["id"] = "skip".into();
    skipped["pre_request_script"] = "pm.execution.skipRequest();".into();
    data["collections"][0]["requests"]
        .as_array_mut()
        .unwrap()
        .push(skipped);
    let (status, w) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"unused-environment-private","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    let(status,live)=call(&router,"POST","/api/workspaces/w/run",None,Some(json!({"collection_id":"c","environment_id":"dev","dataset":{"format":"json","source":"[{\"label\":\"one\"},{\"label\":\"two\"}]"}}))).await;
    assert_eq!(status, StatusCode::OK, "{live}");
    assert_eq!(live["failed"], 2);
    assert_eq!(live["skipped"], 2);
    assert!(live.get("report_save_error").is_none(), "{live}");
    let id = live["report_id"].as_str().unwrap().to_string();
    let (status, report) = call(
        &router,
        "GET",
        &format!("/api/workspaces/w/reports/{id}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["summary"]["tests_failed"], 2);
    assert_eq!(report["results"][0]["outcome"], "failed");
    assert_eq!(report["results"][1]["outcome"], "skipped");
    for secret in [
        "report-live-private-token",
        "canonical-auth-private",
        "unused-environment-private",
    ] {
        assert!(!report.to_string().contains(secret), "stored {secret}");
    }
    assert!(!report.to_string().contains("variable_updates"));
    assert!(!report.to_string().contains("\"body\""));
    let (_, saved) = call(&router, "GET", "/api/workspaces/w", None, None).await;
    assert_eq!(saved["data"], w["data"]);
    assert_eq!(saved["revision"], w["revision"]);
    assert_eq!(saved["updated_at"], w["updated_at"]);
    for format in ["json", "csv", "junit", "html"] {
        let (status, file) = call(
            &router,
            "GET",
            &format!("/api/workspaces/w/reports/{id}/export?format={format}&language=zh-CN"),
            None,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{file}");
        let content = file["content"].as_str().unwrap();
        assert!(!content.contains("report-live-private-token"));
        match format {
            "json" => {
                let parsed: Value = serde_json::from_str(content).unwrap();
                assert_eq!(parsed["id"], id);
            }
            "junit" => {
                let doc = moleapi_core::parse_bounded_xml(content).unwrap();
                let root = doc.root_element();
                assert_eq!(root.attribute("tests"), Some("4"));
                assert_eq!(root.attribute("failures"), Some("2"));
                assert_eq!(root.attribute("skipped"), Some("2"));
            }
            "csv" => {
                assert!(content.contains("'=SUM(1)"));
            }
            "html" => {
                assert!(content.contains("API 测试报告"));
                assert!(!content.contains("<script>oops</script>"));
                assert!(content.contains("&lt;script&gt;oops&lt;/script&gt;"));
            }
            _ => unreachable!(),
        }
    }
    drop(router);
    let restarted = local(&path).await.unwrap();
    let (status, restored) = call(
        &restarted,
        "GET",
        &format!("/api/workspaces/w/reports/{id}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(restored, report);
    assert_eq!(
        call(
            &restarted,
            "DELETE",
            &format!("/api/workspaces/w/reports/{id}"),
            None,
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &restarted,
            "GET",
            &format!("/api/workspaces/w/reports/{id}"),
            None,
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (_, history) = call(&restarted, "GET", "/api/workspaces/w/history", None, None).await;
    assert_eq!(history.as_array().unwrap().len(), 2);
    fixture.abort();
}
#[tokio::test]
async fn reports_are_scoped_to_owner_workspace_and_cascade_on_workspace_deletion() {
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
    let owner = register(&router, "report-owner").await;
    let other = register(&router, "report-other").await;
    let mut data = example_data();
    data["collections"][0]["requests"][0]["pre_request_script"] =
        "pm.execution.skipRequest();".into();
    for id in ["w", "other"] {
        assert_eq!(
            call(
                &router,
                "POST",
                "/api/workspaces",
                Some(&owner),
                Some(json!({"id":id,"name":"Owned","data":data}))
            )
            .await
            .0,
            StatusCode::OK
        );
    }
    let (_, run) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        Some(&owner),
        Some(json!({"collection_id":"c"})),
    )
    .await;
    let id = run["report_id"].as_str().unwrap();
    for (method, path) in [
        ("GET", format!("/api/workspaces/w/reports/{id}")),
        (
            "GET",
            format!("/api/workspaces/w/reports/{id}/export?format=json"),
        ),
        ("DELETE", format!("/api/workspaces/w/reports/{id}")),
    ] {
        assert_eq!(
            call(&router, method, &path, Some(&other), None).await.0,
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        call(
            &router,
            "GET",
            &format!("/api/workspaces/other/reports/{id}"),
            Some(&owner),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &router,
            "GET",
            "/api/workspaces/w/reports?cursor=not-a-cursor",
            Some(&owner),
            None
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (_, mut workspace) = call(&router, "GET", "/api/workspaces/w", Some(&owner), None).await;
    workspace["data"]["collections"][0]["requests"] = json!([]);
    assert_eq!(
        call(
            &router,
            "PUT",
            "/api/workspaces/w",
            Some(&owner),
            Some(json!({"name":"Edited","data":workspace["data"],"expected_revision":1}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, report) = call(
        &router,
        "GET",
        &format!("/api/workspaces/w/reports/{id}"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(report["workspace_revision"], 1);
    assert_eq!(report["results"][0]["request_id"], "r");
    assert_eq!(
        call(
            &router,
            "DELETE",
            "/api/workspaces/w",
            Some(&owner),
            Some(json!({"expected_revision":2}))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &router,
            "GET",
            "/api/workspaces/w/reports",
            Some(&owner),
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
            "/api/workspaces",
            Some(&owner),
            Some(json!({"id":"w","name":"Recreated","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, reports) = call(
        &router,
        "GET",
        "/api/workspaces/w/reports",
        Some(&owner),
        None,
    )
    .await;
    assert!(reports["items"].as_array().unwrap().is_empty());
}
#[tokio::test]
async fn cancelled_runs_are_saved_and_junit_marks_the_incomplete_run_as_error() {
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::get(|| async { std::future::pending::<String>().await }),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("cancel.db")).await.unwrap();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = format!("{url}/").into();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"Canceled","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces/w/run/cancel",
            None,
            Some(json!({"job_id":"canceled"}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, run) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        None,
        Some(json!({"collection_id":"c","job_id":"canceled"})),
    )
    .await;
    assert_eq!(run["cancelled"], true);
    let id = run["report_id"].as_str().unwrap();
    let (_, file) = call(
        &router,
        "GET",
        &format!("/api/workspaces/w/reports/{id}/export?format=junit"),
        None,
        None,
    )
    .await;
    let doc = moleapi_core::parse_bounded_xml(file["content"].as_str().unwrap()).unwrap();
    assert_eq!(doc.root_element().attribute("errors"), Some("1"));
    fixture.abort();
}
#[tokio::test]
async fn summary_seek_pagination_and_retention_keep_only_latest_hundred_reports() {
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement, TransactionTrait};
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("retention.db");
    let router = local(&path).await.unwrap();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["pre_request_script"] =
        "pm.execution.skipRequest();".into();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"Retention","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, run) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        None,
        Some(json!({"collection_id":"c"})),
    )
    .await;
    let id = run["report_id"].as_str().unwrap();
    let (_, template) = call(
        &router,
        "GET",
        &format!("/api/workspaces/w/reports/{id}"),
        None,
        None,
    )
    .await;
    let db = Database::connect(format!("sqlite://{}?mode=rwc", path.display()))
        .await
        .unwrap();
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT owner, payload FROM documents WHERE id = ?",
            [id.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    let owner: String = row.try_get("", "owner").unwrap();
    let tx = db.begin().await.unwrap();
    for index in 0..100u128 {
        let report_id = uuid::Uuid::from_u128(index + 1).to_string();
        let mut report = template.clone();
        report["id"] = report_id.clone().into();
        let summary = json!({"id":report_id,"workspace_revision":report["workspace_revision"],"collection_name":report["collection_name"],"scenario_name":report["scenario_name"],"environment_name":report["environment_name"],"started_at":report["started_at"],"finished_at":report["finished_at"],"summary":report["summary"]});
        for (key, kind, payload) in [
            (report_id.clone(), "run-report", report),
            (format!("run-brief-{report_id}"), "run-brief", summary),
        ] {
            tx.execute(Statement::from_sql_and_values(DbBackend::Sqlite,"INSERT INTO documents (id, owner, kind, ref_id, revision, payload, updated_at) VALUES (?, ?, ?, ?, 0, ?, ?)",[key.into(),owner.clone().into(),kind.into(),"w".into(),payload.to_string().into(),"2020-01-01T00:00:00+00:00".into()])).await.unwrap();
        }
    }
    tx.commit().await.unwrap();
    let (_, latest) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        None,
        Some(json!({"collection_id":"c"})),
    )
    .await;
    assert!(latest.get("report_save_error").is_none(), "{latest}");
    let mut cursor = String::new();
    let mut ids = std::collections::BTreeSet::new();
    loop {
        let (status, page) = call(
            &router,
            "GET",
            &format!(
                "/api/workspaces/w/reports?limit=17{}",
                if cursor.is_empty() {
                    String::new()
                } else {
                    format!("&cursor={cursor}")
                }
            ),
            None,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{page}");
        for item in page["items"].as_array().unwrap() {
            assert!(item.get("results").is_none());
            assert!(
                ids.insert(item["id"].as_str().unwrap().to_string()),
                "duplicate page item"
            );
        }
        match page["next_cursor"].as_str() {
            Some(value) => cursor = value.into(),
            None => break,
        }
    }
    assert_eq!(ids.len(), 100);
    assert!(ids.contains(latest["report_id"].as_str().unwrap()));
    let row=db.query_one(Statement::from_sql_and_values(DbBackend::Sqlite,"SELECT count(*) AS count FROM documents WHERE owner = ? AND ref_id = ? AND kind IN ('run-report', 'run-brief')",[owner.into(),"w".into()])).await.unwrap().unwrap();
    assert_eq!(row.try_get::<i64>("", "count").unwrap(), 200);
    assert_eq!(
        call(&router, "DELETE", "/api/workspaces/w/reports", None, None)
            .await
            .0,
        StatusCode::OK
    );
    let (_, page) = call(&router, "GET", "/api/workspaces/w/reports", None, None).await;
    assert!(page["items"].as_array().unwrap().is_empty());
}
#[tokio::test]
async fn omitted_live_responses_keep_failed_outcomes_and_assertion_counts_in_saved_reports() {
    let text = "x".repeat(1024 * 1024);
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::get(move || {
            let text = text.clone();
            async move { text }
        }),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("omitted.db")).await.unwrap();
    let mut data = example_data();
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = format!("{url}/").into();
    request["assertions"] = json!([{"id":"missing","name":"Missing marker","kind":"contains","target":"","expected":"never-in-synthetic-body"}]);
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"Omitted","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, live) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        None,
        Some(json!({"collection_id":"c","iterations":10})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{live}");
    assert!(live["omitted_responses"].as_u64().unwrap() > 0);
    let id = live["report_id"].as_str().unwrap();
    let (_, report) = call(
        &router,
        "GET",
        &format!("/api/workspaces/w/reports/{id}"),
        None,
        None,
    )
    .await;
    assert_eq!(report["summary"]["failed"], 10);
    assert_eq!(report["summary"]["tests_failed"], 10);
    assert!(report["summary"]["diagnostics_omitted"].as_u64().unwrap() > 0);
    assert!(
        report["results"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item["outcome"] == "failed")
    );
    for item in report["results"].as_array().unwrap() {
        assert!(item["history_id"].is_string());
    }
    let position = live["results"]
        .as_array()
        .unwrap()
        .iter()
        .position(|item| item["response_omitted"] == true)
        .unwrap();
    assert_eq!(
        call(
            &router,
            "GET",
            &format!("/api/workspaces/w/reports/{id}/steps/{position}/response"),
            None,
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    fixture.abort();
}
#[tokio::test]
async fn persistence_failures_keep_live_results_and_report_the_failure_explicitly() {
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("failed-store.db");
    let router = local(&path).await.unwrap();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["pre_request_script"] =
        "pm.execution.skipRequest();".into();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"Failure","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let db = Database::connect(format!("sqlite://{}?mode=rwc", path.display()))
        .await
        .unwrap();
    db.execute(Statement::from_string(DbBackend::Sqlite,"CREATE TRIGGER report_write_fails BEFORE INSERT ON documents WHEN NEW.kind='run-report' BEGIN SELECT RAISE(ABORT,'synthetic report failure'); END;")).await.unwrap();
    let (status, run) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        None,
        Some(json!({"collection_id":"c"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{run}");
    assert_eq!(run["skipped"], 1);
    assert!(run.get("report_id").is_none());
    assert_eq!(
        run["report_save_error"],
        "Run report could not be persisted"
    );
    let (_, page) = call(&router, "GET", "/api/workspaces/w/reports", None, None).await;
    assert!(page["items"].as_array().unwrap().is_empty());
}
#[tokio::test]
async fn report_method_reflects_ephemeral_execution_mutations_without_changing_the_interface() {
    let (url, fixture) =
        serve(Router::new().route("/", axum::routing::post(|| async { "ok" }))).await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("method.db")).await.unwrap();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = format!("{url}/").into();
    data["collections"][0]["requests"][0]["pre_request_script"] =
        "pm.request.method='POST';".into();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"Method","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, run) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        None,
        Some(json!({"collection_id":"c"})),
    )
    .await;
    assert_eq!(run["passed"], 1, "{run}");
    let id = run["report_id"].as_str().unwrap();
    let (_, report) = call(
        &router,
        "GET",
        &format!("/api/workspaces/w/reports/{id}"),
        None,
        None,
    )
    .await;
    assert_eq!(report["results"][0]["method"], "POST");
    let (_, workspace) = call(&router, "GET", "/api/workspaces/w", None, None).await;
    assert_eq!(
        workspace["data"]["collections"][0]["requests"][0]["method"],
        "GET"
    );
    fixture.abort();
}
