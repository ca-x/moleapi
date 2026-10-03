mod common;
use common::*;
use std::time::Duration;
fn data() -> Value {
    let mut data = example_data();
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = json!("ws://127.0.0.1:18886");
    request["examples"] = json!([]);
    request["query"] = json!([{"id":"q","key":"required","value":"yes","enabled":true}]);
    request["headers"] = json!([{"id":"h","key":"X-Fixture","value":"yes","enabled":true}]);
    request["protocol"] = json!({"kind":"socketio","namespace":"/fixture","path":"/custom/socket.io/","auth_source":"{}","listeners":["echo","private"],"event":"echo","arguments_source":"[unfinished{{later}}","attachments_base64":["{{notYet}}"],"request_ack":true,"ack_timeout_ms":5000});
    request["body"] = json!("Unrelated retained {{draftBody}}");
    data
}
async fn workspace(router: &Router, token: Option<&str>, data: Value) -> Value {
    let (status, value) = call(
        router,
        "POST",
        "/api/workspaces",
        token,
        Some(json!({"id":"w","name":"Socket.IO","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    value
}
async fn create(
    router: &Router,
    token: Option<&str>,
    workspace: &Value,
    environment: Option<&str>,
) -> Value {
    let (status, value) = call(router, "POST", "/api/sessions", token, Some(json!({"workspace_id":"w","request":workspace["data"]["collections"][0]["requests"][0],"environment_id":environment}))).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    value
}
async fn state(router: &Router, token: Option<&str>, id: &str, expected: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let (status, value) =
                call(router, "GET", &format!("/api/sessions/{id}"), token, None).await;
            assert_eq!(status, StatusCode::OK, "{value}");
            if value["state"] == expected {
                return value;
            }
            assert!(
                value["state"] == "connecting" || value["state"] == "open",
                "{value}"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap()
}
async fn event(router: &Router, token: Option<&str>, id: &str, name: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let (_, batch) = call(
                router,
                "GET",
                &format!("/api/sessions/{id}/events?after=0"),
                token,
                None,
            )
            .await;
            if let Some(event) =
                batch["events"].as_array().unwrap().iter().find(|event| {
                    event["direction"] == "incoming" && event["message"]["event"] == name
                })
            {
                return event.clone();
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap()
}
#[tokio::test]
async fn saved_socketio_incomplete_drafts_roundtrip_and_explicit_post_script_execution_error() {
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("local.db")).await.unwrap();
    let mut data = data();
    data["collections"][0]["requests"][0]["protocol"]["auth_source"] = json!("{unfinished auth");
    data["collections"][0]["requests"][0]["post_response_script"] =
        json!("console.log('retained');");
    let w = workspace(&router, None, data).await;
    assert_eq!(
        w["data"]["collections"][0]["requests"][0]["protocol"]["arguments_source"],
        "[unfinished{{later}}"
    );
    let (status, result) = call(
        &router,
        "POST",
        "/api/sessions",
        None,
        Some(json!({"workspace_id":"w","request":w["data"]["collections"][0]["requests"][0]})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        result
            .to_string()
            .contains("Post-response scripts are unavailable")
    );
    let (_, saved) = call(&router, "GET", "/api/workspaces/w", None, None).await;
    assert_eq!(
        saved["data"]["collections"][0]["requests"][0]["protocol"],
        w["data"]["collections"][0]["requests"][0]["protocol"]
    );
    assert_eq!(
        saved["data"]["collections"][0]["requests"][0]["post_response_script"],
        "console.log('retained');"
    );
}
#[tokio::test]
#[ignore = "official Socket.IO 4 fixture: see protocols/tests/fixtures/socketio/README.md"]
async fn real_socketio_script_auth_body_hook_original_scopes_private_alias_feedback_and_saved_drafts()
 {
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("local.db")).await.unwrap();
    let mut data = data();
    data["global_variables"] = json!([{"id":"g","key":"message","value":"global","enabled":true}]);
    data["environments"] = json!([{"id":"selected","name":"Selected","variables":[{"id":"v","key":"message","value":"selected-environment","enabled":true},{"id":"s","key":"auth_alias","value":"fixture-token","secret":true,"enabled":true}]},{"id":"different","name":"Different","variables":[{"id":"v2","key":"message","value":"different-environment","enabled":true}]}]);
    data["active_environment_id"] = json!("different");
    data["collections"][0]["requests"][0]["pre_request_script"] = json!(
        "pm.environment.set('message','script-selected');pm.request.body.update(JSON.stringify({token:pm.variables.get('auth_alias')}));console.log(pm.variables.get('auth_alias'));"
    );
    let w = workspace(&router, None, data).await;
    let session = create(&router, None, &w, Some("selected")).await;
    let id = session["id"].as_str().unwrap();
    state(&router, None, id, "open").await;
    assert!(!session.to_string().contains("fixture-token"));
    assert_eq!(call(&router, "POST", &format!("/api/sessions/{id}/send"), None, Some(json!({"kind":"socketio_emit","event":"scope-check","arguments_source":"[\"{{message}}\"]","attachments_base64":[],"ack_id":"scope","ack_timeout_ms":1000}))).await.0, StatusCode::OK);
    let echo = event(&router, None, id, "echo").await;
    assert_eq!(
        echo["message"]["arguments"][0],
        json!({"scope_correct":true,"auth_correct":true})
    );
    assert_eq!(call(&router, "POST", &format!("/api/sessions/{id}/send"), None, Some(json!({"kind":"socketio_emit","event":"private","arguments_source":"[\"{{auth_alias}}\"]","attachments_base64":[]}))).await.0, StatusCode::OK);
    let private = event(&router, None, id, "private").await;
    assert_eq!(private["message"]["arguments"][0]["auth"], "[REDACTED]");
    let (_, batch) = call(
        &router,
        "GET",
        &format!("/api/sessions/{id}/events?after=0"),
        None,
        None,
    )
    .await;
    assert!(!batch.to_string().contains("fixture-token"));
    assert!(batch["events"].as_array().unwrap().iter().any(|e| {
        e["message"]["kind"] == "script_log"
            && e["message"]["message"]
                .as_str()
                .unwrap()
                .contains("[REDACTED]")
    }));
    let (_, saved) = call(&router, "GET", "/api/workspaces/w", None, None).await;
    assert_eq!(
        saved["data"]["collections"][0]["requests"][0]["protocol"]["auth_source"],
        "{}"
    );
    assert_eq!(
        saved["data"]["collections"][0]["requests"][0]["protocol"]["arguments_source"],
        "[unfinished{{later}}"
    );
    assert_eq!(
        saved["data"]["collections"][0]["requests"][0]["body"],
        "Unrelated retained {{draftBody}}"
    );
    assert_eq!(
        call(
            &router,
            "POST",
            &format!("/api/sessions/{id}/close"),
            None,
            None
        )
        .await
        .0,
        StatusCode::OK
    );
}
#[tokio::test]
#[ignore = "official Socket.IO 4 fixture"]
async fn real_socketio_hosted_owner_isolation_logout_and_old_token_fence() {
    let temp = tempfile::tempdir().unwrap();
    let mut cfg = config(
        format!(
            "sqlite://{}?mode=rwc",
            temp.path().join("hosted.db").display()
        ),
        true,
    );
    cfg.allow_private_network = true;
    let router = hosted(cfg).await.unwrap();
    let owner = register(&router, "socketio-owner").await;
    let intruder = register(&router, "socketio-intruder").await;
    let mut data = data();
    data["collections"][0]["requests"][0]["protocol"]["auth_source"] =
        json!("{\"token\":\"fixture-token\"}");
    let w = workspace(&router, Some(&owner), data).await;
    let session = create(&router, Some(&owner), &w, None).await;
    let id = session["id"].as_str().unwrap();
    state(&router, Some(&owner), id, "open").await;
    assert_eq!(
        call(
            &router,
            "GET",
            &format!("/api/sessions/{id}"),
            Some(&intruder),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &router,
            "POST",
            &format!("/api/sessions/{id}/send"),
            Some(&intruder),
            Some(json!({"kind":"socketio_emit","event":"echo","arguments_source":"[]"}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(call(&router, "POST", &format!("/api/sessions/{id}/send"), Some(&owner), Some(json!({"kind":"socketio_emit","event":"ack-timeout","arguments_source":"[]","ack_id":"logout","ack_timeout_ms":120000}))).await.0, StatusCode::OK);
    assert_eq!(
        call(&router, "POST", "/api/auth/logout", Some(&owner), None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &router,
            "GET",
            &format!("/api/sessions/{id}"),
            Some(&owner),
            None
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let (status, login) = call(
        &router,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({"username":"socketio-owner","password":"goodpassword123"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    state(&router, login["token"].as_str(), id, "closed").await;
}
