mod common;
use common::*;

#[tokio::test]
async fn request_history_finds_changes_without_exposing_unchanged_or_foreign_snapshots() {
    let directory = tempfile::tempdir().unwrap();
    let router = hosted(config(
        format!(
            "sqlite://{}?mode=rwc",
            directory.path().join("versions.db").display()
        ),
        true,
    ))
    .await
    .unwrap();
    let owner = register(&router, "versionowner").await;
    let foreign = register(&router, "versionother").await;
    let mut original = example_data();
    original["collections"][0]["requests"][0]["name"] = json!("Alpha");
    original["collections"][0]["requests"][0]["body"] = json!("history-private-value");
    let (status, created) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"history-w","name":"Workspace","data":original.clone()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let mut changed = original.clone();
    changed["collections"][0]["requests"][0]["name"] = json!("Beta");
    let (status, updated) = call(
        &router,
        "PUT",
        "/api/workspaces/history-w",
        Some(&owner),
        Some(json!({"name":"Workspace","data":changed.clone(),"expected_revision":1})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    let (status, unchanged) = call(
        &router,
        "PUT",
        "/api/workspaces/history-w",
        Some(&owner),
        Some(json!({"name":"Renamed workspace","data":changed.clone(),"expected_revision":2})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{unchanged}");
    changed["collections"][0]["requests"] = json!([]);
    changed["collections"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"d","name":"Other","description":"","requests":[]}));
    let (status, deleted) = call(
        &router,
        "PUT",
        "/api/workspaces/history-w",
        Some(&owner),
        Some(json!({"name":"Renamed workspace","data":changed,"expected_revision":3})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{deleted}");
    let list = "/api/workspaces/history-w/requests/r/versions";
    let (status, history) = call(&router, "GET", list, Some(&owner), None).await;
    assert_eq!(status, StatusCode::OK, "{history}");
    assert_eq!(history["versions"].as_array().unwrap().len(), 2);
    assert_eq!(history["versions"][0]["revision"], 3);
    assert_eq!(history["versions"][0]["name"], "Beta");
    assert_eq!(history["versions"][1]["revision"], 1);
    assert_eq!(history["versions"][1]["name"], "Alpha");
    assert_eq!(history["next_before"], serde_json::Value::Null);
    assert!(!history.to_string().contains("history-private-value"));
    let detail = format!("{list}/1");
    let (status, request) = call(&router, "GET", &detail, Some(&owner), None).await;
    assert_eq!(status, StatusCode::OK, "{request}");
    assert_eq!(request["request"]["name"], "Alpha");
    assert_eq!(request["request"]["body"], "history-private-value");
    assert_eq!(request["collection_id"], "c");
    for path in [list, detail.as_str()] {
        assert_eq!(
            call(&router, "GET", path, Some(&foreign), None).await.0,
            StatusCode::NOT_FOUND
        );
    }
    let restore = format!("{list}/3/restore");
    assert_eq!(
        call(
            &router,
            "POST",
            &restore,
            Some(&foreign),
            Some(json!({"expected_revision":4}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &router,
            "POST",
            &restore,
            Some(&owner),
            Some(json!({"expected_revision":4,"destination_collection_id":"d"}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (status, restored) = call(
        &router,
        "POST",
        &restore,
        Some(&owner),
        Some(json!({"expected_revision":4})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{restored}");
    assert_eq!(restored["revision"], 5);
    assert_eq!(restored["name"], "Renamed workspace");
    assert_eq!(
        restored["data"]["collections"][0]["requests"][0]["name"],
        "Beta"
    );
    let (status, after_recreation) = call(&router, "GET", list, Some(&owner), None).await;
    assert_eq!(status, StatusCode::OK, "{after_recreation}");
    assert_eq!(after_recreation["versions"][0]["revision"], 4);
    assert_eq!(after_recreation["versions"][0]["exists"], false);
    assert_eq!(
        call(
            &router,
            "POST",
            &restore,
            Some(&owner),
            Some(json!({"expected_revision":4}))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(
            &router,
            "POST",
            &restore,
            Some(&owner),
            Some(json!({"expected_revision":5}))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let restore_alpha = format!("{list}/1/restore");
    let (first, second) = tokio::join!(
        call(
            &router,
            "POST",
            &restore_alpha,
            Some(&owner),
            Some(json!({"expected_revision":5}))
        ),
        call(
            &router,
            "POST",
            &restore_alpha,
            Some(&owner),
            Some(json!({"expected_revision":5}))
        )
    );
    assert!(
        (first.0 == StatusCode::OK && second.0 == StatusCode::CONFLICT)
            || (second.0 == StatusCode::OK && first.0 == StatusCode::CONFLICT),
        "{first:?} {second:?}"
    );
}

#[tokio::test]
async fn request_history_pages_past_unrelated_changes_and_marks_prior_absence() {
    let directory = tempfile::tempdir().unwrap();
    let router = local(&directory.path().join("paged.db")).await.unwrap();
    let original = data();
    let (status, created) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"paged-w","name":"New","data":original})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let request_data = example_data();
    let (status, updated) = call(
        &router,
        "PUT",
        "/api/workspaces/paged-w",
        None,
        Some(json!({"name":"New","data":request_data.clone(),"expected_revision":1})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    for revision in 2..=36 {
        let (status, next) = call(&router, "PUT", "/api/workspaces/paged-w", None, Some(json!({"name":format!("Renamed {revision}"),"data":request_data.clone(),"expected_revision":revision}))).await;
        assert_eq!(status, StatusCode::OK, "{next}");
    }
    let path = "/api/workspaces/paged-w/requests/r/versions";
    let (status, first) = call(&router, "GET", path, None, None).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert!(first["versions"].as_array().unwrap().is_empty());
    assert_eq!(first["next_before"], 5);
    let (status, second) = call(&router, "GET", &format!("{path}?before=5"), None, None).await;
    assert_eq!(status, StatusCode::OK, "{second}");
    assert_eq!(second["versions"].as_array().unwrap().len(), 1);
    assert_eq!(second["versions"][0]["revision"], 1);
    assert_eq!(second["versions"][0]["exists"], false);
    assert_eq!(second["next_before"], serde_json::Value::Null);
    let (status, detail) = call(&router, "GET", &format!("{path}/1"), None, None).await;
    assert_eq!(status, StatusCode::OK, "{detail}");
    assert!(detail["request"].is_null());
    let (status, share) = call(
        &router,
        "POST",
        "/api/workspaces/paged-w/mock-shares",
        None,
        Some(json!({"request_id":"r","expires_in_days":7})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{share}");
    let public = format!("/api/public/mock/{}/e", share["token"].as_str().unwrap());
    assert_eq!(
        call(&router, "GET", &public, None, None).await.0,
        StatusCode::CREATED
    );
    let (status, restored) = call(
        &router,
        "POST",
        &format!("{path}/1/restore"),
        None,
        Some(json!({"expected_revision":37})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{restored}");
    assert_eq!(restored["revision"], 38);
    assert!(
        restored["data"]["collections"][0]["requests"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        call(&router, "GET", &public, None, None).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn restoring_deleted_request_needs_a_live_destination_and_valid_dependencies() {
    let directory = tempfile::tempdir().unwrap();
    let router = local(&directory.path().join("destination.db"))
        .await
        .unwrap();
    let specification =
        json!({"id":"spec","name":"Spec","kind":"openapi","dialect":"3.0.0","source":"{}"});
    let mut original = example_data();
    original["specifications"] = json!([specification.clone()]);
    original["collections"][0]["requests"][0]["specification_id"] = json!("spec");
    let (status, created) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"destination-w","name":"Original","data":original})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let mut current = data();
    current["collections"] = json!([{"id":"d","name":"Other","description":"","requests":[]}]);
    let (status, updated) = call(
        &router,
        "PUT",
        "/api/workspaces/destination-w",
        None,
        Some(json!({"name":"Current","data":current.clone(),"expected_revision":1})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    let restore = "/api/workspaces/destination-w/requests/r/versions/1/restore";
    assert_eq!(
        call(
            &router,
            "POST",
            restore,
            None,
            Some(json!({"expected_revision":2}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &router,
            "POST",
            restore,
            None,
            Some(json!({"expected_revision":2,"destination_collection_id":"d"}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (status, unchanged) =
        call(&router, "GET", "/api/workspaces/destination-w", None, None).await;
    assert_eq!(status, StatusCode::OK, "{unchanged}");
    assert_eq!(unchanged["revision"], 2);
    current["specifications"] = json!([specification]);
    let (status, ready) = call(
        &router,
        "PUT",
        "/api/workspaces/destination-w",
        None,
        Some(json!({"name":"Current","data":current,"expected_revision":2})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{ready}");
    let (status, restored) = call(
        &router,
        "POST",
        restore,
        None,
        Some(json!({"expected_revision":3,"destination_collection_id":"d"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{restored}");
    assert_eq!(restored["revision"], 4);
    assert_eq!(restored["name"], "Current");
    assert_eq!(restored["data"]["collections"][0]["id"], "d");
    assert_eq!(restored["data"]["collections"][0]["requests"][0]["id"], "r");
}

#[tokio::test]
async fn large_archived_payload_stays_out_of_the_history_list() {
    let directory = tempfile::tempdir().unwrap();
    let router = local(&directory.path().join("large-history.db"))
        .await
        .unwrap();
    let mut original = example_data();
    original["collections"][0]["requests"][0]["examples"][0]["body"] =
        json!("private-historical-body-".repeat(45_000));
    let (status, created) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"large-w","name":"Large","data":original.clone()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let mut current = original;
    current["collections"][0]["requests"][0]["name"] = json!("Changed");
    let (status, updated) = call(
        &router,
        "PUT",
        "/api/workspaces/large-w",
        None,
        Some(json!({"name":"Large","data":current,"expected_revision":1})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    let path = "/api/workspaces/large-w/requests/r/versions";
    let (status, list) = call(&router, "GET", path, None, None).await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert!(list.to_string().len() < 1000);
    assert!(!list.to_string().contains("private-historical-body"));
    let (status, detail) = call(&router, "GET", &format!("{path}/1"), None, None).await;
    assert_eq!(status, StatusCode::OK, "{detail}");
    assert!(
        detail["request"]["examples"][0]["body"]
            .as_str()
            .unwrap()
            .len()
            > 1_000_000
    );
}
