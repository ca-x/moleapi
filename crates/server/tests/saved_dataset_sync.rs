mod common;
use common::*;
use std::sync::{Arc, Mutex};
#[tokio::test]
async fn old_server_dropping_saved_sources_cannot_acknowledge_native_sync() {
    let remote_state = Arc::new(Mutex::new(None::<Value>));
    let read = remote_state.clone();
    let write = remote_state.clone();
    let remote=Router::new().route("/api/auth/login",axum::routing::post(||async{axum::Json(json!({"token":"dataset-sync-token"}))})).route("/api/workspaces/w",axum::routing::get(move||{let read=read.clone();async move{match read.lock().unwrap().clone(){Some(value)=>(StatusCode::OK,axum::Json(value)),None=>(StatusCode::NOT_FOUND,axum::Json(json!({"error":"missing"})))}}})).route("/api/workspaces",axum::routing::post(move|axum::Json(mut body):axum::Json<Value>|{let write=write.clone();async move{body["data"].as_object_mut().unwrap().remove("datasets");let workspace=json!({"id":body["id"],"name":body["name"],"data":body["data"],"revision":1,"updated_at":"now"});*write.lock().unwrap()=Some(workspace.clone());axum::Json(workspace)}}));
    let (url, fixture) = serve(remote).await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("sync.db")).await.unwrap();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/sync/connect",
            None,
            Some(json!({"server_url":url,"username":"dataset-user","password":"dataset-password"}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let mut data = example_data();
    data["datasets"] =
        json!([{"id":"d","name":"Saved","source":{"format":"csv","source":"id\n1\n"}}]);
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"Dataset sync","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, result) = call(
        &router,
        "POST",
        "/api/workspaces/w/sync",
        None,
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{result}");
    assert!(result.to_string().contains("did not preserve"));
    let (_, saved) = call(&router, "GET", "/api/workspaces/w", None, None).await;
    assert_eq!(saved["data"]["datasets"][0]["source"]["source"], "id\n1\n");
    fixture.abort();
}
