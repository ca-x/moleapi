mod common;
use common::*;
#[tokio::test]
async fn execution_history_redacts_credentials_and_runner_counts_assertions() {
    let dir = tempfile::tempdir().unwrap();
    let router = common::local(&dir.path().join("native.db")).await.unwrap();
    let (url, server) = serve(Router::new().route(
        "/",
        axum::routing::get(|headers: axum::http::HeaderMap, request: axum::extract::Request| async move {
            assert_eq!(headers["authorization"],"Bearer manual-header-token");
            let query: std::collections::BTreeMap<_,_> = url::form_urlencoded::parse(request.uri().query().unwrap().as_bytes())
                .map(|(key,value)|(key.into_owned(),value.into_owned())).collect();
            ([("set-cookie", "private=1")], json!({"ok":true,"header_token":"manual-header-token","marked":query["marked"],"api_key":query["api_key"]}).to_string())
        }),
    ))
    .await;
    let mut workspace = example_data();
    let request = &mut workspace["collections"][0]["requests"][0];
    request["url"] = json!(format!("{url}/?api_key=private&lookup={{{{secret}}}}"));
    request["auth"]["kind"] = json!("none");
    request["headers"] = json!([{"id":"auth","key":"Authorization","value":"Bearer manual-header-token","enabled":true}]);
    request["query"] = json!([{"id":"marked","key":"marked","value":"marked-query-secret","enabled":true,"secret":true}]);
    request["auth"]["token"] = json!("auth-credential");
    request["assertions"] =
        json!([{"id":"a","name":"status","kind":"status","target":"","expected":"200"}]);
    workspace["environments"] = json!([{"id":"env","name":"Env","variables":[{"id":"v","key":"secret","value":"secret-value","enabled":true,"secret":true}]}]);
    workspace["active_environment_id"] = json!("env");
    let request = workspace["collections"][0]["requests"][0].clone();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            None,
            Some(json!({"id":"history","name":"History","data":workspace}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, response) = call(
        &router,
        "POST",
        "/api/execute",
        None,
        Some(json!({"workspace_id":"history","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["tests"][0]["passed"], true);
    let (_, history) = call(
        &router,
        "GET",
        "/api/workspaces/history/history",
        None,
        None,
    )
    .await;
    let text = history.to_string();
    assert!(!text.contains("auth-credential"));
    assert!(!text.contains("manual-header-token"));
    assert!(!text.contains("marked-query-secret"));
    assert!(!text.contains("\"api_key\":\"private\""));
    assert!(!text.contains("secret-value"));
    assert!(!text.contains("private=1"));
    assert!(!text.contains("api_key=private"));
    assert_eq!(history.as_array().unwrap().len(), 1);
    let (_, run) = call(
        &router,
        "POST",
        "/api/workspaces/history/run",
        None,
        Some(json!({"collection_id":"c"})),
    )
    .await;
    assert_eq!(run["passed"], 1);
    assert_eq!(run["failed"], 0);
    assert_eq!(
        call(
            &router,
            "DELETE",
            "/api/workspaces/history/history",
            None,
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &router,
            "GET",
            "/api/workspaces/history/history",
            None,
            None
        )
        .await
        .1,
        json!([])
    );
    server.abort();
}

#[tokio::test]
async fn nested_inherited_auth_variables_and_scripts_execute_without_child_credential_copies() {
    let dir = tempfile::tempdir().unwrap();
    let router = common::local(&dir.path().join("inheritance.db"))
        .await
        .unwrap();
    let (url,server)=serve(Router::new().route("/",axum::routing::get(|headers:axum::http::HeaderMap|async move {
        json!({"authorization":headers.get("authorization").map(|v|v.to_str().unwrap()),"sequence":headers.get("x-sequence").map(|v|v.to_str().unwrap())}).to_string()
    }))).await;
    let mut data = example_data();
    let mut request = data["collections"][0]["requests"][0].clone();
    request["url"] = json!(url);
    request["auth"] = json!({"kind":"inherit","token":"","username":"","password":""});
    request["pre_request_script"] = json!(
        "pm.variables.set('sequence',pm.variables.get('sequence')+'R');pm.request.headers.upsert({key:'X-Sequence',value:pm.variables.get('sequence')});"
    );
    request["post_response_script"] = json!(
        "pm.test('inherited sequence',()=>pm.expect(pm.variables.get('sequence')).to.equal('WCFR'));pm.variables.set('post','R');"
    );
    data["pre_request_script"] = json!("pm.variables.set('sequence','W');");
    data["post_response_script"] =
        json!("pm.test('post sequence',()=>pm.expect(pm.variables.get('post')).to.equal('RFC'));");
    let root = &mut data["collections"][0];
    root["requests"] = json!([]);
    root["auth"] = json!({"kind":"bearer","token":"{{credential}}","username":"","password":""});
    root["variables"] =
        json!([{"id":"root-value","key":"credential","value":"root-secret","enabled":true}]);
    root["pre_request_script"] =
        json!("pm.variables.set('sequence',pm.variables.get('sequence')+'C');");
    root["post_response_script"] = json!("pm.variables.set('post',pm.variables.get('post')+'C');");
    data["collections"].as_array_mut().unwrap().push(json!({"id":"folder","parent_id":"c","name":"Folder","description":"","auth":null,"variables":[{"id":"leaf-value","key":"credential","value":"leaf-secret","enabled":true}],"pre_request_script":"pm.variables.set('sequence',pm.variables.get('sequence')+'F');","post_response_script":"pm.variables.set('post',pm.variables.get('post')+'F');","requests":[request]}));
    let (status, w) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"inheritance","name":"Inheritance","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    let (status, response) = call(
        &router,
        "POST",
        "/api/execute",
        None,
        Some(json!({"workspace_id":"inheritance","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    let body: serde_json::Value = serde_json::from_str(response["body"].as_str().unwrap()).unwrap();
    assert_eq!(body["authorization"], "Bearer leaf-secret");
    assert_eq!(body["sequence"], "WCFR");
    assert!(
        response["tests"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["passed"] == true),
        "{response}"
    );
    let (_, history) = call(
        &router,
        "GET",
        "/api/workspaces/inheritance/history",
        None,
        None,
    )
    .await;
    assert!(!history.to_string().contains("leaf-secret"));
    let (_, saved) = call(&router, "GET", "/api/workspaces/inheritance", None, None).await;
    assert_eq!(
        saved["data"]["collections"][1]["requests"][0]["auth"]["kind"],
        "inherit"
    );
    assert_eq!(
        saved["data"]["collections"][1]["requests"][0]["auth"]["token"],
        ""
    );
    server.abort();
}

#[tokio::test]
async fn parent_runner_visits_descendants_and_isolates_collection_scope_changes() {
    let dir = tempfile::tempdir().unwrap();
    let router = common::local(&dir.path().join("runner-tree.db"))
        .await
        .unwrap();
    let (url, server) = serve(Router::new().route(
        "/",
        axum::routing::get(|headers: axum::http::HeaderMap| async move {
            headers["authorization"].to_str().unwrap().to_owned()
        }),
    ))
    .await;
    let mut data = example_data();
    let mut r = data["collections"][0]["requests"][0].clone();
    r["url"] = json!(url);
    r["auth"] = json!({"kind":"inherit","token":"","username":"","password":""});
    r["assertions"] = json!([]);
    let mut root_request = r.clone();
    root_request["id"] = json!("root-request");
    root_request["pre_request_script"] =
        json!("pm.collectionVariables.set('credential','updated-root');");
    data["collections"][0]["requests"] = json!([root_request]);
    data["collections"][0]["auth"] =
        json!({"kind":"bearer","token":"{{credential}}","username":"","password":""});
    data["collections"][0]["variables"] =
        json!([{"id":"root-v","key":"credential","value":"original-root","enabled":true}]);
    let mut first = r.clone();
    first["id"] = json!("first");
    first["pre_request_script"] =
        json!("pm.collectionVariables.set('credential','first-folder-update');");
    let mut second = r.clone();
    second["id"] = json!("second");
    data["collections"].as_array_mut().unwrap().extend([
      json!({"id":"folder-one","parent_id":"c","name":"One","description":"","variables":[],"requests":[first]}),
      json!({"id":"folder-two","parent_id":"c","name":"Two","description":"","variables":[],"requests":[second]}),
    ]);
    let (status, w) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"tree","name":"Tree","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    let (status, result) = call(
        &router,
        "POST",
        "/api/workspaces/tree/run",
        None,
        Some(json!({"collection_id":"c"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["passed"], 3);
    assert_eq!(result["failed"], 0);
    let rows = result["results"].as_array().unwrap();
    assert_eq!(rows[0]["request_id"], "root-request");
    assert_eq!(rows[1]["request_id"], "first");
    assert_eq!(rows[2]["request_id"], "second");
    assert_eq!(rows[0]["response"]["body"], "Bearer updated-root");
    assert_eq!(rows[1]["response"]["body"], "Bearer first-folder-update");
    assert_eq!(rows[2]["response"]["body"], "Bearer updated-root");
    let (_, history) = call(&router, "GET", "/api/workspaces/tree/history", None, None).await;
    assert!(!history.to_string().contains("updated-root"));
    assert!(!history.to_string().contains("first-folder-update"));
    server.abort();
}
