mod common;
use common::*;
#[tokio::test]
async fn extraction_updates_selected_environment_before_scripts_and_next_request_with_private_history()
 {
    let (url, fixture) = serve(
        Router::new()
            .route(
                "/token",
                axum::routing::get(|| async {
                    axum::Json(json!({"token":"extracted-private-value"}))
                }),
            )
            .route(
                "/echo",
                axum::routing::get(|headers: axum::http::HeaderMap| async move {
                    axum::Json(json!({"token":headers.get("x-token").unwrap().to_str().unwrap()}))
                }),
            ),
    )
    .await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("extract.db")).await.unwrap();
    let mut data = example_data();
    data["environments"] = json!([{"id":"dev","name":"Development","variables":[]},{"id":"prod","name":"Production","variables":[]}]);
    data["active_environment_id"] = "dev".into();
    let first = &mut data["collections"][0]["requests"][0];
    first["url"] = format!("{url}/token").into();
    first["extractions"] = json!([{"id":"token","name":"Token","kind":"jsonpath","target":"$.token","scope":"environment","key":"token","required":true},{"id":"optional","name":"Optional","kind":"json","target":"/missing","scope":"temporary","key":"optional","required":false}]);
    first["post_response_script"]="pm.test('available before post',()=>pm.expect(pm.environment.get('token')).to.equal(pm.response.json().token));".into();
    let mut second = first.clone();
    second["id"] = "second".into();
    second["url"] = format!("{url}/echo").into();
    second["headers"] = json!([{"id":"h","key":"X-Token","value":"{{token}}","enabled":true}]);
    second["extractions"] = json!([]);
    second["post_response_script"] = "".into();
    data["collections"][0]["requests"]
        .as_array_mut()
        .unwrap()
        .push(second);
    let (status, w) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"Extraction","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    let (status, result) = call(
        &router,
        "POST",
        "/api/workspaces/w/run",
        None,
        Some(json!({"collection_id":"c","environment_id":"dev"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["passed"], 2);
    assert_eq!(result["failed"], 0);
    assert!(
        result["results"][1]["response"]["body"]
            .as_str()
            .unwrap()
            .contains("extracted-private-value")
    );
    assert!(
        result["results"][0]["response"]["variable_updates"]
            .as_array()
            .unwrap()
            .iter()
            .any(|update| update["scope"] == "environment" && update["key"] == "token")
    );
    let (_, history) = call(&router, "GET", "/api/workspaces/w/history", None, None).await;
    assert!(!history.to_string().contains("extracted-private-value"));
    let (_, saved) = call(&router, "GET", "/api/workspaces/w", None, None).await;
    assert_eq!(saved["data"], w["data"]);
    fixture.abort();
}
#[tokio::test]
async fn required_extraction_without_an_environment_fails_but_optional_missing_values_do_not() {
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::get(|| async { axum::Json(json!({"value":"safe"})) }),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("missing-env.db")).await.unwrap();
    let mut data = example_data();
    data["collections"][0]["requests"][0]["url"] = format!("{url}/").into();
    data["collections"][0]["requests"][0]["extractions"] = json!([{"id":"env","name":"Environment required","kind":"json","target":"/value","scope":"environment","key":"value","required":true},{"id":"optional","name":"Missing optional","kind":"json","target":"/missing","scope":"temporary","key":"optional","required":false}]);
    let (_, w) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"Missing environment","data":data})),
    )
    .await;
    let (status, result) = call(
        &router,
        "POST",
        "/api/execute",
        None,
        Some(json!({"workspace_id":"w","request":w["data"]["collections"][0]["requests"][0]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["tests"].as_array().unwrap().len(), 1);
    assert_eq!(result["tests"][0]["passed"], false);
    assert!(result["variable_updates"].as_array().unwrap().is_empty());
    fixture.abort();
}
