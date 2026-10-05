mod common;
use base64::{Engine, engine::general_purpose::STANDARD};
use common::*;
use std::time::Duration;
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
                        Some("data_finished" | "data_error")
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
