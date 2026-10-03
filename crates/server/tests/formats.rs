mod common;
use common::*;
#[tokio::test]
async fn import_export_routes_validate_formats_and_default_to_secret_redaction() {
    let dir = tempfile::tempdir().unwrap();
    let router = common::local(&dir.path().join("local.db")).await.unwrap();
    let(status,imported)=call(&router,"POST","/api/import",None,Some(json!({"format":"curl","content":"curl -X POST 'https://example.com/resource?api_key=private-key' -H 'Authorization: Bearer private-token' --data-raw 'payload'"}))).await;
    assert_eq!(status, StatusCode::OK, "{imported}");
    assert_eq!(
        imported["data"]["collections"][0]["requests"][0]["method"],
        "POST"
    );
    let (status, _) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"import","name":"Imported","data":imported["data"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, exported) = call(
        &router,
        "POST",
        "/api/workspaces/import/export",
        None,
        Some(json!({"format":"moleapi","include_secrets":false})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{exported}");
    assert!(
        !exported["content"]
            .as_str()
            .unwrap()
            .contains("private-token")
    );
    assert!(
        !exported["content"]
            .as_str()
            .unwrap()
            .contains("private-key")
    );
    assert_eq!(exported["mime"], "application/json");
    let (_, included) = call(
        &router,
        "POST",
        "/api/workspaces/import/export",
        None,
        Some(json!({"format":"moleapi","include_secrets":true})),
    )
    .await;
    assert!(
        included["content"]
            .as_str()
            .unwrap()
            .contains("private-token")
    );
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/import",
            None,
            Some(json!({"format":"unknown","content":"payload"}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces/missing/export",
            None,
            Some(json!({"format":"moleapi","include_secrets":false}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}
