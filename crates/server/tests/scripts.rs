mod common;
use common::*;
fn pair(key: &str, value: &str) -> Value {
    json!({"id":key,"key":key,"value":value,"enabled":true})
}
async fn create(router: &Router, token: Option<&str>, data: Value) {
    let (status, result) = call(
        router,
        "POST",
        "/api/workspaces",
        token,
        Some(json!({"id":"script","name":"Script","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
}
#[tokio::test]
async fn scoped_scripts_wrap_real_network_and_history_scrubs_private_values() {
    let dir = tempfile::tempdir().unwrap();
    let local = common::local(&dir.path().join("scripts.db")).await.unwrap();
    let (url, server) = serve(Router::new().route(
        "/",
        axum::routing::post(|headers: axum::http::HeaderMap, body: String| async move {
            assert_eq!(headers["x-scope"], "temporary");
            assert_eq!(headers["authorization"], "Bearer native-only-token");
            assert_eq!(body, "payload");
            json!({"token":"native-only-token","next":"chained"}).to_string()
        }),
    ))
    .await;
    let mut data = example_data();
    data["global_variables"] = json!([pair("level", "project")]);
    data["pre_request_script"] = json!("pm.variables.set('order','project');");
    data["post_response_script"] = json!(
        "pm.test('project post order',()=>pm.expect(pm.variables.get('postorder')).to.equal('request,collection'));"
    );
    let collection = &mut data["collections"][0];
    collection["variables"] = json!([pair("level", "collection")]);
    collection["pre_request_script"] =
        json!("pm.variables.set('order',pm.variables.get('order')+',collection');");
    collection["post_response_script"] =
        json!("pm.variables.set('postorder',pm.variables.get('postorder')+',collection');");
    let request = &mut collection["requests"][0];
    request["url"] = json!(format!("{url}/"));
    request["pre_request_script"] = json!(
        "pm.test('precedence',()=>pm.expect(pm.variables.get('level')).to.equal('temporary'));pm.test('pre order',()=>pm.expect(pm.variables.get('order')).to.equal('project,collection'));pm.request.method='POST';pm.request.headers.upsert({key:'x-scope',value:pm.variables.get('level')});pm.request.headers.upsert({key:'authorization',value:'Bearer '+pm.environment.get('token')});pm.request.body.update('payload');console.log(pm.environment.get('token')); "
    );
    request["post_response_script"] = json!(
        "pm.test('json flow',()=>{pm.response.to.have.status(200);pm.expect(pm.response.json().next).to.equal('chained');});pm.environment.set('next',pm.response.json().next);pm.variables.set('postorder','request');console.log(pm.response.json().token);"
    );
    let request = request.clone();
    let mut token = pair("token", "shared-token");
    token["local_value"] = json!("native-only-token");
    data["environments"] =
        json!([{"id":"e","name":"Env","variables":[pair("level","environment"),token]}]);
    data["active_environment_id"] = json!("e");
    create(&local, None, data.clone()).await;
    let (status,response) = call(&local,"POST","/api/execute",None,Some(json!({"workspace_id":"script","request":request,"data":[pair("level","execution-data")],"variables":[pair("level","temporary")]}))).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert!(
        response["tests"]
            .as_array()
            .unwrap()
            .iter()
            .all(|test| test["passed"] == true),
        "{response}"
    );
    assert_eq!(response["logs"][0]["message"], "native-only-token");
    assert!(
        response["request_updates"]
            .as_array()
            .unwrap()
            .iter()
            .any(|update| update["field"] == "method" && update["value"] == "POST")
    );
    assert!(
        response["variable_updates"]
            .as_array()
            .unwrap()
            .iter()
            .any(|update| update["scope"] == "environment" && update["key"] == "next")
    );
    let (_, history) = call(&local, "GET", "/api/workspaces/script/history", None, None).await;
    assert!(
        !history.to_string().contains("native-only-token"),
        "{history}"
    );
    assert_eq!(history[0]["response"]["logs"], json!([]));
    assert_eq!(history[0]["response"]["request_updates"], json!([]));
    assert_eq!(history[0]["response"]["variable_updates"], json!([]));
    let (_, saved) = call(&local, "GET", "/api/workspaces/script", None, None).await;
    let canonical: moleapi_core::WorkspaceData = serde_json::from_value(data).unwrap();
    assert_eq!(saved["data"], serde_json::to_value(canonical).unwrap());
    server.abort();
}
#[tokio::test]
async fn runner_chains_variables_and_counts_script_failures_without_saving() {
    let dir = tempfile::tempdir().unwrap();
    let local = common::local(&dir.path().join("runner.db")).await.unwrap();
    let (url, server) = serve(Router::new().route(
        "/",
        axum::routing::get(|| async { "{\"next\":\"next-value\"}" }),
    ))
    .await;
    let mut data = example_data();
    let first = &mut data["collections"][0]["requests"][0];
    first["url"] = json!(format!("{url}/"));
    first["post_response_script"] = json!("pm.environment.set('next',pm.response.json().next);");
    let mut second = first.clone();
    second["id"] = json!("second");
    second["pre_request_script"] = json!(
        "pm.test('chained',()=>pm.expect(pm.environment.get('next')).to.equal('next-value'));"
    );
    second["post_response_script"] = json!("throw new Error('post failed');");
    let mut third = first.clone();
    third["id"] = json!("third");
    third["pre_request_script"] = json!("while(true) {};");
    data["collections"][0]["requests"]
        .as_array_mut()
        .unwrap()
        .extend([second, third]);
    create(&local, None, data.clone()).await;
    let (status, run) = call(
        &local,
        "POST",
        "/api/workspaces/script/run",
        None,
        Some(json!({"collection_id":"c"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{run}");
    assert_eq!(run["passed"], 1);
    assert_eq!(run["failed"], 2);
    assert_eq!(run["results"][1]["response"]["tests"][0]["passed"], true);
    assert!(
        run["results"][2]["error"]
            .as_str()
            .unwrap()
            .contains("JavaScript")
    );
    let canonical: moleapi_core::WorkspaceData = serde_json::from_value(data).unwrap();
    assert_eq!(
        call(&local, "GET", "/api/workspaces/script", None, None)
            .await
            .1["data"],
        serde_json::to_value(canonical).unwrap()
    );
    server.abort();
}
#[tokio::test]
async fn hosted_scrubs_saved_local_values_and_enforces_account_and_network_policy() {
    let dir = tempfile::tempdir().unwrap();
    let hosted = common::hosted(config(
        format!(
            "sqlite://{}?mode=rwc",
            dir.path().join("hosted.db").display()
        ),
        true,
    ))
    .await
    .unwrap();
    let alice = register(&hosted, "alice").await;
    let bob = register(&hosted, "bobby").await;
    let mut data = example_data();
    let mut var = pair("key", "shared");
    var["local_value"] = json!("local-secret");
    data["global_variables"] = json!([var.clone()]);
    data["collections"][0]["variables"] = json!([var.clone()]);
    data["environments"] = json!([{"id":"e","name":"Env","variables":[var]}]);
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = json!("http://127.0.0.1:1/");
    request["pre_request_script"] = json!(
        "pm.test('hosted ignores saved local value',()=>pm.expect(pm.globals.get('key')).to.equal('shared'));"
    );
    let request = request.clone();
    create(&hosted, Some(&alice), data).await;
    let (_, saved) = call(&hosted, "GET", "/api/workspaces/script", Some(&alice), None).await;
    assert!(!saved.to_string().contains("local-secret"));
    let (status, error) = call(
        &hosted,
        "POST",
        "/api/execute",
        Some(&alice),
        Some(json!({"workspace_id":"script","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error["error"].as_str().unwrap().contains("blocked"));
    assert_eq!(
        call(
            &hosted,
            "POST",
            "/api/execute",
            Some(&bob),
            Some(json!({"workspace_id":"script","request":request}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let mut changed = saved["data"].clone();
    changed["global_variables"][0]["local_value"] = json!("another-local-secret");
    let (_, updated) = call(
        &hosted,
        "PUT",
        "/api/workspaces/script",
        Some(&alice),
        Some(json!({"name":"Script","data":changed,"expected_revision":1})),
    )
    .await;
    assert!(!updated.to_string().contains("another-local-secret"));
}
#[tokio::test]
async fn hosted_scoped_locals_reach_pm_and_network_without_persisting() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(
        format!(
            "sqlite://{}?mode=rwc",
            dir.path().join("locals.db").display()
        ),
        true,
    );
    cfg.allow_private_network = true;
    let hosted = common::hosted(cfg).await.unwrap();
    let token = register(&hosted, "localsuser").await;
    let (url, server) = serve(Router::new().route(
        "/",
        axum::routing::get(
            |headers: axum::http::HeaderMap, request: axum::extract::Request| async move {
                assert_eq!(headers["x-local"], "browser-private-token");
                assert_eq!(headers["x-project"], "browser-private-project");
                assert_eq!(headers["x-collection"], "browser-private-module");
                assert_eq!(request.uri().query(), Some("lookup=browser-private-token"));
                json!({"token":headers["x-local"].to_str().unwrap()}).to_string()
            },
        ),
    ))
    .await;
    let mut data = example_data();
    data["global_variables"] = json!([pair("global", "shared-project")]);
    data["collections"][0]["variables"] = json!([pair("module", "shared-module")]);
    data["environments"] =
        json!([{"id":"env","name":"Env","variables":[pair("token","shared-env")]}]);
    data["active_environment_id"] = json!("env");
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = json!(format!("{url}/"));
    request["query"] = json!([pair("lookup", "{{token}}")]);
    request["pre_request_script"] = json!(
        "pm.test('scoped locals',()=>{pm.expect(pm.environment.get('token')).not.equal('shared-env');pm.expect(pm.globals.get('global')).not.equal('shared-project');pm.expect(pm.collectionVariables.get('module')).not.equal('shared-module');});pm.request.headers.upsert({key:'x-local',value:pm.environment.get('token')});pm.request.headers.upsert({key:'x-project',value:pm.globals.get('global')});pm.request.headers.upsert({key:'x-collection',value:pm.collectionVariables.get('module')});console.log(pm.environment.get('token'));"
    );
    let request = request.clone();
    create(&hosted, Some(&token), data).await;
    let locals = json!([{"scope":"environment","key":"token","value":"browser-private-token"},{"scope":"project","key":"global","value":"browser-private-project"},{"scope":"collection","key":"module","value":"browser-private-module"}]);
    let (status, response) = call(
        &hosted,
        "POST",
        "/api/execute",
        Some(&token),
        Some(json!({"workspace_id":"script","request":request,"locals":locals})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["tests"][0]["passed"], true);
    assert_eq!(response["logs"][0]["message"], "browser-private-token");
    let (status, run) = call(
        &hosted,
        "POST",
        "/api/workspaces/script/run",
        Some(&token),
        Some(json!({"collection_id":"c","locals":locals})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{run}");
    assert_eq!(run["passed"], 1);
    let (_, saved) = call(&hosted, "GET", "/api/workspaces/script", Some(&token), None).await;
    assert_eq!(
        saved["data"]["environments"][0]["variables"][0]["value"],
        "shared-env"
    );
    assert!(!saved.to_string().contains("browser-private"));
    let (_, history) = call(
        &hosted,
        "GET",
        "/api/workspaces/script/history",
        Some(&token),
        None,
    )
    .await;
    assert!(
        !history.to_string().contains("browser-private"),
        "{history}"
    );
    assert_eq!(call(&hosted,"POST","/api/execute",Some(&token),Some(json!({"workspace_id":"script","request":request,"locals":[{"scope":"temporary","key":"token","value":"bad"}]}))).await.0,StatusCode::BAD_REQUEST);
    server.abort();
}
#[tokio::test]
async fn history_redacts_query_encoded_local_values_after_post_script_unsets_scope() {
    let dir = tempfile::tempdir().unwrap();
    let local = common::local(&dir.path().join("encoded-private.db"))
        .await
        .unwrap();
    let (url, server) = serve(Router::new().route(
        "/",
        axum::routing::get(|request: axum::extract::Request| async move {
            request.uri().query().unwrap_or("").to_owned()
        }),
    ))
    .await;
    let mut data = example_data();
    data["global_variables"] = json!([{"id":"v","key":"credential","value":"shared","local_value":"private value/+?","enabled":true}]);
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = json!(format!("{url}/"));
    request["query"] = json!([pair("lookup", "{{credential}}")]);
    request["post_response_script"] = json!("pm.globals.unset('credential');");
    let request = request.clone();
    create(&local, None, data).await;
    let (status, response) = call(
        &local,
        "POST",
        "/api/execute",
        None,
        Some(json!({"workspace_id":"script","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["body"], "lookup=private+value%2F%2B%3F");
    assert!(
        response["variable_updates"]
            .as_array()
            .unwrap()
            .iter()
            .any(|update| update["key"] == "credential" && update.get("value").is_none())
    );
    let (_, history) = call(&local, "GET", "/api/workspaces/script/history", None, None).await;
    let text = history.to_string();
    assert!(
        !text.contains("private+value%2F%2B%3F"),
        "encoded local value leaked: {history}"
    );
    assert!(!text.contains("private value/+?"));
    assert!(
        history[0]["response"]["body"]
            .as_str()
            .unwrap()
            .contains("REDACTED")
    );
    assert!(history[0]["url"].as_str().unwrap().contains("REDACTED"));
    server.abort();
}
#[tokio::test]
async fn failed_post_script_cannot_persist_generated_or_response_derived_private_values() {
    let dir = tempfile::tempdir().unwrap();
    let local = common::local(&dir.path().join("failed-private.db"))
        .await
        .unwrap();
    let (url, server) = serve(Router::new().route(
        "/",
        axum::routing::get(|| async { "response-private-token" }),
    ))
    .await;
    let mut data = example_data();
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = json!(format!("{url}/"));
    let request = request.clone();
    create(&local, None, data).await;
    for script in [
        "pm.variables.set('issued','generated-'+Date.now()); throw new Error(pm.variables.get('issued'));",
        "pm.variables.set('issued',pm.response.text()); throw new Error(pm.variables.get('issued'));",
    ] {
        let mut request = request.clone();
        request["post_response_script"] = json!(script);
        let (status, response) = call(
            &local,
            "POST",
            "/api/execute",
            None,
            Some(json!({"workspace_id":"script","request":request})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{response}");
        assert_eq!(response["tests"][0]["passed"], false);
        assert_eq!(response["variable_updates"], json!([]));
        let error = response["tests"][0]["actual"].as_str().unwrap();
        let private_value = error.strip_prefix("JavaScript error: ").unwrap();
        let (_, history) = call(&local, "GET", "/api/workspaces/script/history", None, None).await;
        assert!(
            !history[0].to_string().contains(private_value),
            "failed phase private value leaked: {history}"
        );
    }
    server.abort();
}
#[tokio::test]
async fn server_worker_deadline_aborts_pre_network_and_withholds_uncertain_post_history() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let directory = tempfile::tempdir().unwrap();
    let local = common::local(&directory.path().join("worker-watchdog.db"))
        .await
        .unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let (url, server) = serve(Router::new().route(
        "/",
        axum::routing::get(move || {
            let seen = seen.clone();
            async move {
                seen.fetch_add(1, Ordering::SeqCst);
                "timeout-private-response"
            }
        }),
    ))
    .await;
    let mut data = example_data();
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = json!(format!("{url}/"));
    let request = request.clone();
    create(&local, None, data).await;
    let mut pre = request.clone();
    pre["pre_request_script"] = json!("(2n ** 1000000n).toString();");
    let start = std::time::Instant::now();
    let (status, error) = call(
        &local,
        "POST",
        "/api/execute",
        None,
        Some(json!({"workspace_id":"script","request":pre})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
    assert!(
        error["error"]
            .as_str()
            .unwrap()
            .contains("1 second deadline")
    );
    assert!(start.elapsed() < std::time::Duration::from_secs(2));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let mut post = request;
    post["post_response_script"] =
        json!("pm.variables.set('issued',pm.response.text());(2n ** 1000000n).toString();");
    let (status, response) = call(
        &local,
        "POST",
        "/api/execute",
        None,
        Some(json!({"workspace_id":"script","request":post})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["body"], "timeout-private-response");
    assert_eq!(response["tests"][0]["passed"], false);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let (_, history) = call(&local, "GET", "/api/workspaces/script/history", None, None).await;
    assert!(
        !history.to_string().contains("timeout-private-response"),
        "{history}"
    );
    assert_eq!(history[0]["response"]["body"], "[REDACTED: failed script]");
    assert_eq!(history[0]["response"]["headers"], json!([]));
    server.abort();
}
#[tokio::test]
async fn caught_privacy_overflow_cannot_commit_mutations_or_save_response_private_data() {
    let directory = tempfile::tempdir().unwrap();
    let local = common::local(&directory.path().join("caught-overflow.db"))
        .await
        .unwrap();
    let token = "new-private-".repeat(40);
    let returned = token.clone();
    let (url, server) = serve(Router::new().route(
        "/",
        axum::routing::get(move || {
            let returned = returned.clone();
            async move { returned }
        }),
    ))
    .await;
    let mut data = example_data();
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = json!(format!("{url}/"));
    request["post_response_script"] = json!(
        "for(let i=0;i<4;i++){pm.variables.set('x','a'.repeat(1048500)+i);pm.variables.unset('x');}try{pm.variables.set('lost',pm.response.text());}catch(error){console.log(error.message);}42;"
    );
    let request = request.clone();
    create(&local, None, data).await;
    let (status, response) = call(
        &local,
        "POST",
        "/api/execute",
        None,
        Some(json!({"workspace_id":"script","request":request})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["body"], token);
    assert_eq!(response["tests"][0]["passed"], false);
    assert!(
        response["tests"][0]["actual"]
            .as_str()
            .unwrap()
            .contains("Private variable history exceeds execution limit")
    );
    assert_eq!(response["variable_updates"], json!([]));
    let (_, history) = call(&local, "GET", "/api/workspaces/script/history", None, None).await;
    assert!(!history[0].to_string().contains(&token), "{history}");
    assert_eq!(history[0]["response"]["body"], "[REDACTED: failed script]");
    let (_, saved) = call(&local, "GET", "/api/workspaces/script", None, None).await;
    assert!(
        saved["data"]
            .get("global_variables")
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty()
    );
    server.abort();
}
