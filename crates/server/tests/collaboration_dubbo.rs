mod common;
use common::*;
use std::time::Duration;

struct Shared {
    _dir: tempfile::TempDir,
    router: Router,
    owner: String,
    editor: String,
    other: String,
    viewer: String,
    foreign: String,
    base: String,
    saved: Value,
}
async fn setup(url: &str) -> Shared {
    setup_with(url).await
}
async fn setup_with(url: &str) -> Shared {
    let dir = tempfile::tempdir().unwrap();
    let mut settings = config(
        moleapi_server::sqlite_database_url(&dir.path().join("dubbo.db")).unwrap(),
        true,
    );
    settings.allow_private_network = true;
    let router = hosted(settings).await.unwrap();
    let owner = register(&router, "dubboowner").await;
    let editor = register(&router, "dubboeditor").await;
    let other = register(&router, "dubboother").await;
    let viewer = register(&router, "dubboviewer").await;
    let foreign = register(&router, "dubboforeign").await;
    let mut data = example_data();
    data["script_mode"] = "v2".into();
    data["collections"][0]["auth"] =
        json!({"kind":"bearer","token":"folder-private","username":"","password":""});
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = url.into();
    request["examples"] = json!([]);
    request["protocol"] = json!({"kind":"dubbo","source_version":1,"source":"direct","transport":"classic","serialization":"hessian2","service":"fixture.RegistryService","method":"echo","version":"1.0.0","group":"qa","arguments":[{"id":"arg","type_name":"java.lang.String","value_source":"\"{{message}}\"","secret":false}],"attachments":[],"metadata_center_url":"nacos://127.0.0.1:1","metadata_application":"moleapi-metadata-qa"});
    request["pre_request_script"] = "pm.variables.set('message','owner-script');".into();
    request["auth"] = json!({"kind":"none","token":"","username":"","password":""});
    request["timeout_ms"] = json!(60000);
    data["global_variables"] = json!([{"id":"v","key":"message","value":"owner-value","enabled":true},{"id":"s","key":"fixture_token","value":"fixture-token","enabled":true,"secret":true}]);
    data["global_variables"].as_array_mut().unwrap().push(
        json!({"id":"n","key":"numeric_private","value":"987654","enabled":true,"secret":true}),
    );
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Dubbo","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let mut collision = data;
    collision["global_variables"][0]["value"] = "wrong-member".into();
    collision["global_variables"][1]["value"] = "wrong-token".into();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            Some(&editor),
            Some(json!({"id":"w","name":"Collision","data":collision}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let mut base = String::new();
    for (token, role) in [(&editor, "editor"), (&other, "editor"), (&viewer, "viewer")] {
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
        assert_eq!(status, StatusCode::OK, "{accepted}");
        base = format!(
            "/api/owners/{}/workspaces/w",
            accepted["owner_id"].as_str().unwrap()
        );
    }
    Shared {
        _dir: dir,
        router,
        owner,
        editor,
        other,
        viewer,
        foreign,
        base,
        saved,
    }
}
#[tokio::test]
async fn shared_dubbo_draft_uses_member_finite_admission() {
    let shared = setup("dubbo://127.0.0.1:1").await;
    let (status,value)=call(&shared.router,"POST",&format!("{}/execute-draft",shared.base),Some(&shared.editor),Some(json!({"request":shared.saved["data"]["collections"][0]["requests"][0],"collection_id":"c","expected_workspace_revision":shared.saved["revision"]}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{value}");
    assert!(
        !value.to_string().contains("Draft execution supports"),
        "{value}"
    );
}
#[tokio::test]
async fn shared_saved_dubbo_rejects_member_scope_overrides_before_execution() {
    let shared = setup("dubbo://127.0.0.1:1").await;
    let (status, value) = call(
        &shared.router,
        "POST",
        &format!("{}/requests/r/execute", shared.base),
        Some(&shared.editor),
        Some(json!({"locals":[{"scope":"temporary","key":"message","value":"member"}]})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{value}");
    assert!(
        value.to_string().contains("canonical workspace scopes"),
        "{value}"
    );
}
#[tokio::test]
async fn shared_metadata_routes_admit_saved_requests_and_enforce_revision() {
    let shared = setup("dubbo://127.0.0.1:1").await;
    let (status, value) = call(
        &shared.router,
        "POST",
        &format!("{}/dubbo/metadata/preview", shared.base),
        Some(&shared.editor),
        Some(json!({"request_id":"r","expected_revision":0})),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{value}");
}
#[tokio::test]
async fn shared_dubbo_environment_override_and_stop_are_saved_revision_bound() {
    let shared = setup("dubbo://127.0.0.1:1").await;
    let (status, value) = call(
        &shared.router,
        "POST",
        &format!("{}/requests/r/execute", shared.base),
        Some(&shared.editor),
        Some(json!({"environment_id":"member-selection"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{value}");
    assert!(
        value.to_string().contains("canonical workspace scopes"),
        "{value}"
    );
    let (status, value) = call(
        &shared.router,
        "POST",
        &format!("{}/dubbo/cancel", shared.base),
        Some(&shared.editor),
        Some(json!({"request_id":"r","expected_revision":0,"job_id":"early"})),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{value}");
}
fn definition() -> Value {
    json!({"canonicalName":"fixture.RegistryService","vendor":{"original":true},"methods":[{"name":"echo","parameterTypes":["java.lang.String"],"returnType":"java.lang.String"},{"name":"exchange","parameterTypes":["fixture.Profile"],"returnType":"fixture.Profile"}],"types":[{"type":"fixture.Profile","properties":{"name":"java.lang.String","address":"fixture.Address","tags":"java.util.List<java.lang.String>"},"items":[],"enums":[]},{"type":"fixture.Address","properties":{"street":"java.lang.String"},"items":[],"enums":[]}]})
}
async fn metadata_http() -> (
    String,
    std::sync::Arc<std::sync::Mutex<Value>>,
    tokio::task::JoinHandle<()>,
) {
    use axum::{
        Json,
        routing::{get, post},
    };
    let raw = std::sync::Arc::new(std::sync::Mutex::new(definition()));
    let output = raw.clone();
    let app = Router::new()
        .route(
            "/nacos/v1/cs/configs",
            get(
                move |axum::extract::Query(query): axum::extract::Query<
                    std::collections::HashMap<String, String>,
                >| {
                    let output = output.clone();
                    async move {
                        assert_eq!(
                            query.get("dataId").unwrap(),
                            "fixture.RegistryService:1.0.0:qa:provider:moleapi-metadata-qa"
                        );
                        output.lock().unwrap().to_string()
                    }
                },
            ),
        )
        .route(
            "/nacos/v1/auth/users/login",
            post(|body: String| async move {
                assert!(
                    body.contains("username=reader") && body.contains("password=fixture-token")
                );
                Json(json!({"accessToken":"ephemeral-private-token"}))
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("nacos://{address}"), raw, task)
}
async fn revise(shared: &mut Shared, data: Value) {
    let(status,w)=call(&shared.router,"PUT",&shared.base,Some(&shared.editor),Some(json!({"name":shared.saved["name"],"expected_revision":shared.saved["revision"],"data":data}))).await;
    assert_eq!(status, StatusCode::OK, "{w}");
    shared.saved = w;
}
fn source(shared: &Shared) -> Value {
    json!({"request_id":"r","expected_revision":shared.saved["revision"]})
}
async fn preview(shared: &Shared) -> (StatusCode, Value) {
    call(
        &shared.router,
        "POST",
        &format!("{}/dubbo/metadata/preview", shared.base),
        Some(&shared.editor),
        Some(source(shared)),
    )
    .await
}
#[tokio::test]
async fn shared_dubbo_early_stop_is_actor_request_epoch_bound() {
    let shared = setup("dubbo://127.0.0.1:1").await;
    let mut input = source(&shared);
    input["job_id"] = json!("early");
    let path = format!("{}/dubbo/cancel", shared.base);
    for (token, status) in [
        (&shared.viewer, StatusCode::FORBIDDEN),
        (&shared.foreign, StatusCode::NOT_FOUND),
    ] {
        assert_eq!(
            call(
                &shared.router,
                "POST",
                &path,
                Some(token),
                Some(input.clone())
            )
            .await
            .0,
            status
        );
    }
    assert_eq!(
        call(
            &shared.router,
            "POST",
            &path,
            Some(&shared.other),
            Some(input.clone())
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, value) = call(
        &shared.router,
        "POST",
        &format!("{}/requests/r/execute", shared.base),
        Some(&shared.editor),
        Some(json!({"job_id":"early"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{value}");
    assert!(!value.to_string().contains("stopped locally"), "{value}");
    assert_eq!(
        call(
            &shared.router,
            "POST",
            &path,
            Some(&shared.editor),
            Some(input)
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, value) = call(
        &shared.router,
        "POST",
        &format!("{}/requests/r/execute", shared.base),
        Some(&shared.editor),
        Some(json!({"job_id":"early"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{value}");
    assert!(value.to_string().contains("stopped locally"), "{value}");
}
#[tokio::test]
async fn member_metadata_candidate_has_exact_source_nested_dto_decisions_and_no_persistence_until_cas()
 {
    let (endpoint, raw, server) = metadata_http().await;
    let mut shared = setup("dubbo://127.0.0.1:1").await;
    let mut data = shared.saved["data"].clone();
    data["global_variables"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"metadata","key":"metadata_endpoint","value":endpoint,"enabled":true}));
    data["collections"][0]["requests"][0]["protocol"]["metadata_center_url"] =
        json!("{{metadata_endpoint}}");
    let mut authored = data["collections"][0]["requests"][0].clone();
    authored["id"] = json!("authored");
    authored["name"] = json!("Do not overwrite");
    data["collections"][0]["requests"]
        .as_array_mut()
        .unwrap()
        .push(authored.clone());
    revise(&mut shared, data).await;
    let (status, value) = preview(&shared).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(value["raw_document"], raw.lock().unwrap().to_string());
    assert_eq!(value["methods"].as_array().unwrap().len(), 2);
    for (token, status) in [
        (&shared.viewer, StatusCode::FORBIDDEN),
        (&shared.foreign, StatusCode::NOT_FOUND),
    ] {
        assert_eq!(
            call(
                &shared.router,
                "POST",
                &format!("{}/dubbo/metadata/preview", shared.base),
                Some(token),
                Some(source(&shared))
            )
            .await
            .0,
            status
        );
    }
    let mut invalid = source(&shared);
    invalid["request"] = shared.saved["data"]["collections"][0]["requests"][0].clone();
    assert_eq!(
        call(
            &shared.router,
            "POST",
            &format!("{}/dubbo/metadata/preview", shared.base),
            Some(&shared.editor),
            Some(invalid)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let mut import = source(&shared);
    import["document_sha256"] = value["document_sha256"].clone();
    import["decisions"] = json!([{"signature":"echo(java.lang.String)","action":"skip"},{"signature":"exchange(fixture.Profile)","action":"add"}]);
    let (status, candidate) = call(
        &shared.router,
        "POST",
        &format!("{}/dubbo/metadata/import", shared.base),
        Some(&shared.editor),
        Some(import.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{candidate}");
    assert_eq!(candidate["revision"], shared.saved["revision"]);
    assert_eq!(
        candidate["data"]["collections"][0]["requests"][0],
        shared.saved["data"]["collections"][0]["requests"][0]
    );
    assert_eq!(candidate["data"]["collections"][0]["requests"][1], authored);
    let generated = &candidate["data"]["collections"][0]["requests"][2];
    let argument: Value = serde_json::from_str(
        generated["protocol"]["arguments"][0]["value_source"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(argument["$class"], "fixture.Profile");
    assert_eq!(argument["$"]["address"]["$class"], "fixture.Address");
    assert!(argument["$"]["address"]["$"]["street"].is_null());
    assert!(argument["$"]["tags"].as_array().unwrap().is_empty());
    assert_eq!(
        generated["protocol"]["metadata_center_url"],
        "{{metadata_endpoint}}"
    );
    let saved_source: Value = serde_json::from_str(
        candidate["data"]["specifications"][0]["source"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(saved_source["raw"], value["raw_document"]);
    assert_eq!(saved_source["url"], "{{metadata_endpoint}}");
    let (_, unchanged) = call(
        &shared.router,
        "GET",
        "/api/workspaces/w",
        Some(&shared.owner),
        None,
    )
    .await;
    assert_eq!(unchanged, shared.saved);
    let mut changed = raw.lock().unwrap().clone();
    changed["vendor"] = json!("changed after preview");
    *raw.lock().unwrap() = changed;
    assert_eq!(
        call(
            &shared.router,
            "POST",
            &format!("{}/dubbo/metadata/import", shared.base),
            Some(&shared.editor),
            Some(import)
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    revise(&mut shared, candidate["data"].clone()).await;
    assert_eq!(shared.saved["data"], candidate["data"]);
    let (status, fresh) = preview(&shared).await;
    assert_eq!(status, StatusCode::OK, "{fresh}");
    let mut replace = source(&shared);
    replace["document_sha256"] = fresh["document_sha256"].clone();
    replace["decisions"] = json!([{"signature":"echo(java.lang.String)","action":"replace","replace_request_id":"r"},{"signature":"exchange(fixture.Profile)","action":"skip"}]);
    let (status, replaced) = call(
        &shared.router,
        "POST",
        &format!("{}/dubbo/metadata/import", shared.base),
        Some(&shared.editor),
        Some(replace),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{replaced}");
    assert_eq!(replaced["data"]["collections"][0]["requests"][1], authored);
    assert_eq!(
        replaced["data"]["collections"][0]["requests"][0]["pre_request_script"],
        ""
    );
    assert_eq!(
        replaced["data"]["collections"][0]["requests"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    server.abort();
}
#[tokio::test]
async fn shared_metadata_withholds_private_raw_keys_numbers_and_transient_login_tokens() {
    let (endpoint, raw, server) = metadata_http().await;
    let mut shared = setup("dubbo://127.0.0.1:1").await;
    let mut data = shared.saved["data"].clone();
    data["collections"][0]["requests"][0]["protocol"]["metadata_center_url"] = json!(endpoint);
    revise(&mut shared, data).await;
    for private in [
        json!("fixture-token"),
        json!({"folder-private":"key"}),
        json!(987654),
    ] {
        let mut doc = definition();
        doc["vendor"] = private;
        *raw.lock().unwrap() = doc;
        let (status, value) = preview(&shared).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{value}");
        assert!(
            !value.to_string().contains("fixture-token")
                && !value.to_string().contains("folder-private")
                && !value.to_string().contains("987654")
        );
    }
    let mut data = shared.saved["data"].clone();
    data["collections"][0]["requests"][0]["protocol"]["metadata_auth"] =
        json!({"username":"reader","password":"{{fixture_token}}"});
    revise(&mut shared, data).await;
    let mut doc = definition();
    doc["vendor"] = json!("ephemeral-private-token");
    *raw.lock().unwrap() = doc;
    let (status, value) = preview(&shared).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "Transient login token must never enter a candidate: {value}"
    );
    assert!(!value.to_string().contains("ephemeral-private-token"));
    server.abort();
}
#[tokio::test]
#[ignore = "requires pinned worker, explicit Java and current official Apache/Curator MetadataFixture"]
async fn real_shared_dubbo_provider_metadata_registry_calls_and_worker_lifecycle() {
    use std::{path::Path, time::Duration};
    let path = std::env::var("MOLEAPI_DUBBO_SHARED_FIXTURE")
        .expect("fixture.json from current Apache MetadataFixture");
    let fixture: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let endpoint = fixture["METADATA_REGISTRY"].as_str().unwrap();
    let provider = format!(
        "dubbo://127.0.0.1:{}",
        fixture["METADATA_PROVIDER_PORT"].as_str().unwrap()
    );
    let mut shared = setup(&provider).await;
    let mut data = shared.saved["data"].clone();
    data["collections"][0]["requests"][0]["assertions"] = json!([{"id":"status","name":"Dubbo success","kind":"status","target":"","expected":"200"}]);
    data["collections"][0]["requests"][0]["protocol"]["metadata_center_url"] = json!(endpoint);
    revise(&mut shared, data).await;
    let (status, metadata) = preview(&shared).await;
    assert_eq!(status, StatusCode::OK, "{metadata}");
    assert_eq!(metadata["raw_document"], fixture["METADATA_JSON"]);
    assert!(
        metadata["types"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["type_name"] == "fixture.Profile")
    );
    let (status, response) = call(
        &shared.router,
        "POST",
        &format!("{}/requests/r/execute", shared.base),
        Some(&shared.editor),
        Some(json!({"job_id":"direct"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], 200);
    assert_eq!(response["tests"][0]["passed"], true, "{response}");
    assert_eq!(
        serde_json::from_str::<Value>(response["body"].as_str().unwrap()).unwrap(),
        "metadata:owner-script"
    );
    let mut draft = shared.saved["data"]["collections"][0]["requests"][0].clone();
    draft["protocol"]["arguments"][0]["value_source"] = json!("\"draft {{message}}\"");
    let(status,response)=call(&shared.router,"POST",&format!("{}/execute-draft",shared.base),Some(&shared.editor),Some(json!({"request":draft,"collection_id":"c","expected_workspace_revision":shared.saved["revision"],"job_id":"draft"}))).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(
        serde_json::from_str::<Value>(response["body"].as_str().unwrap()).unwrap(),
        "metadata:draft owner-script"
    );
    for (value, status0) in [
        ("return-null", 200),
        ("throw", 500),
        ("{{fixture_token}}", 200),
    ] {
        let mut request = shared.saved["data"]["collections"][0]["requests"][0].clone();
        request["protocol"]["arguments"][0]["value_source"] = json!(format!("\"{value}\""));
        let(status,response)=call(&shared.router,"POST",&format!("{}/execute-draft",shared.base),Some(&shared.editor),Some(json!({"request":request,"collection_id":"c","expected_workspace_revision":shared.saved["revision"],"job_id":"semantic"}))).await;
        assert_eq!(status, StatusCode::OK, "{response}");
        assert_eq!(response["status"], status0, "{response}");
        if value == "return-null" {
            assert_eq!(response["body"], "null");
        }
        if value == "throw" {
            assert!(
                response["body"]
                    .as_str()
                    .unwrap()
                    .contains("fixture remote failure")
            );
        }
    }
    let (_, history) = call(
        &shared.router,
        "GET",
        &format!("{}/history", shared.base),
        Some(&shared.editor),
        None,
    )
    .await;
    assert!(!history.to_string().contains("fixture-token"));
    let (_, canonical) = call(
        &shared.router,
        "GET",
        "/api/workspaces/w",
        Some(&shared.owner),
        None,
    )
    .await;
    assert_eq!(canonical, shared.saved);
    let mut data = shared.saved["data"].clone();
    data["collections"][0]["requests"][0]["url"] = json!(endpoint);
    data["collections"][0]["requests"][0]["protocol"]["source"] = json!("zookeeper");
    data["collections"][0]["requests"][0]["protocol"]["source_version"] = json!(2);
    revise(&mut shared, data).await;
    let (status, response) = call(
        &shared.router,
        "POST",
        &format!("{}/requests/r/execute", shared.base),
        Some(&shared.editor),
        Some(json!({"job_id":"registry"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(
        serde_json::from_str::<Value>(response["body"].as_str().unwrap()).unwrap(),
        "metadata:owner-script"
    );
    assert!(response["logs"].to_string().contains("ZooKeeper"));
    // Owned metadata import still writes immediately, unlike the shared candidate operation.
    let (status, owned) = call(
        &shared.router,
        "POST",
        "/api/workspaces",
        Some(&shared.owner),
        Some(
            json!({"id":"owned","name":"Owned metadata compatibility","data":shared.saved["data"]}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{owned}");
    let request = owned["data"]["collections"][0]["requests"][0].clone();
    let(status,imported)=call(&shared.router,"POST","/api/workspaces/owned/dubbo/metadata/import",Some(&shared.owner),Some(json!({"request":request,"collection_id":"c","expected_revision":owned["revision"],"document_sha256":metadata["document_sha256"],"decisions":[{"signature":"echo(java.lang.String)","action":"skip"},{"signature":"exchange(fixture.Profile)","action":"add"}]}))).await;
    assert_eq!(status, StatusCode::OK, "{imported}");
    assert!(imported["revision"].as_i64().unwrap() > owned["revision"].as_i64().unwrap());
    let (_, persisted) = call(
        &shared.router,
        "GET",
        "/api/workspaces/owned",
        Some(&shared.owner),
        None,
    )
    .await;
    assert_eq!(persisted, imported);
    fn worker_pids() -> Vec<u32> {
        std::fs::read_dir("/proc")
            .unwrap()
            .flatten()
            .filter_map(|entry| {
                let pid = entry.file_name().to_string_lossy().parse::<u32>().ok()?;
                let status = std::fs::read_to_string(entry.path().join("status")).ok()?;
                let parent = status.lines().find_map(|line| {
                    line.strip_prefix("PPid:")
                        .and_then(|s| s.trim().parse::<u32>().ok())
                })?;
                if parent != std::process::id() {
                    return None;
                }
                let command = std::fs::read(entry.path().join("cmdline")).ok()?;
                let args = String::from_utf8_lossy(&command);
                (args.contains("-jar") && args.contains("dubbo-worker.jar")).then_some(pid)
            })
            .collect()
    }
    for boundary in ["stop", "revision", "role", "logout"] {
        let mut shared = setup(&provider).await;
        let mut data = shared.saved["data"].clone();
        data["collections"][0]["requests"][0]["pre_request_script"] = json!("");
        data["collections"][0]["requests"][0]["protocol"]["arguments"][0]["value_source"] =
            json!("\"wait\"");
        revise(&mut shared, data).await;
        let router = shared.router.clone();
        let actor = shared.editor.clone();
        let url = format!("{}/requests/r/execute", shared.base);
        let work = tokio::spawn(async move {
            call(
                &router,
                "POST",
                &url,
                Some(&actor),
                Some(json!({"job_id":"held"})),
            )
            .await
        });
        let pid = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let Some(pid) = worker_pids().first() {
                    break *pid;
                }
                tokio::time::sleep(Duration::from_millis(15)).await;
            }
        })
        .await
        .expect("real worker spawned");
        assert!(Path::new(&format!("/proc/{pid}")).exists());
        let expected = match boundary {
            "stop" => {
                let mut cancel = source(&shared);
                cancel["job_id"] = json!("held");
                assert_eq!(
                    call(
                        &shared.router,
                        "POST",
                        &format!("{}/dubbo/cancel", shared.base),
                        Some(&shared.editor),
                        Some(cancel)
                    )
                    .await
                    .0,
                    StatusCode::OK
                );
                StatusCode::BAD_REQUEST
            }
            "revision" => {
                let data = shared.saved["data"].clone();
                revise(&mut shared, data).await;
                StatusCode::CONFLICT
            }
            "role" => {
                let (_, members) = call(
                    &shared.router,
                    "GET",
                    &format!("{}/members", shared.base),
                    Some(&shared.owner),
                    None,
                )
                .await;
                let member = members
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|m| m["username"] == "dubboeditor")
                    .unwrap();
                assert_eq!(
                    call(
                        &shared.router,
                        "PATCH",
                        &format!(
                            "{}/members/{}",
                            shared.base,
                            member["member_id"].as_str().unwrap()
                        ),
                        Some(&shared.owner),
                        Some(json!({"role":"viewer","expected_revision":member["revision"]}))
                    )
                    .await
                    .0,
                    StatusCode::OK
                );
                StatusCode::NOT_FOUND
            }
            _ => {
                assert_eq!(
                    call(
                        &shared.router,
                        "POST",
                        "/api/auth/logout",
                        Some(&shared.editor),
                        None
                    )
                    .await
                    .0,
                    StatusCode::OK
                );
                StatusCode::UNAUTHORIZED
            }
        };
        let (status, value) = tokio::time::timeout(Duration::from_secs(3), work)
            .await
            .expect("pending operation fenced")
            .unwrap();
        assert_eq!(status, expected, "{boundary}: {value}");
        tokio::time::timeout(Duration::from_secs(3), async {
            while Path::new(&format!("/proc/{pid}")).exists() {
                tokio::time::sleep(Duration::from_millis(15)).await;
            }
        })
        .await
        .expect("actual Java worker reaped");
    }
}
#[tokio::test]
async fn pending_member_metadata_reads_withhold_results_and_close_sockets_on_role_revision_logout()
{
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    for operation in ["preview", "import"] {
        for boundary in ["role", "revision", "logout"] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
            let (closed_tx, closed_rx) = tokio::sync::oneshot::channel();
            let peer = tokio::spawn(async move {
                let (mut socket, mut header) = loop {
                    let (mut socket, _) = listener.accept().await.unwrap();
                    let mut first = [0];
                    if socket.read(&mut first).await.unwrap() == 0 {
                        continue;
                    }
                    break (socket, vec![first[0]]);
                };
                let mut byte = [0];
                while !header.ends_with(b"\r\n\r\n") {
                    assert_eq!(socket.read(&mut byte).await.unwrap(), 1);
                    header.push(byte[0]);
                }
                assert!(!String::from_utf8_lossy(&header).contains("fixture-token"));
                socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 65535\r\n\r\n").await.unwrap();
                ready_tx.send(()).unwrap();
                let _ = closed_tx.send(socket.read(&mut byte).await);
            });
            let mut shared = setup("dubbo://127.0.0.1:1").await;
            let mut data = shared.saved["data"].clone();
            data["collections"][0]["requests"][0]["protocol"]["metadata_center_url"] =
                json!(format!("nacos://{address}"));
            revise(&mut shared, data).await;
            let mut input = source(&shared);
            if operation == "import" {
                input["document_sha256"] = json!("0".repeat(64));
                input["decisions"] = json!([]);
            }
            let router = shared.router.clone();
            let token = shared.editor.clone();
            let path = format!("{}/dubbo/metadata/{operation}", shared.base);
            let work = tokio::spawn(async move {
                call(&router, "POST", &path, Some(&token), Some(input)).await
            });
            tokio::time::timeout(Duration::from_secs(3), ready_rx)
                .await
                .unwrap()
                .unwrap();
            let expected = match boundary {
                "revision" => {
                    let data = shared.saved["data"].clone();
                    revise(&mut shared, data).await;
                    StatusCode::CONFLICT
                }
                "role" => {
                    let (_, members) = call(
                        &shared.router,
                        "GET",
                        &format!("{}/members", shared.base),
                        Some(&shared.owner),
                        None,
                    )
                    .await;
                    let member = members
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|m| m["username"] == "dubboeditor")
                        .unwrap();
                    assert_eq!(
                        call(
                            &shared.router,
                            "PATCH",
                            &format!(
                                "{}/members/{}",
                                shared.base,
                                member["member_id"].as_str().unwrap()
                            ),
                            Some(&shared.owner),
                            Some(json!({"role":"viewer","expected_revision":member["revision"]}))
                        )
                        .await
                        .0,
                        StatusCode::OK
                    );
                    StatusCode::NOT_FOUND
                }
                _ => {
                    assert_eq!(
                        call(
                            &shared.router,
                            "POST",
                            "/api/auth/logout",
                            Some(&shared.editor),
                            None
                        )
                        .await
                        .0,
                        StatusCode::OK
                    );
                    StatusCode::UNAUTHORIZED
                }
            };
            let (status, result) = tokio::time::timeout(Duration::from_secs(2), work)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(status, expected, "{operation}/{boundary}: {result}");
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(2), closed_rx)
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap(),
                0,
                "{operation}/{boundary}"
            );
            peer.await.unwrap();
        }
    }
}
#[tokio::test]
#[ignore = "requires isolated authenticated Nacos2.5.1 plus official Apache metadata/provider fixture and pinned worker/Java"]
async fn real_shared_dubbo_nacos_auth_namespace_metadata_and_invocation() {
    let path = std::env::var("MOLEAPI_DUBBO_NACOS_SHARED_FIXTURE")
        .expect("official authenticated Nacos fixture.json");
    let fixture: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let endpoint = fixture["METADATA_REGISTRY"].as_str().unwrap();
    let mut shared = setup("dubbo://127.0.0.1:1").await;
    let mut data = shared.saved["data"].clone();
    data["global_variables"][1]["value"] = json!("fixture-password");
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = json!(endpoint);
    let config = &mut request["protocol"];
    config["source"] = json!("nacos");
    config["source_version"] = json!(2);
    config["service"] = json!("fixture.MetadataService");
    config["method"] = json!("exchange");
    config["registry_namespace"] = fixture["METADATA_NAMESPACE"].clone();
    config["registry_auth"] = json!({"username":"fixture-reader","password":"{{fixture_token}}"});
    config["metadata_center_url"] = json!(endpoint);
    config["metadata_application"] = json!("moleapi-nacos-metadata-qa");
    config["metadata_namespace"] = fixture["METADATA_NAMESPACE"].clone();
    config["metadata_group"] = fixture["METADATA_GROUP"].clone();
    config["metadata_auth"] = json!({"username":"fixture-reader","password":"{{fixture_token}}"});
    config["arguments"] = json!([{"id":"profile","type_name":"fixture.MetadataProfile","value_source":"{\"$class\":\"fixture.MetadataProfile\",\"$\":{\"name\":\"shared nacos\"}}","secret":false}]);
    revise(&mut shared, data).await;
    let (status, value) = preview(&shared).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(value["raw_document"], fixture["METADATA_JSON"]);
    assert!(value.get("private_auth_values").is_none());
    assert!(!value.to_string().contains("fixture-password"));
    let (status, response) = call(
        &shared.router,
        "POST",
        &format!("{}/requests/r/execute", shared.base),
        Some(&shared.editor),
        Some(json!({"job_id":"nacos"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], 200);
    assert!(
        response["body"].as_str().unwrap().contains("shared nacos"),
        "{response}"
    );
    assert!(response["logs"].to_string().contains("Nacos"));
    let mut input = source(&shared);
    input["document_sha256"] = value["document_sha256"].clone();
    input["decisions"] = json!([{"signature":"exchange(fixture.MetadataProfile)","action":"add"}]);
    let (status, candidate) = call(
        &shared.router,
        "POST",
        &format!("{}/dubbo/metadata/import", shared.base),
        Some(&shared.editor),
        Some(input),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{candidate}");
    let artifact: Value = serde_json::from_str(
        candidate["data"]["specifications"][0]["source"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(artifact["raw"], fixture["METADATA_JSON"]);
    assert_eq!(artifact["namespace"], fixture["METADATA_NAMESPACE"]);
    assert_eq!(artifact["config_group"], fixture["METADATA_GROUP"]);
    assert!(!artifact.to_string().contains("fixture-password"));
    let (_, canonical) = call(
        &shared.router,
        "GET",
        "/api/workspaces/w",
        Some(&shared.owner),
        None,
    )
    .await;
    assert_eq!(canonical, shared.saved);
    let good = shared.saved["data"].clone();
    for field in ["metadata_auth", "metadata_namespace", "metadata_group"] {
        let mut bad = good.clone();
        let config = &mut bad["collections"][0]["requests"][0]["protocol"];
        if field == "metadata_auth" {
            config[field]["password"] = json!("wrong-password");
        } else {
            config[field] = json!("wrong-scope");
        }
        revise(&mut shared, bad).await;
        let (status, result) = preview(&shared).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{field}: {result}");
        assert!(!result.to_string().contains("fixture-password"));
        assert_eq!(
            call(
                &shared.router,
                "GET",
                "/api/auth/me",
                Some(&shared.editor),
                None
            )
            .await
            .0,
            StatusCode::OK
        );
    }
}
