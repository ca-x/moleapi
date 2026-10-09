mod common;
use common::*;
#[tokio::test]
async fn multipart_snippet_api_is_owned_and_does_not_expose_saved_bytes_or_credentials() {
    let temp = tempfile::tempdir().unwrap();
    let router = hosted(config(
        format!(
            "sqlite://{}?mode=rwc",
            temp.path().join("multipart.db").display()
        ),
        true,
    ))
    .await
    .unwrap();
    let owner = register(&router, "multipart-owner").await;
    let other = register(&router, "multipart-other").await;
    let mut data = example_data();
    let request = &mut data["collections"][0]["requests"][0];
    request["method"] = "POST".into();
    request["body_kind"] = "multipart".into();
    let source=json!({"parts":[{"id":"password","name":"password","value":{"kind":"text","text":"multipart-secret","mime":""}},{"id":"upload","name":"upload","value":{"kind":"file","file":{"file_name":"upload.bin","mime":"application/octet-stream","base64":"AP+AClg="}}}]}).to_string();
    request["body"] = source.clone().into();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            Some(&owner),
            Some(json!({"id":"w","name":"Multipart","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let payload = json!({"workspace_id":"w","request_id":"r","target":"node","client":"fetch"});
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/generation/snippets",
            Some(&other),
            Some(payload.clone())
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, result) = call(
        &router,
        "POST",
        "/api/generation/snippets",
        Some(&owner),
        Some(payload.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert!(result["code"].as_str().unwrap().contains("upload.bin"));
    assert!(
        !result.to_string().contains("multipart-secret")
            && !result.to_string().contains("AP+AClg=")
    );
    let mut explicit = payload;
    explicit["include_secrets"] = true.into();
    let (status, result) = call(
        &router,
        "POST",
        "/api/generation/snippets",
        Some(&owner),
        Some(explicit),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert!(
        result["code"]
            .as_str()
            .unwrap()
            .contains("multipart-secret")
    );
    assert!(!result.to_string().contains("AP+AClg="));
    assert_eq!(
        call(&router, "GET", "/api/workspaces/w", Some(&owner), None)
            .await
            .1["data"]["collections"][0]["requests"][0]["body"],
        source
    );
}
