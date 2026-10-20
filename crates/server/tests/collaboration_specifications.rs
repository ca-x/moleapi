mod common;
use common::*;
#[tokio::test]
async fn member_openapi_preview_preserves_canonical_data_and_requires_revision_and_editor() {
    let dir = tempfile::tempdir().unwrap();
    let router = hosted(config(
        moleapi_server::sqlite_database_url(&dir.path().join("specs.db")).unwrap(),
        true,
    ))
    .await
    .unwrap();
    let owner = register(&router, "specowner").await;
    let editor = register(&router, "speceditor").await;
    let viewer = register(&router, "specviewer").await;
    let other = register(&router, "specother").await;
    let mut data = example_data();
    data["global_variables"] =
        json!([{"id":"keep","key":"keep","value":"original","enabled":true}]);
    let (_, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Specs","data":data})),
    )
    .await;
    let mut base = String::new();
    for (token, role) in [(&editor, "editor"), (&viewer, "viewer")] {
        let (_, invite) = call(
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
            Some(json!({"token":invite["token"]})),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        base = format!(
            "/api/owners/{}/workspaces/w",
            accepted["owner_id"].as_str().unwrap()
        );
    }
    let source = "openapi: 3.0.3\ninfo:\n  title: Source private-spec-marker\n  version: 1.0.0\nservers:\n  - url: https://example.test\npaths:\n  /items:\n    get:\n      operationId: listItems\n      responses:\n        '200':\n          description: ok\n          content:\n            application/json:\n              schema:\n                type: object\n                properties:\n                  id:\n                    type: integer\n";
    let endpoint = format!("{base}/specifications/preview");
    let input = json!({"expected_revision":saved["revision"],"name":"Imported","source":source,"include_requests":true});
    assert_eq!(
        call(
            &router,
            "POST",
            &endpoint,
            Some(&viewer),
            Some(input.clone())
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &router,
            "POST",
            &endpoint,
            Some(&other),
            Some(input.clone())
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, preview) = call(
        &router,
        "POST",
        &endpoint,
        Some(&editor),
        Some(input.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(
        preview["data"]["collections"][0],
        saved["data"]["collections"][0]
    );
    assert_eq!(
        preview["data"]["global_variables"],
        saved["data"]["global_variables"]
    );
    assert_eq!(
        preview["data"]["active_environment_id"],
        saved["data"]["active_environment_id"]
    );
    assert_eq!(preview["data"]["specifications"][0]["source"], source);
    assert_eq!(preview["data"]["specifications"][0]["name"], "Imported");
    assert_eq!(
        preview["data"]["collections"][1]["requests"][0]["specification_id"],
        preview["data"]["specifications"][0]["id"]
    );
    let (_, unchanged) = call(&router, "GET", "/api/workspaces/w", Some(&owner), None).await;
    assert_eq!(unchanged["data"], saved["data"]);
    let (status, updated) = call(
        &router,
        "PUT",
        &base,
        Some(&editor),
        Some(json!({"name":"Specs","expected_revision":saved["revision"],"data":preview["data"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(
        call(
            &router,
            "POST",
            &endpoint,
            Some(&editor),
            Some(input.clone())
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let replacement = json!({"expected_revision":updated["revision"],"specification_id":updated["data"]["specifications"][0]["id"],"name":"Renamed","source":source.replace("listItems","updatedItems"),"include_requests":true});
    let (status, replaced) =
        call(&router, "POST", &endpoint, Some(&editor), Some(replacement)).await;
    assert_eq!(status, StatusCode::OK, "{replaced}");
    assert_eq!(
        replaced["data"]["collections"],
        updated["data"]["collections"]
    );
    assert_eq!(
        replaced["data"]["environments"],
        updated["data"]["environments"]
    );
    assert_eq!(
        replaced["data"]["specifications"][0]["id"],
        updated["data"]["specifications"][0]["id"]
    );
    let mut invalid = input.clone();
    invalid["expected_revision"] = updated["revision"].clone();
    invalid["source"] = "invalid".into();
    assert_eq!(
        call(&router, "POST", &endpoint, Some(&editor), Some(invalid))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );

    let mut spec_only = input.clone();
    spec_only["expected_revision"] = updated["revision"].clone();
    spec_only["include_requests"] = false.into();
    let (status, only) = call(
        &router,
        "POST",
        &endpoint,
        Some(&editor),
        Some(spec_only.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{only}");
    assert_eq!(only["data"]["collections"], updated["data"]["collections"]);
    assert_eq!(
        only["data"]["environments"],
        updated["data"]["environments"]
    );
    for (key, value) in [
        ("name", json!("汉".repeat(86))),
        ("source", json!("x".repeat(5 * 1024 * 1024 + 1))),
    ] {
        let mut oversized = spec_only.clone();
        oversized[key] = value;
        assert_eq!(
            call(&router, "POST", &endpoint, Some(&editor), Some(oversized))
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    let mut dangling = updated["data"].clone();
    dangling["specifications"] = json!([]);
    assert_eq!(
        call(
            &router,
            "PUT",
            &base,
            Some(&editor),
            Some(json!({"name":"Specs","expected_revision":updated["revision"],"data":dangling}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let mut linked = updated["data"].clone();
    let specification_id = linked["specifications"][0]["id"].clone();
    linked["collections"][1]["requests"][0]["examples"] = json!([{"id":"linked","name":"Keep linked schema","status":200,"headers":[],"body":"static","mock_schema_link":{"specification_id":specification_id,"pointer":"#/paths/~1items/get/responses/200/content/application~1json/schema"}}]);
    let (status, bound) = call(
        &router,
        "PUT",
        &base,
        Some(&editor),
        Some(json!({"name":"Specs","expected_revision":updated["revision"],"data":linked})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{bound}");
    let removed_source =
        json!({"openapi":"3.0.3","info":{"title":"Empty","version":"1.0.0"},"paths":{}})
            .to_string();
    assert_eq!(call(&router,"POST",&endpoint,Some(&editor),Some(json!({"expected_revision":bound["revision"],"specification_id":specification_id,"name":"Cannot remove linked response","source":removed_source}))).await.0,StatusCode::BAD_REQUEST);
    let (_, still_bound) = call(&router, "GET", &base, Some(&editor), None).await;
    assert_eq!(still_bound["data"], bound["data"]);
    let (_, audit) = call(&router, "GET", &format!("{base}/audit"), Some(&owner), None).await;
    assert!(audit.to_string().contains("specification.preview"));
    assert!(!audit.to_string().contains("private-spec-marker"));
}
