mod common;
use base64::{Engine, engine::general_purpose::STANDARD};
use common::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[tokio::test]
async fn member_runs_use_canonical_saved_temporary_and_query_data_without_mutating_workspace() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::get(
            move |axum::extract::Query(value): axum::extract::Query<
                std::collections::HashMap<String, String>,
            >| {
                let counter = counter.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    axum::Json(json!(value))
                }
            },
        ),
    ))
    .await;
    let directory = tempfile::tempdir().unwrap();
    let mut settings = config(
        moleapi_server::sqlite_database_url(&directory.path().join("runs.db")).unwrap(),
        true,
    );
    settings.allow_private_network = true;
    let router = moleapi_server::hosted_with_worker(
        settings,
        std::path::Path::new(env!("CARGO_BIN_EXE_moleapi-server")),
    )
    .await
    .unwrap();
    let owner = register(&router, "runowner").await;
    let editor = register(&router, "runeditor").await;
    let viewer = register(&router, "runviewer").await;
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = format!("{url}/?value={{{{value}}}}").into();
    let mut query = data["collections"][0]["requests"][0].clone();
    query["id"] = "query".into();
    query["url"] = "".into();
    query["protocol"] = json!({"kind":"data","source":"local_file","file_name":"rows.csv","file_format":"csv","file_base64":STANDARD.encode(b"value\nfresh-one\nfresh-two\n"),"table_name":"data","sql":"SELECT value FROM data","read_only":true});
    data["collections"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"sources","name":"Sources","description":"","requests":[query]}));
    data["datasets"] = json!([
        {"id":"d","name":"Rows","source":{"format":"csv","source":"value\nowner-alpha\nowner-beta\n"}},
        {"id":"fresh","name":"Fresh","query":{"request_id":"query"}},
        {"id":"missing","name":"Missing"}
    ]);
    data["scenarios"] = json!([{"id":"s","name":"Scenario","description":"","collection_id":"c","steps":[{"id":"step","request_id":"r"}]}]);
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Runs","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let mut collision = data.clone();
    collision["datasets"][0]["source"]["source"] = "value\nwrong-member-data\n".into();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            Some(&editor),
            Some(json!({"id":"w","name":"Own collision","data":collision}))
        )
        .await
        .0,
        StatusCode::OK
    );
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
    for (options, expected) in [
        (json!({"dataset_id":"d"}), vec!["owner-alpha", "owner-beta"]),
        (
            json!({"dataset_id":"d","scenario_id":"s","iterations":1}),
            vec!["owner-alpha"],
        ),
        (
            json!({"dataset":{"format":"csv","source":"value\ntemporary-gamma\ntemporary-delta\n"}}),
            vec!["temporary-gamma", "temporary-delta"],
        ),
        (
            json!({"dataset_id":"fresh"}),
            vec!["fresh-one", "fresh-two"],
        ),
    ] {
        let mut input = options;
        input["collection_id"] = "c".into();
        let (status, result) = call(
            &router,
            "POST",
            &format!("{base}/run"),
            Some(&editor),
            Some(input),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{result}");
        assert_eq!(result["passed"], expected.len());
        for (row, value) in result["results"].as_array().unwrap().iter().zip(expected) {
            let body: Value =
                serde_json::from_str(row["response"]["body"].as_str().unwrap()).unwrap();
            assert_eq!(body["value"], value);
        }
    }
    let before = calls.load(Ordering::SeqCst);
    assert_eq!(before, 7);
    for options in [
        json!({"dataset_id":"deleted"}),
        json!({"dataset_id":"missing"}),
        json!({"iterations":101}),
        json!({"dataset":{"format":"json","source":"invalid"}}),
    ] {
        let mut input = options;
        input["collection_id"] = "c".into();
        let (status, _) = call(
            &router,
            "POST",
            &format!("{base}/run"),
            Some(&editor),
            Some(input),
        )
        .await;
        assert!(status.is_client_error(), "{status}");
    }
    assert_eq!(
        call(
            &router,
            "POST",
            &format!("{base}/run"),
            Some(&viewer),
            Some(json!({"collection_id":"c","dataset_id":"d"}))
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(calls.load(Ordering::SeqCst), before);
    let (_, current) = call(&router, "GET", "/api/workspaces/w", Some(&owner), None).await;
    assert_eq!(current["data"], saved["data"]);
    let mut broken = current["data"].clone();
    broken["collections"][1]["requests"][0]["protocol"]["sql"] = "SELECT missing FROM data".into();
    assert_eq!(
        call(
            &router,
            "PUT",
            "/api/workspaces/w",
            Some(&owner),
            Some(json!({"name":"Runs","expected_revision":current["revision"],"data":broken}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, result) = call(
        &router,
        "POST",
        &format!("{base}/run"),
        Some(&editor),
        Some(json!({"collection_id":"c","dataset_id":"fresh"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["stopped_reason"], "dataset_refresh_failed");
    assert_eq!(calls.load(Ordering::SeqCst), before);
    fixture.abort();
}
