mod common;
use common::*;
#[tokio::test]
async fn explicitly_selected_templates_cannot_copy_private_values_in_default_generation() {
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("template.db")).await.unwrap();
    let source=json!({"openapi":"3.0.3","info":{"title":"Templates","version":"1"},"paths":{},"components":{"schemas":{"Secret":{"type":"string","format":"password","example":"template-private-example"}}}}).to_string();
    let mut data = data();
    data["specifications"] =
        json!([{"id":"s","name":"Templates","kind":"openapi","dialect":"3.0.3","source":source}]);
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"w","name":"Templates","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let body = json!({"workspace_id":"w","specification_id":"s","job_id":"template-private","target":"typescript-fetch","templates":{"format":"moleapi-codegen-templates-v1","files":[{"path":"models.mustache","content":"hardcoded template-private-example"}]}});
    let (status, result) = call(
        &router,
        "POST",
        "/api/generation/projects",
        None,
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(result.to_string().contains("private values"), "{result}");
    assert_eq!(
        call(&router, "GET", "/api/workspaces/w", None, None)
            .await
            .1["data"]["specifications"][0]["source"],
        source
    );
}
