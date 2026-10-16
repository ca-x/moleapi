mod common;
use base64::{Engine, engine::general_purpose::STANDARD};
use common::*;
use std::time::Duration;
static DATA_CLIENT_TESTS: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
fn worker() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_moleapi-server"))
}
fn data() -> Value {
    let mut data = example_data();
    let r = &mut data["collections"][0]["requests"][0];
    r["url"] = json!("");
    r["method"] = json!("GET");
    r["body_kind"] = json!("none");
    r["body"] = json!("");
    r["assertions"] = json!([]);
    r["protocol"] = json!({"kind":"data","source":"local_file","file_name":"fixture.csv","file_format":"csv","file_base64":STANDARD.encode(b"id,name\n1,first\n2,second\n"),"table_name":"data","sql":"SELECT name FROM data WHERE id=2"});
    data
}
async fn create(router: &Router, token: Option<&str>) -> Value {
    let (status, w) = call(
        router,
        "POST",
        "/api/workspaces",
        token,
        Some(json!({"id":"data","name":"Data","data":data()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    let (status, s) = call(
        router,
        "POST",
        "/api/sessions",
        token,
        Some(json!({"workspace_id":"data","request":w["data"]["collections"][0]["requests"][0]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{s}");
    s
}
async fn ready(router: &Router, token: Option<&str>, id: &str) {
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let (_, s) = call(router, "GET", &format!("/api/sessions/{id}"), token, None).await;
            if s["state"] == "open" {
                break;
            }
            assert_ne!(s["state"], "error", "{s}");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
}
async fn query(router: &Router, token: Option<&str>, id: &str, query_id: &str, sql: &str) -> Value {
    let (status, r) = call(
        router,
        "POST",
        &format!("/api/sessions/{id}/send"),
        token,
        Some(json!({"kind":"data_query","query_id":query_id,"sql":sql,"read_only":true})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{r}");
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let (_, batch) = call(
                router,
                "GET",
                &format!("/api/sessions/{id}/events?after=0"),
                token,
                None,
            )
            .await;
            if batch["events"].as_array().unwrap().iter().any(|event| {
                event["message"]["query_id"] == query_id
                    && matches!(
                        event["message"]["kind"].as_str(),
                        Some(
                            "data_finished"
                                | "data_error"
                                | "data_preprocess_error"
                                | "data_skipped"
                        )
                    )
            }) {
                return batch;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap()
}
#[tokio::test]
async fn actual_native_router_file_worker_schema_query_readonly_and_workspace_lifecycle() {
    let _serial = DATA_CLIENT_TESTS.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let router = moleapi_server::local_with_worker(&temp.path().join("data.db"), &worker())
        .await
        .unwrap();
    let session = create(&router, None).await;
    let id = session["id"].as_str().unwrap();
    ready(&router, None, id).await;
    let batch = query(
        &router,
        None,
        id,
        "query",
        "SELECT name FROM data WHERE id=2",
    )
    .await;
    let events = batch["events"].as_array().unwrap();
    assert!(events.iter().any(|e| e["message"]["kind"] == "data_schema"));
    let rows = events
        .iter()
        .find(|e| e["message"]["kind"] == "data_rows")
        .unwrap();
    assert_eq!(rows["message"]["rows"][0][0]["value"], "second");
    let failed = query(&router, None, id, "readonly", "DELETE FROM data").await;
    assert!(failed["events"].as_array().unwrap().iter().any(|e|e["message"]["query_id"]=="readonly"&&e["message"]["kind"]=="data_error"));
    let (status, _) = call(
        &router,
        "DELETE",
        "/api/workspaces/data",
        None,
        Some(json!({"expected_revision":1})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        call(&router, "GET", &format!("/api/sessions/{id}"), None, None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}
#[tokio::test]
async fn hosted_data_sessions_are_owner_scoped_and_logout_closes_work() {
    let _serial = DATA_CLIENT_TESTS.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let router = moleapi_server::hosted_with_worker(
        config(
            format!(
                "sqlite://{}?mode=rwc",
                temp.path().join("owner.db").display()
            ),
            true,
        ),
        &worker(),
    )
    .await
    .unwrap();
    let owner = register(&router, "owner").await;
    let other = register(&router, "other").await;
    let session = create(&router, Some(&owner)).await;
    let id = session["id"].as_str().unwrap();
    ready(&router, Some(&owner), id).await;
    assert_eq!(
        call(
            &router,
            "GET",
            &format!("/api/sessions/{id}/events"),
            Some(&other),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    query(
        &router,
        Some(&owner),
        id,
        "owner-only",
        "SELECT name FROM data WHERE id=2",
    )
    .await;
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let (_, entries) = call(
                &router,
                "GET",
                "/api/workspaces/data/history",
                Some(&owner),
                None,
            )
            .await;
            if entries.as_array().is_some_and(|rows| rows.len() == 1) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        call(
            &router,
            "GET",
            "/api/workspaces/data/history",
            Some(&other),
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
            "/api/auth/logout",
            Some(&owner),
            Some(json!({}))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &router,
            "GET",
            &format!("/api/sessions/{id}"),
            Some(&owner),
            None
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn saved_data_ca_accepts_certificates_and_rejects_private_keys_before_persistence() {
    let _serial = DATA_CLIENT_TESTS.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("data-ca.db")).await.unwrap();
    let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let pem = certificate.cert.pem();
    let key = certificate.signing_key.serialize_pem();
    for (index, bad) in [
        key.clone(),
        format!("{pem}\n{key}"),
        "not a certificate".into(),
        format!("{key}\n{{{{custom_ca}}}}"),
        format!("{{{{{key}}}}}"),
        format!(
            "{pem}\n{}",
            key.replace("PRIVATE KEY", "ENCRYPTED PRIVATE KEY")
        ),
        format!(
            "{pem}\n{}",
            key.replace("PRIVATE KEY", "OPENSSH PRIVATE KEY")
        ),
        format!("{pem}\n-----BEGIN PRIVATE KEY-----\nunfinished"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut input = data();
        input["collections"][0]["requests"][0]["protocol"]["ca_pem"] = json!(bad);
        let (status, result) = call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":format!("bad-ca-{index}"),"name":"CA input","data":input})),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{result}");
        assert!(!result.to_string().contains(&key));
    }
    let mut input = data();
    input["collections"][0]["requests"][0]["protocol"]["ca_pem"] = json!(pem);
    let (status, result) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"certificate","name":"CA input","data":input})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(
        result["data"]["collections"][0]["requests"][0]["protocol"]["ca_pem"],
        pem
    );
    let mut input = data();
    input["collections"][0]["requests"][0]["protocol"]["ca_pem"] = json!("{{custom_ca}}");
    let (status, result) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"ca-template","name":"CA input","data":input})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let request: moleapi_core::RequestSpec =
        serde_json::from_value(input["collections"][0]["requests"][0].clone()).unwrap();
    let mut environment: moleapi_core::Environment = serde_json::from_value(json!({"id":"ca","name":"CA","variables":[{"id":"ca","key":"custom_ca","value":pem,"enabled":true}]})).unwrap();
    let resolved = moleapi_core::resolve_request(&request, Some(&environment)).unwrap();
    moleapi_core::validate_request(&resolved, false).unwrap();
    environment.variables[0].value = key;
    let result = moleapi_core::resolve_request(&request, Some(&environment));
    assert!(result.is_err() || moleapi_core::validate_request(&result.unwrap(), false).is_err());
}
#[tokio::test]
async fn remote_file_network_dns_settings_download_real_csv_and_run_bounded_sql_worker() {
    let _serial = DATA_CLIENT_TESTS.lock().await;
    let (base, server) = serve(Router::new().route(
        "/data.csv",
        axum::routing::get(|headers: axum::http::HeaderMap| async move {
            assert!(
                headers["host"]
                    .to_str()
                    .unwrap()
                    .starts_with("data-network.test:")
            );
            assert_eq!(headers["accept-encoding"], "identity");
            (
                [("content-type", "text/csv")],
                "id,name\n1,first\n2,network-second\n",
            )
        }),
    ))
    .await;
    let port = url::Url::parse(&base).unwrap().port().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let router =
        moleapi_server::local_with_worker(&temp.path().join("remote-network.db"), &worker())
            .await
            .unwrap();
    let mut source = data();
    let r = &mut source["collections"][0]["requests"][0];
    r["url"] = json!(format!("http://data-network.test:{port}/data.csv"));
    r["protocol"]["source"] = json!("remote_file");
    r["protocol"]["file_base64"] = json!("");
    r["network"] = json!({"dns":[{"hostname":"data-network.test","addresses":["127.0.0.1"]}]});
    let (status, w) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"data","name":"Remote network","data":source})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    let (status, s) = call(
        &router,
        "POST",
        "/api/sessions",
        None,
        Some(json!({"workspace_id":"data","request":w["data"]["collections"][0]["requests"][0]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{s}");
    let id = s["id"].as_str().unwrap();
    ready(&router, None, id).await;
    let result = query(
        &router,
        None,
        id,
        "network-query",
        "SELECT name FROM data WHERE id=2",
    )
    .await;
    let row = result["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| {
            e["message"]["kind"] == "data_rows" && e["message"]["query_id"] == "network-query"
        })
        .expect("SQL worker result missing");
    assert_eq!(row["message"]["rows"][0][0]["value"], "network-second");
    call(
        &router,
        "DELETE",
        &format!("/api/sessions/{id}"),
        None,
        None,
    )
    .await;
    server.abort();
}

#[tokio::test]
async fn hosted_checked_remote_xlsx_selects_named_sheet_and_runs_sql() {
    let _serial = DATA_CLIENT_TESTS.lock().await;
    let (base, server) = serve(Router::new().route(
        "/workbook.xlsx",
        axum::routing::get(|headers: axum::http::HeaderMap| async move {
            assert!(
                headers["host"]
                    .to_str()
                    .unwrap()
                    .starts_with("xlsx-network.test:")
            );
            assert_eq!(headers["accept-encoding"], "identity");
            (
                [(
                    "content-type",
                    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                )],
                include_bytes!("../../data/tests/fixtures/workbook.xlsx").to_vec(),
            )
        }),
    ))
    .await;
    let port = url::Url::parse(&base).unwrap().port().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let mut settings = config(
        moleapi_server::sqlite_database_url(&temp.path().join("xlsx-hosted.db")).unwrap(),
        true,
    );
    settings.allow_private_network = true;
    let router = hosted(settings).await.unwrap();
    let token = register(&router, "xlsx-owner").await;
    let mut source = data();
    let request = &mut source["collections"][0]["requests"][0];
    request["url"] = json!(format!("http://xlsx-network.test:{port}/workbook.xlsx"));
    request["protocol"]["source"] = json!("remote_file");
    request["protocol"]["file_format"] = json!("xlsx");
    request["protocol"]["file_name"] = json!("workbook.xlsx");
    request["protocol"]["sheet_name"] = json!("Transactions");
    request["protocol"]["file_base64"] = json!("");
    request["network"] =
        json!({"dns":[{"hostname":"xlsx-network.test","addresses":["127.0.0.1"]}]});
    let (status, workspace) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&token),
        Some(json!({"id":"xlsx","name":"Remote workbook","data":source})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{workspace}");
    let (status, session) = call(&router,"POST","/api/sessions",Some(&token),
        Some(json!({"workspace_id":"xlsx","request":workspace["data"]["collections"][0]["requests"][0]}))).await;
    assert_eq!(status, StatusCode::OK, "{session}");
    let id = session["id"].as_str().unwrap();
    ready(&router, Some(&token), id).await;
    let events = query(
        &router,
        Some(&token),
        id,
        "xlsx-query",
        "SELECT id, CAST(amount AS DECIMAL(10,2)) AS amount, exact_text FROM data WHERE id=1",
    )
    .await;
    let row = events["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|event| {
            event["message"]["kind"] == "data_rows" && event["message"]["query_id"] == "xlsx-query"
        })
        .expect("Hosted XLSX query result missing");
    assert_eq!(row["message"]["rows"][0][0]["value"], "1");
    assert_eq!(row["message"]["rows"][0][1]["value"], "1.23");
    assert_eq!(row["message"]["rows"][0][2]["value"], "9007199254740993");
    call(
        &router,
        "DELETE",
        &format!("/api/sessions/{id}"),
        Some(&token),
        None,
    )
    .await;
    server.abort();
}

#[tokio::test]
async fn hosted_checked_remote_ods_selects_named_sheet_and_runs_sql() {
    let _serial = DATA_CLIENT_TESTS.lock().await;
    let (base, server) = serve(Router::new().route(
        "/workbook.ods",
        axum::routing::get(|headers: axum::http::HeaderMap| async move {
            assert!(
                headers["host"]
                    .to_str()
                    .unwrap()
                    .starts_with("ods-network.test:")
            );
            assert_eq!(headers["accept-encoding"], "identity");
            (
                [(
                    "content-type",
                    "application/vnd.oasis.opendocument.spreadsheet",
                )],
                include_bytes!("../../data/tests/fixtures/workbook.ods").to_vec(),
            )
        }),
    ))
    .await;
    let port = url::Url::parse(&base).unwrap().port().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let mut settings = config(
        moleapi_server::sqlite_database_url(&temp.path().join("ods-hosted.db")).unwrap(),
        true,
    );
    settings.allow_private_network = true;
    let router = hosted(settings).await.unwrap();
    let token = register(&router, "ods-owner").await;
    let mut source = data();
    let request = &mut source["collections"][0]["requests"][0];
    request["url"] = json!(format!("http://ods-network.test:{port}/workbook.ods"));
    request["protocol"]["source"] = json!("remote_file");
    request["protocol"]["file_format"] = json!("ods");
    request["protocol"]["file_name"] = json!("workbook.ods");
    request["protocol"]["sheet_name"] = json!("Transactions");
    request["protocol"]["file_base64"] = json!("");
    request["network"] = json!({"dns":[{"hostname":"ods-network.test","addresses":["127.0.0.1"]}]});
    let (status, workspace) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&token),
        Some(json!({"id":"ods","name":"Remote workbook","data":source})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{workspace}");
    let (status, session) = call(&router,"POST","/api/sessions",Some(&token),
        Some(json!({"workspace_id":"ods","request":workspace["data"]["collections"][0]["requests"][0]}))).await;
    assert_eq!(status, StatusCode::OK, "{session}");
    let id = session["id"].as_str().unwrap();
    ready(&router, Some(&token), id).await;
    let events = query(
        &router,
        Some(&token),
        id,
        "ods-query",
        "SELECT id, CAST(amount AS DECIMAL(10,2)) AS amount, note FROM data WHERE id=2",
    )
    .await;
    let row = events["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|event| {
            event["message"]["kind"] == "data_rows" && event["message"]["query_id"] == "ods-query"
        })
        .expect("Hosted ODS query result missing");
    assert_eq!(row["message"]["rows"][0][0]["value"], "2");
    assert_eq!(row["message"]["rows"][0][1]["value"], "2.30");
    assert_eq!(row["message"]["rows"][0][2]["value"], "café");
    call(
        &router,
        "DELETE",
        &format!("/api/sessions/{id}"),
        Some(&token),
        None,
    )
    .await;
    server.abort();
}

#[tokio::test]
async fn hosted_checked_remote_xls_selects_named_sheet_and_runs_sql() {
    let _serial = DATA_CLIENT_TESTS.lock().await;
    let (base, server) = serve(Router::new().route(
        "/workbook.xls",
        axum::routing::get(|headers: axum::http::HeaderMap| async move {
            assert!(
                headers["host"]
                    .to_str()
                    .unwrap()
                    .starts_with("xls-network.test:")
            );
            assert_eq!(headers["accept-encoding"], "identity");
            (
                [("content-type", "application/vnd.ms-excel")],
                include_bytes!("../../data/tests/fixtures/workbook.xls").to_vec(),
            )
        }),
    ))
    .await;
    let port = url::Url::parse(&base).unwrap().port().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let mut settings = config(
        moleapi_server::sqlite_database_url(&temp.path().join("xls-hosted.db")).unwrap(),
        true,
    );
    settings.allow_private_network = true;
    let router = hosted(settings).await.unwrap();
    let token = register(&router, "xls-owner").await;
    let mut source = data();
    let request = &mut source["collections"][0]["requests"][0];
    request["url"] = json!(format!("http://xls-network.test:{port}/workbook.xls"));
    request["protocol"]["source"] = json!("remote_file");
    request["protocol"]["file_format"] = json!("xls");
    request["protocol"]["file_name"] = json!("workbook.xls");
    request["protocol"]["sheet_name"] = json!("Transactions");
    request["protocol"]["file_base64"] = json!("");
    request["network"] = json!({"dns":[{"hostname":"xls-network.test","addresses":["127.0.0.1"]}]});
    let (status, workspace) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&token),
        Some(json!({"id":"xls","name":"Remote workbook","data":source})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{workspace}");
    let (status, session) = call(&router,"POST","/api/sessions",Some(&token),
        Some(json!({"workspace_id":"xls","request":workspace["data"]["collections"][0]["requests"][0]}))).await;
    assert_eq!(status, StatusCode::OK, "{session}");
    let id = session["id"].as_str().unwrap();
    ready(&router, Some(&token), id).await;
    let events = query(
        &router,
        Some(&token),
        id,
        "xls-query",
        "SELECT id, CAST(amount AS DECIMAL(10,2)) AS amount, note FROM data WHERE id=2",
    )
    .await;
    let row = events["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|event| {
            event["message"]["kind"] == "data_rows" && event["message"]["query_id"] == "xls-query"
        })
        .expect("Hosted XLS query result missing");
    assert_eq!(row["message"]["rows"][0][0]["value"], "2");
    assert_eq!(row["message"]["rows"][0][1]["value"], "2.30");
    assert_eq!(row["message"]["rows"][0][2]["value"], "café");
    call(
        &router,
        "DELETE",
        &format!("/api/sessions/{id}"),
        Some(&token),
        None,
    )
    .await;
    server.abort();
}

#[tokio::test]
async fn completed_file_data_query_enters_owner_history_without_rows() {
    let _serial = DATA_CLIENT_TESTS.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("data-history.db");
    let router = moleapi_server::local_with_worker(&database, &worker())
        .await
        .unwrap();
    let session = create(&router, None).await;
    let id = session["id"].as_str().unwrap();
    ready(&router, None, id).await;
    let events = query(
        &router,
        None,
        id,
        "history-query",
        "SELECT name FROM data WHERE id=2",
    )
    .await;
    assert!(events["events"].as_array().unwrap().iter().any(|event| {
        event["message"]["kind"] == "data_finished"
            && event["message"]["query_id"] == "history-query"
    }));
    let history = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let (_, history) =
                call(&router, "GET", "/api/workspaces/data/history", None, None).await;
            if let Some(entry) = history.as_array().and_then(|rows| rows.first()) {
                return entry.clone();
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("Data query was not saved to history");
    assert_eq!(history["request_id"], "r");
    assert_eq!(history["method"], "SQL");
    let body: serde_json::Value =
        serde_json::from_str(history["response"]["body"].as_str().unwrap()).unwrap();
    assert_eq!(body["format"], "moleapi-data-result-v1");
    assert_eq!(body["query_id"], "history-query");
    assert!(body.get("rows").is_none());
    assert_eq!(body["rows_withheld"], "file-backed rows are live-only");
    call(
        &router,
        "DELETE",
        &format!("/api/sessions/{id}"),
        None,
        None,
    )
    .await;
    drop(router);
    let restored = moleapi_server::local_with_worker(&database, &worker())
        .await
        .unwrap();
    let (_, entries) = call(&restored, "GET", "/api/workspaces/data/history", None, None).await;
    assert_eq!(entries[0]["id"], history["id"]);
}

#[tokio::test]
async fn data_query_runs_post_script_once_and_saves_error_outcome() {
    let _serial = DATA_CLIENT_TESTS.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let router = moleapi_server::local_with_worker(&temp.path().join("data-post.db"), &worker())
        .await
        .unwrap();
    let mut document = data();
    let request = &mut document["collections"][0]["requests"][0];
    request["post_response_script"] = json!(
        "pm.test('typed row',()=>pm.expect(pm.response.json().rows[0][0].value).to.equal('second'));"
    );
    request["extractions"] = json!([{"id":"missing","name":"Required marker","kind":"json","target":"/missing","scope":"temporary","key":"marker","required":true}]);
    let (status, workspace) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"data","name":"Data","data":document})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{workspace}");
    let (status, session) = call(&router, "POST", "/api/sessions", None,
        Some(json!({"workspace_id":"data","request":workspace["data"]["collections"][0]["requests"][0]}))).await;
    assert_eq!(status, StatusCode::OK, "{session}");
    let id = session["id"].as_str().unwrap();
    ready(&router, None, id).await;
    query(
        &router,
        None,
        id,
        "post",
        "SELECT name FROM data WHERE id=2",
    )
    .await;
    let processed = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let (_, events) = call(
                &router,
                "GET",
                &format!("/api/sessions/{id}/events"),
                None,
                None,
            )
            .await;
            if let Some(event) = events["events"]
                .as_array()
                .unwrap()
                .iter()
                .find(|event| event["message"]["kind"] == "data_processed")
            {
                return event["message"].clone();
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(processed["query_id"], "post");
    assert!(processed["history_id"].is_string());
    assert_eq!(processed["tests"].as_array().unwrap().len(), 2);
    assert!(
        processed["tests"]
            .as_array()
            .unwrap()
            .iter()
            .any(|test| test["name"] == "typed row" && test["passed"] == true)
    );
    assert!(
        processed["tests"]
            .as_array()
            .unwrap()
            .iter()
            .any(|test| test["name"] == "Required marker" && test["passed"] == false)
    );
    let (_, saved) = call(&router, "GET", "/api/workspaces/data/history", None, None).await;
    assert!(saved[0]["response"]["tests"].as_array().unwrap().is_empty());

    query(&router, None, id, "bad-sql", "SELECT missing FROM data").await;
    let history = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let (_, history) =
                call(&router, "GET", "/api/workspaces/data/history", None, None).await;
            if history.as_array().is_some_and(|entries| entries.len() == 2) {
                return history;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let entries = history.as_array().unwrap();
    assert_eq!(
        entries
            .iter()
            .filter(|entry| entry["response"]["status"] == 200)
            .count(),
        1
    );
    let failed = entries
        .iter()
        .find(|entry| entry["response"]["status"] == 500)
        .unwrap();
    let body: serde_json::Value =
        serde_json::from_str(failed["response"]["body"].as_str().unwrap()).unwrap();
    assert_eq!(body["query_id"], "bad-sql");
    assert_eq!(
        body["error"],
        "[WITHHELD: file query diagnostics are live-only]"
    );
    let failed_processing = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let (_, events) = call(
                &router,
                "GET",
                &format!("/api/sessions/{id}/events"),
                None,
                None,
            )
            .await;
            if let Some(event) = events["events"].as_array().unwrap().iter().find(|event| {
                event["message"]["kind"] == "data_processed"
                    && event["message"]["query_id"] == "bad-sql"
            }) {
                return event["message"].clone();
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert!(failed_processing["tests"].as_array().unwrap().is_empty());

    query(
        &router,
        None,
        id,
        "literal",
        "SELECT 'result exceeds the 5 MiB script/history body limit' AS name",
    )
    .await;
    let processed = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let (_, events) = call(
                &router,
                "GET",
                &format!("/api/sessions/{id}/events"),
                None,
                None,
            )
            .await;
            if let Some(event) = events["events"].as_array().unwrap().iter().find(|event| {
                event["message"]["kind"] == "data_processed"
                    && event["message"]["query_id"] == "literal"
            }) {
                return event["message"].clone();
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert!(processed["notice"].is_null());
    assert!(
        processed["tests"]
            .as_array()
            .unwrap()
            .iter()
            .any(|test| test["name"] == "typed row")
    );
}

#[tokio::test]
async fn oversized_data_result_saves_metadata_and_skips_post_script() {
    let _serial = DATA_CLIENT_TESTS.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let router = moleapi_server::local_with_worker(&temp.path().join("data-large.db"), &worker())
        .await
        .unwrap();
    let mut document = data();
    document["collections"][0]["requests"][0]["post_response_script"] =
        json!("pm.test('must not run',()=>pm.expect(true).to.equal(true));");
    let (status, workspace) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"data","name":"Data","data":document})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{workspace}");
    let (status, session) = call(&router, "POST", "/api/sessions", None,
        Some(json!({"workspace_id":"data","request":workspace["data"]["collections"][0]["requests"][0]}))).await;
    assert_eq!(status, StatusCode::OK, "{session}");
    let id = session["id"].as_str().unwrap();
    ready(&router, None, id).await;
    query(
        &router,
        None,
        id,
        "large",
        "SELECT repeat('x', 60000) AS value FROM range(100)",
    )
    .await;
    let processed = tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let (_, events) = call(
                &router,
                "GET",
                &format!("/api/sessions/{id}/events"),
                None,
                None,
            )
            .await;
            if let Some(event) = events["events"]
                .as_array()
                .unwrap()
                .iter()
                .find(|event| event["message"]["kind"] == "data_processed")
            {
                return event["message"].clone();
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(processed["notice"], "body_limit");
    assert!(processed["tests"].as_array().unwrap().is_empty());
    let (_, history) = call(&router, "GET", "/api/workspaces/data/history", None, None).await;
    let body: serde_json::Value =
        serde_json::from_str(history[0]["response"]["body"].as_str().unwrap()).unwrap();
    assert!(body.get("rows").is_none());
    assert!(body["rows_withheld"].as_str().unwrap().contains("5 MiB"));
}

#[tokio::test]
async fn cancelled_data_query_records_terminal_outcome_without_replay() {
    let _serial = DATA_CLIENT_TESTS.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let router = moleapi_server::local_with_worker(&temp.path().join("data-cancel.db"), &worker())
        .await
        .unwrap();
    let session = create(&router, None).await;
    let id = session["id"].as_str().unwrap();
    ready(&router, None, id).await;
    let (status, sent) = call(&router, "POST", &format!("/api/sessions/{id}/send"), None,
        Some(json!({"kind":"data_query","query_id":"cancel-query","sql":"SELECT sum(i) FROM range(10000000000) t(i)","read_only":true}))).await;
    assert_eq!(status, StatusCode::OK, "{sent}");
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let (_, events) = call(
                &router,
                "GET",
                &format!("/api/sessions/{id}/events"),
                None,
                None,
            )
            .await;
            if events["events"].as_array().unwrap().iter().any(|event| {
                event["message"]["kind"] == "data_started"
                    && event["message"]["query_id"] == "cancel-query"
            }) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let (status, stopped) = call(
        &router,
        "POST",
        &format!("/api/sessions/{id}/send"),
        None,
        Some(json!({"kind":"data_cancel","query_id":"cancel-query"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{stopped}");
    tokio::time::timeout(Duration::from_secs(6), async {
        loop {
            let (_, events) = call(
                &router,
                "GET",
                &format!("/api/sessions/{id}/events"),
                None,
                None,
            )
            .await;
            if events["events"].as_array().unwrap().iter().any(|event| {
                event["message"]["kind"] == "data_cancelled"
                    && event["message"]["query_id"] == "cancel-query"
            }) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let history = tokio::time::timeout(Duration::from_secs(6), async {
        loop {
            let (_, entries) =
                call(&router, "GET", "/api/workspaces/data/history", None, None).await;
            if let Some(entry) = entries.as_array().and_then(|rows| rows.first()) {
                return entry.clone();
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let body: serde_json::Value =
        serde_json::from_str(history["response"]["body"].as_str().unwrap()).unwrap();
    assert_eq!(body["query_id"], "cancel-query");
    assert_eq!(body["cancelled"], true);
    assert_eq!(body["write_outcome_unknown"], false);
    assert_eq!(history["response"]["status"], 499);
    assert_eq!(body["elapsed_ms"], history["response"]["elapsed_ms"]);
}

#[tokio::test]
async fn clearing_history_does_not_restore_an_already_finished_data_query() {
    let _serial = DATA_CLIENT_TESTS.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let router = moleapi_server::local_with_worker(&temp.path().join("data-clear.db"), &worker())
        .await
        .unwrap();
    let mut document = data();
    document["collections"][0]["requests"][0]["post_response_script"] = json!(
        "const until=Date.now()+50;while(Date.now()<until){};pm.test('after-clear',()=>pm.expect(true).to.equal(true));"
    );
    let (status, workspace) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"data","name":"Data","data":document})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{workspace}");
    let (status, session) = call(&router, "POST", "/api/sessions", None,
        Some(json!({"workspace_id":"data","request":workspace["data"]["collections"][0]["requests"][0]}))).await;
    assert_eq!(status, StatusCode::OK, "{session}");
    let id = session["id"].as_str().unwrap();
    ready(&router, None, id).await;
    query(
        &router,
        None,
        id,
        "first",
        "SELECT name FROM data WHERE id=2",
    )
    .await;
    query(
        &router,
        None,
        id,
        "clear-me",
        "SELECT name FROM data WHERE id=2",
    )
    .await;
    let (status, cleared) = call(
        &router,
        "DELETE",
        "/api/workspaces/data/history",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{cleared}");
    let processed = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let (_, events) = call(
                &router,
                "GET",
                &format!("/api/sessions/{id}/events"),
                None,
                None,
            )
            .await;
            if let Some(event) = events["events"].as_array().unwrap().iter().find(|event| {
                event["message"]["kind"] == "data_processed"
                    && event["message"]["query_id"] == "clear-me"
            }) {
                return event["message"].clone();
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert!(
        processed["tests"]
            .as_array()
            .unwrap()
            .iter()
            .any(|test| test["name"] == "after-clear" && test["passed"] == true),
        "{processed}"
    );
    let (_, history) = call(&router, "GET", "/api/workspaces/data/history", None, None).await;
    assert_eq!(history, json!([]));
}

#[tokio::test]
async fn data_completion_keeps_the_collection_chain_from_session_creation() {
    let _serial = DATA_CLIENT_TESTS.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let router = moleapi_server::local_with_worker(&temp.path().join("data-chain.db"), &worker())
        .await
        .unwrap();
    let mut document = data();
    document["collections"][0]["post_response_script"] =
        json!("pm.test('original-chain',()=>pm.expect(true).to.equal(true));");
    let mut moved = document["collections"][0].clone();
    moved["id"] = json!("new");
    moved["name"] = json!("New collection");
    moved["post_response_script"] =
        json!("pm.test('new-chain',()=>pm.expect(true).to.equal(true));");
    moved["requests"] = json!([]);
    document["collections"].as_array_mut().unwrap().push(moved);
    let (status, workspace) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"data","name":"Data","data":document})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{workspace}");
    let (status, session) = call(&router, "POST", "/api/sessions", None,
        Some(json!({"workspace_id":"data","request":workspace["data"]["collections"][0]["requests"][0]}))).await;
    assert_eq!(status, StatusCode::OK, "{session}");
    let id = session["id"].as_str().unwrap();
    ready(&router, None, id).await;
    let mut changed = workspace["data"].clone();
    changed["collections"][1]["requests"] = changed["collections"][0]["requests"].clone();
    changed["collections"][0]["requests"] = json!([]);
    let (status, updated) = call(
        &router,
        "PUT",
        "/api/workspaces/data",
        None,
        Some(json!({"name":"Data","expected_revision":workspace["revision"],"data":changed})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    query(
        &router,
        None,
        id,
        "chain",
        "SELECT name FROM data WHERE id=2",
    )
    .await;
    let processed = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let (_, events) = call(
                &router,
                "GET",
                &format!("/api/sessions/{id}/events"),
                None,
                None,
            )
            .await;
            if let Some(event) = events["events"]
                .as_array()
                .unwrap()
                .iter()
                .find(|event| event["message"]["kind"] == "data_processed")
            {
                return event["message"].clone();
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert!(
        processed["tests"]
            .as_array()
            .unwrap()
            .iter()
            .any(|test| test["name"] == "original-chain")
    );
    assert!(
        !processed["tests"]
            .as_array()
            .unwrap()
            .iter()
            .any(|test| test["name"] == "new-chain")
    );
}

#[tokio::test]
async fn full_data_completion_queue_reports_postprocessing_loss_without_sql_replay() {
    let _serial = DATA_CLIENT_TESTS.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let router =
        moleapi_server::local_with_worker(&temp.path().join("data-capacity.db"), &worker())
            .await
            .unwrap();
    let mut document = data();
    document["collections"][0]["requests"][0]["post_response_script"] =
        json!("const until=Date.now()+400;while(Date.now()<until){};");
    let (status, workspace) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"data","name":"Data","data":document})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{workspace}");
    let (status, session) = call(&router, "POST", "/api/sessions", None,
        Some(json!({"workspace_id":"data","request":workspace["data"]["collections"][0]["requests"][0]}))).await;
    assert_eq!(status, StatusCode::OK, "{session}");
    let id = session["id"].as_str().unwrap();
    ready(&router, None, id).await;
    for index in 0..8 {
        query(
            &router,
            None,
            id,
            &format!("queued-{index}"),
            "SELECT name FROM data WHERE id=2",
        )
        .await;
    }
    let (_, events) = call(
        &router,
        "GET",
        &format!("/api/sessions/{id}/events"),
        None,
        None,
    )
    .await;
    let events = events["events"].as_array().unwrap();
    let lost = events
        .iter()
        .find(|event| {
            event["message"]["kind"] == "data_postprocess_error"
                && event["message"]["message"]
                    .as_str()
                    .is_some_and(|text| text.contains("capacity unavailable"))
        })
        .expect("full completion queue did not report the dropped processing result");
    let query_id = &lost["message"]["query_id"];
    assert_eq!(
        events
            .iter()
            .filter(|event| event["message"]["kind"] == "data_finished"
                && &event["message"]["query_id"] == query_id)
            .count(),
        1
    );
    call(
        &router,
        "DELETE",
        &format!("/api/sessions/{id}"),
        None,
        None,
    )
    .await;
}

#[tokio::test]
async fn query_pre_script_mutates_each_sql_and_sees_prior_post_updates() {
    let _serial = DATA_CLIENT_TESTS.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let router = moleapi_server::local_with_worker(&temp.path().join("data-pre.db"), &worker())
        .await
        .unwrap();
    let mut document = data();
    let request = &mut document["collections"][0]["requests"][0];
    request["protocol"]["query_pre_script"] = json!(
        "const n=Number(pm.collectionVariables.get('query_count')||'0')+1;if(n>1)pm.test('post-visible',()=>pm.expect(pm.collectionVariables.get('post')).to.equal('ready'));pm.collectionVariables.set('query_count',String(n));pm.request.body.update('SELECT name FROM data WHERE id='+n);"
    );
    request["post_response_script"] = json!("pm.collectionVariables.set('post','ready');");
    let (status, workspace) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"data","name":"Data","data":document})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{workspace}");
    let (status, session) = call(&router, "POST", "/api/sessions", None,
        Some(json!({"workspace_id":"data","request":workspace["data"]["collections"][0]["requests"][0]}))).await;
    assert_eq!(status, StatusCode::OK, "{session}");
    let id = session["id"].as_str().unwrap();
    ready(&router, None, id).await;
    let first = query(
        &router,
        None,
        id,
        "pre-one",
        "SELECT name FROM data WHERE id=999",
    )
    .await;
    assert!(
        first["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["message"]["kind"] == "data_rows"
                && event["message"]["rows"][0][0]["value"] == "first"),
        "{first}"
    );
    let second = query(
        &router,
        None,
        id,
        "pre-two",
        "SELECT name FROM data WHERE id=999",
    )
    .await;
    assert!(
        second["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["message"]["kind"] == "data_rows"
                && event["message"]["rows"][0][0]["value"] == "second")
    );
    assert!(second["events"].as_array().unwrap().iter().any(|event| {
        event["message"]["kind"] == "data_prepared"
            && event["message"]["query_id"] == "pre-two"
            && event["message"]["tests"]
                .as_array()
                .unwrap()
                .iter()
                .any(|test| test["name"] == "post-visible" && test["passed"] == true)
    }));
    let (status, duplicate) = call(&router, "POST", &format!("/api/sessions/{id}/send"), None,
        Some(json!({"kind":"data_query","query_id":"pre-one","sql":"SELECT name FROM data WHERE id=999","read_only":true}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{duplicate}");
    let (_, events) = call(
        &router,
        "GET",
        &format!("/api/sessions/{id}/events"),
        None,
        None,
    )
    .await;
    assert_eq!(
        events["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event["message"]["kind"] == "data_started"
                && event["message"]["query_id"] == "pre-one")
            .count(),
        1
    );
}

#[tokio::test]
async fn rejected_query_pre_mutation_does_not_dispatch_or_save_sql() {
    let _serial = DATA_CLIENT_TESTS.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let router =
        moleapi_server::local_with_worker(&temp.path().join("data-pre-reject.db"), &worker())
            .await
            .unwrap();
    let mut document = data();
    document["collections"][0]["requests"][0]["protocol"]["query_pre_script"] = json!(
        "pm.request.url='http://changed.invalid/';pm.request.body.update('SELECT name FROM data WHERE id=2');"
    );
    let (status, workspace) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"data","name":"Data","data":document})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{workspace}");
    let (status, session) = call(&router, "POST", "/api/sessions", None,
        Some(json!({"workspace_id":"data","request":workspace["data"]["collections"][0]["requests"][0]}))).await;
    assert_eq!(status, StatusCode::OK, "{session}");
    let id = session["id"].as_str().unwrap();
    ready(&router, None, id).await;
    let events = query(
        &router,
        None,
        id,
        "pre-rejected",
        "SELECT name FROM data WHERE id=1",
    )
    .await;
    let events = events["events"].as_array().unwrap();
    assert!(
        events
            .iter()
            .any(|event| event["message"]["kind"] == "data_preprocess_error"
                && event["message"]["query_id"] == "pre-rejected")
    );
    assert!(
        !events
            .iter()
            .any(|event| event["message"]["kind"] == "data_started"
                && event["message"]["query_id"] == "pre-rejected")
    );
    let (_, history) = call(&router, "GET", "/api/workspaces/data/history", None, None).await;
    assert_eq!(history, json!([]));
}

#[tokio::test]
async fn cancelling_query_preparation_never_dispatches_sql() {
    let _serial = DATA_CLIENT_TESTS.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let router =
        moleapi_server::local_with_worker(&temp.path().join("data-pre-cancel.db"), &worker())
            .await
            .unwrap();
    let mut document = data();
    document["collections"][0]["requests"][0]["protocol"]["query_pre_script"] = json!(
        "const until=Date.now()+100;while(Date.now()<until){};pm.request.body.update('SELECT name FROM data WHERE id=2');"
    );
    let (status, workspace) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"data","name":"Data","data":document})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{workspace}");
    let (status, session) = call(&router, "POST", "/api/sessions", None,
        Some(json!({"workspace_id":"data","request":workspace["data"]["collections"][0]["requests"][0]}))).await;
    assert_eq!(status, StatusCode::OK, "{session}");
    let id = session["id"].as_str().unwrap().to_owned();
    ready(&router, None, &id).await;
    let sender = router.clone();
    let query_id = id.clone();
    let task = tokio::spawn(async move {
        call(&sender, "POST", &format!("/api/sessions/{query_id}/send"), None,
        Some(json!({"kind":"data_query","query_id":"pre-cancel","sql":"SELECT name FROM data WHERE id=1","read_only":true}))).await
    });
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let (_, events) = call(
                &router,
                "GET",
                &format!("/api/sessions/{id}/events"),
                None,
                None,
            )
            .await;
            if events["events"].as_array().unwrap().iter().any(|event| {
                event["message"]["kind"] == "data_preparing"
                    && event["message"]["query_id"] == "pre-cancel"
            }) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let (status, cancelled) = call(
        &router,
        "POST",
        &format!("/api/sessions/{id}/send"),
        None,
        Some(json!({"kind":"data_cancel","query_id":"pre-cancel"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{cancelled}");
    assert_eq!(task.await.unwrap().0, StatusCode::OK);
    let (_, events) = call(
        &router,
        "GET",
        &format!("/api/sessions/{id}/events"),
        None,
        None,
    )
    .await;
    let events = events["events"].as_array().unwrap();
    assert!(
        events
            .iter()
            .any(|event| event["message"]["kind"] == "data_cancelled"
                && event["message"]["query_id"] == "pre-cancel")
    );
    assert!(
        !events
            .iter()
            .any(|event| event["message"]["kind"] == "data_started"
                && event["message"]["query_id"] == "pre-cancel")
    );
    let (_, history) = call(&router, "GET", "/api/workspaces/data/history", None, None).await;
    assert_eq!(history, json!([]));
}

#[tokio::test]
async fn query_pre_script_skip_keeps_the_connection_and_runs_no_sql() {
    let _serial = DATA_CLIENT_TESTS.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let router =
        moleapi_server::local_with_worker(&temp.path().join("data-pre-skip.db"), &worker())
            .await
            .unwrap();
    let mut document = data();
    document["collections"][0]["requests"][0]["protocol"]["query_pre_script"] =
        json!("pm.execution.skipRequest();");
    let (status, workspace) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"data","name":"Data","data":document})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{workspace}");
    let (status, session) = call(&router, "POST", "/api/sessions", None,
        Some(json!({"workspace_id":"data","request":workspace["data"]["collections"][0]["requests"][0]}))).await;
    assert_eq!(status, StatusCode::OK, "{session}");
    let id = session["id"].as_str().unwrap();
    ready(&router, None, id).await;
    let events = query(
        &router,
        None,
        id,
        "skipped",
        "SELECT name FROM data WHERE id=1",
    )
    .await;
    let events = events["events"].as_array().unwrap();
    assert!(
        events
            .iter()
            .any(|event| event["message"]["kind"] == "data_skipped"
                && event["message"]["query_id"] == "skipped"),
        "{events:?}"
    );
    assert!(
        events
            .iter()
            .any(|event| event["message"]["kind"] == "data_prepared"
                && event["message"]["query_id"] == "skipped")
    );
    assert!(
        !events
            .iter()
            .any(|event| event["message"]["kind"] == "data_started"
                && event["message"]["query_id"] == "skipped")
    );
    let (_, summary) = call(&router, "GET", &format!("/api/sessions/{id}"), None, None).await;
    assert_eq!(summary["state"], "open");
    let (_, history) = call(&router, "GET", "/api/workspaces/data/history", None, None).await;
    assert_eq!(history, json!([]));
}

#[tokio::test]
async fn query_pre_script_cannot_turn_a_read_only_query_into_a_write() {
    let _serial = DATA_CLIENT_TESTS.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let router =
        moleapi_server::local_with_worker(&temp.path().join("data-pre-write.db"), &worker())
            .await
            .unwrap();
    let mut document = data();
    document["collections"][0]["requests"][0]["protocol"]["query_pre_script"] =
        json!("pm.request.body.update('DELETE FROM data');");
    let (status, workspace) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"data","name":"Data","data":document})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{workspace}");
    let (status, session) = call(&router, "POST", "/api/sessions", None,
        Some(json!({"workspace_id":"data","request":workspace["data"]["collections"][0]["requests"][0]}))).await;
    assert_eq!(status, StatusCode::OK, "{session}");
    let id = session["id"].as_str().unwrap();
    ready(&router, None, id).await;
    let events = query(
        &router,
        None,
        id,
        "no-write",
        "SELECT name FROM data WHERE id=1",
    )
    .await;
    let events = events["events"].as_array().unwrap();
    assert!(
        events
            .iter()
            .any(|event| event["message"]["kind"] == "data_preprocess_error"
                && event["message"]["query_id"] == "no-write")
    );
    assert!(
        !events
            .iter()
            .any(|event| event["message"]["kind"] == "data_started"
                && event["message"]["query_id"] == "no-write")
    );
    let (_, history) = call(&router, "GET", "/api/workspaces/data/history", None, None).await;
    assert_eq!(history, json!([]));
}

#[tokio::test]
async fn query_pre_script_masks_private_logs_and_update_values() {
    let _serial = DATA_CLIENT_TESTS.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let router =
        moleapi_server::local_with_worker(&temp.path().join("data-pre-private.db"), &worker())
            .await
            .unwrap();
    let mut document = data();
    document["global_variables"] = json!([{"id":"secret","key":"secret","value":"private-pre-value","enabled":true,"secret":true}]);
    document["collections"][0]["requests"][0]["protocol"]["query_pre_script"] = json!(
        "console.log(pm.globals.get('secret'));pm.variables.set('copied',pm.globals.get('secret'));pm.variables.set(pm.globals.get('secret'),'x');pm.request.body.update('SELECT name FROM data WHERE id=2');"
    );
    let (status, workspace) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"data","name":"Data","data":document})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{workspace}");
    let (status, session) = call(&router, "POST", "/api/sessions", None,
        Some(json!({"workspace_id":"data","request":workspace["data"]["collections"][0]["requests"][0]}))).await;
    assert_eq!(status, StatusCode::OK, "{session}");
    let id = session["id"].as_str().unwrap();
    ready(&router, None, id).await;
    let events = query(
        &router,
        None,
        id,
        "private-pre",
        "SELECT name FROM data WHERE id=1",
    )
    .await;
    assert!(
        !events.to_string().contains("private-pre-value"),
        "{events}"
    );
    let prepared = events["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|event| event["message"]["kind"] == "data_prepared")
        .unwrap();
    assert_eq!(prepared["message"]["logs"][0]["message"], "[REDACTED]");
    assert!(
        prepared["message"]["variable_updates"]
            .as_array()
            .unwrap()
            .iter()
            .all(|update| update["value"].is_null())
    );
    assert!(
        prepared["message"]["variable_updates"]
            .as_array()
            .unwrap()
            .iter()
            .any(|update| update["key"] == "[REDACTED]")
    );
}

#[tokio::test]
async fn query_pre_script_failure_masks_private_error_and_never_dispatches() {
    let _serial = DATA_CLIENT_TESTS.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let router =
        moleapi_server::local_with_worker(&temp.path().join("data-pre-failure.db"), &worker())
            .await
            .unwrap();
    let mut document = data();
    document["global_variables"] = json!([{"id":"secret","key":"secret","value":"failure-private-value","enabled":true,"secret":true}]);
    document["collections"][0]["requests"][0]["protocol"]["query_pre_script"] =
        json!("throw new Error(pm.globals.get('secret'));");
    let (status, workspace) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"data","name":"Data","data":document})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{workspace}");
    let (status, session) = call(&router, "POST", "/api/sessions", None,
        Some(json!({"workspace_id":"data","request":workspace["data"]["collections"][0]["requests"][0]}))).await;
    assert_eq!(status, StatusCode::OK, "{session}");
    let id = session["id"].as_str().unwrap();
    ready(&router, None, id).await;
    let events = query(
        &router,
        None,
        id,
        "failed-pre",
        "SELECT name FROM data WHERE id=1",
    )
    .await;
    assert!(
        !events.to_string().contains("failure-private-value"),
        "{events}"
    );
    assert!(events["events"].as_array().unwrap().iter().any(|event| {
        event["message"]["kind"] == "data_preprocess_error"
            && event["message"]["message"]
                .as_str()
                .is_some_and(|text| text.contains("REDACTED"))
    }));
    assert!(
        !events["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["message"]["kind"] == "data_started")
    );
}

#[tokio::test]
async fn query_pre_private_value_is_masked_in_database_error_events() {
    let _serial = DATA_CLIENT_TESTS.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let router =
        moleapi_server::local_with_worker(&temp.path().join("data-pre-mask-worker.db"), &worker())
            .await
            .unwrap();
    let mut document = data();
    document["collections"][0]["requests"][0]["protocol"]["query_pre_script"] = json!(
        "pm.variables.set('dynamic_secret','new-private-value');pm.request.body.update(\"SELECT CAST('{{dynamic_secret}}' AS BIGINT) AS value FROM data\");"
    );
    let (status, workspace) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"data","name":"Data","data":document})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{workspace}");
    let (status, session) = call(&router, "POST", "/api/sessions", None,
        Some(json!({"workspace_id":"data","request":workspace["data"]["collections"][0]["requests"][0]}))).await;
    assert_eq!(status, StatusCode::OK, "{session}");
    let id = session["id"].as_str().unwrap();
    ready(&router, None, id).await;
    let events = query(
        &router,
        None,
        id,
        "masked-worker",
        "SELECT name FROM data WHERE id=1",
    )
    .await;
    assert!(
        events["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["message"]["kind"] == "data_error"),
        "{events}"
    );
    assert!(
        !events.to_string().contains("new-private-value"),
        "{events}"
    );
    let error = events["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|event| event["message"]["kind"] == "data_error")
        .unwrap();
    assert!(
        error["message"]["message"]
            .as_str()
            .unwrap()
            .contains("REDACTED"),
        "{events}"
    );
}

#[tokio::test]
async fn oversized_preparation_event_rolls_back_variable_updates() {
    let _serial = DATA_CLIENT_TESTS.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let router =
        moleapi_server::local_with_worker(&temp.path().join("data-pre-event-limit.db"), &worker())
            .await
            .unwrap();
    let mut document = data();
    document["collections"][0]["requests"][0]["protocol"]["query_pre_script"] = json!(
        "if(!pm.variables.get('attempted')){pm.variables.set('attempted','yes');for(let i=0;i<999;i++)pm.variables.set('k'.repeat(1020)+String(i).padStart(4,'0'),'x');}pm.request.body.update('SELECT name FROM data WHERE id=2');"
    );
    let (status, workspace) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"data","name":"Data","data":document})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{workspace}");
    let (status, session) = call(&router, "POST", "/api/sessions", None,
        Some(json!({"workspace_id":"data","request":workspace["data"]["collections"][0]["requests"][0]}))).await;
    assert_eq!(status, StatusCode::OK, "{session}");
    let id = session["id"].as_str().unwrap();
    ready(&router, None, id).await;
    for query_id in ["first-large", "second-large"] {
        let events = query(
            &router,
            None,
            id,
            query_id,
            "SELECT name FROM data WHERE id=1",
        )
        .await;
        assert!(
            events["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|event| event["message"]["kind"] == "data_preprocess_error"
                    && event["message"]["query_id"] == query_id
                    && event["message"]["message"]
                        .as_str()
                        .is_some_and(|text| text.contains("event limits"))),
            "{events}"
        );
        assert!(
            !events["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|event| event["message"]["kind"] == "data_started"
                    && event["message"]["query_id"] == query_id)
        );
    }
}
