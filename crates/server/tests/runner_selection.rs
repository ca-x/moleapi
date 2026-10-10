mod common;
use common::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
#[tokio::test]
async fn request_filters_are_subtree_bound_and_preserve_collection_order() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::get(move || {
            let counter = counter.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                "ok"
            }
        }),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("filter.db")).await.unwrap();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = format!("{url}/").into();
    let mut second = data["collections"][0]["requests"][0].clone();
    second["id"] = "second".into();
    data["collections"][0]["requests"]
        .as_array_mut()
        .unwrap()
        .push(second.clone());
    second["id"] = "child-request".into();
    data["collections"].as_array_mut().unwrap().push(json!({"id":"child","name":"Child","description":"","parent_id":"c","requests":[second.clone()]}));
    second["id"] = "outside".into();
    data["collections"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"other","name":"Other","description":"","requests":[second]}));
    data["scenarios"] = json!([{"id":"s","name":"Scenario","collection_id":"c","steps":[{"id":"step","request_id":"r"}]}]);
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"Filters","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    for ids in [
        json!([]),
        json!(["r", "r"]),
        json!(["outside"]),
        json!(["missing"]),
    ] {
        assert_eq!(
            call(
                &router,
                "POST",
                "/api/workspaces/w/run",
                None,
                Some(json!({"collection_id":"c","request_ids":ids}))
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces/w/run",
            None,
            Some(json!({"collection_id":"c","scenario_id":"s","request_ids":["r"]}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let (status, result) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        None,
        Some(json!({"collection_id":"c","request_ids":["child-request","r"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["passed"], 2);
    assert_eq!(result["results"][0]["request_id"], "r");
    assert_eq!(result["results"][1]["request_id"], "child-request");
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    fixture.abort();
}
