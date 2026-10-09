mod common;
use common::*;
#[tokio::test]
async fn binary_snippets_preserve_mime_and_ownership_without_exporting_saved_bytes() {
    let temp = tempfile::tempdir().unwrap();
    let router = hosted(config(
        format!(
            "sqlite://{}?mode=rwc",
            temp.path().join("binary.db").display()
        ),
        true,
    ))
    .await
    .unwrap();
    let owner = register(&router, "binary-owner").await;
    let other = register(&router, "binary-other").await;
    let mut data = example_data();
    let request = &mut data["collections"][0]["requests"][0];
    request["method"] = "POST".into();
    request["body_kind"] = "binary".into();
    let source =
        json!({"file_name":"upload.bin","mime":"application/x-moleapi-file","base64":"AP+AClg="})
            .to_string();
    request["body"] = source.clone().into();
    request["headers"] = json!([{"id":"ct","key":"Content-Type","value":"application/x-header-file","enabled":true}]);
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            Some(&owner),
            Some(json!({"id":"w","name":"Binary","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let payload =
        json!({"workspace_id":"w","request_id":"r","target":"csharp","client":"restsharp"});
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
    for include in [false, true] {
        let mut payload = payload.clone();
        payload["include_secrets"] = include.into();
        let (status, result) = call(
            &router,
            "POST",
            "/api/generation/snippets",
            Some(&owner),
            Some(payload),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{result}");
        assert!(
            result["code"].as_str().unwrap().contains("upload.bin")
                && result["code"]
                    .as_str()
                    .unwrap()
                    .contains("application/x-header-file")
        );
        assert!(!result.to_string().contains("AP+AClg="));
        assert!(!result["code"].as_str().unwrap().contains("application/x-moleapi-file"));
    }
    assert_eq!(
        call(&router, "GET", "/api/workspaces/w", Some(&owner), None)
            .await
            .1["data"]["collections"][0]["requests"][0]["body"],
        source
    );
}
