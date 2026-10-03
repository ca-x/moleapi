mod common;
use common::*;
fn mqtt_data() -> Value {
    let mut data = example_data();
    let r = &mut data["collections"][0]["requests"][0];
    r["url"] = json!("mqtt://127.0.0.1:18891");
    r["examples"] = json!([]);
    r["protocol"] = json!({"kind":"mqtt","version":"5","message":{"topic":"{{later}}","payload_source":"{unfinished","encoding":"json"},"saved_messages":[{"id":"one","name":"draft","message":{"topic":"","payload_source":"not-yet-base64","encoding":"base64"}}],"will":{"message":{"topic":"","payload_source":"{unfinished","encoding":"json"},"delay_interval":1}});
    data
}
async fn save(router: &Router, data: Value) -> Value {
    let (status, w) = call(
        router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"MQTT","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    w
}
#[tokio::test]
async fn incomplete_mqtt_drafts_roundtrip_execute_validate_and_no_http_fallback() {
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("mqtt.db")).await.unwrap();
    let w = save(&router, mqtt_data()).await;
    let r = w["data"]["collections"][0]["requests"][0].clone();
    assert_eq!(r["protocol"]["message"]["payload_source"], "{unfinished");
    assert_eq!(
        r["protocol"]["saved_messages"][0]["message"]["encoding"],
        "base64"
    );
    let (status, result) = call(
        &router,
        "POST",
        "/api/sessions",
        None,
        Some(json!({"workspace_id":"w","request":r})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{result}");
    assert!(
        result["error"].as_str().unwrap().contains("topic"),
        "{result}"
    );
    let (status, result) = call(
        &router,
        "POST",
        "/api/execute",
        None,
        Some(json!({"workspace_id":"w","request":w["data"]["collections"][0]["requests"][0]})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{result}");
}
#[tokio::test]
async fn mqtt_prescripts_reject_http_mutations_and_live_post_phases() {
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("mqtt.db")).await.unwrap();
    let mut data = mqtt_data();
    data["collections"][0]["requests"][0]["protocol"]["will"] = Value::Null;
    let w = save(&router, data).await;
    for script in [
        "pm.request.body.raw='HTTP mutation';",
        "pm.request.headers.add({key:'X-Test',value:'unsupported'});",
        "pm.request.method='POST';",
    ] {
        let mut r = w["data"]["collections"][0]["requests"][0].clone();
        r["pre_request_script"] = json!(script);
        let (status, result) = call(
            &router,
            "POST",
            "/api/sessions",
            None,
            Some(json!({"workspace_id":"w","request":r})),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{script}: {result}");
        assert!(
            result["error"].as_str().unwrap().contains("pre-script"),
            "{result}"
        );
    }
    let mut r = w["data"]["collections"][0]["requests"][0].clone();
    r["post_response_script"] = json!("console.log('event');");
    let (status, result) = call(
        &router,
        "POST",
        "/api/sessions",
        None,
        Some(json!({"workspace_id":"w","request":r})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(result["error"].as_str().unwrap().contains("Post-response"));
}

async fn mqtt_state(router: &Router, token: Option<&str>, id: &str, expected: &str) -> Value {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let (status, v) =
                call(router, "GET", &format!("/api/sessions/{id}"), token, None).await;
            assert_eq!(status, StatusCode::OK, "{v}");
            if v["state"] == expected {
                return v;
            }
            assert!(v["state"] == "connecting" || v["state"] == "open", "{v}");
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap()
}
#[tokio::test]
#[ignore = "requires mature MQTT broker at MOLEAPI_MQTT_FIXTURE_URL (default18891)"]
async fn actual_mqtt_server_original_scopes_private_credentials_and_workspace_lifecycle() {
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("mqtt.db")).await.unwrap();
    let mut data = mqtt_data();
    let url = std::env::var("MOLEAPI_MQTT_FIXTURE_URL")
        .unwrap_or_else(|_| "mqtt://127.0.0.1:18891".into());
    data["global_variables"] =
        json!([{"id":"topic","key":"topic","value":"wrong-global","enabled":true}]);
    data["collections"][0]["variables"] =
        json!([{"id":"topic","key":"topic","value":"server-original-topic","enabled":true}]);
    data["environments"] = json!([{"id":"original","name":"Original","variables":[{"id":"url","key":"broker","value":url,"enabled":true},{"id":"selected","key":"selected","value":"original-environment","enabled":true}]},{"id":"new","name":"New scope","variables":[{"id":"selected","key":"selected","value":"wrong-new-environment","enabled":true}]}]);
    let r = &mut data["collections"][0]["requests"][0];
    r["url"] = json!("{{broker}}");
    r["protocol"]["will"] = Value::Null;
    r["protocol"]["client_id"] = json!("{{client}}");
    r["protocol"]["subscriptions"] = json!([{"filter":"server/{{topic}}","qos":2,"enabled":true}]);
    r["pre_request_script"] = json!("pm.variables.set('scriptVar','script-scope');");
    r["auth"] =
        json!({"kind":"basic","username":"{{username}}","password":"{{password}}","token":""});
    let w = save(&router, data).await;
    let locals = json!([{"scope":"environment","key":"username","value":"fixture-user"},{"scope":"environment","key":"password","value":"fixture-password"},{"scope":"environment","key":"client","value":"server-original-client"},{"scope":"environment","key":"private","value":"execution-private"}]);
    let(status,v)=call(&router,"POST","/api/sessions",None,Some(json!({"workspace_id":"w","request":w["data"]["collections"][0]["requests"][0],"environment_id":"original","locals":locals}))).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let id = v["id"].as_str().unwrap();
    assert!(v["request_updates"].as_array().is_none_or(Vec::is_empty));
    assert!(
        v["variable_updates"]
            .as_array()
            .is_none_or(|values| values.iter().all(|v| v["scope"] != "local"))
    );
    mqtt_state(&router, None, id, "open").await;
    let(status,v)=call(&router,"POST",&format!("/api/sessions/{id}/send"),None,Some(json!({"kind":"mqtt_publish","message":{"topic":"server/{{topic}}","payload_source":"{\"selected\":\"{{selected}}\",\"script\":\"{{scriptVar}}\",\"private\":\"{{private}}\"}","encoding":"json","qos":2}}))).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let (_, v) = call(
                &router,
                "GET",
                &format!("/api/sessions/{id}/events?after=0"),
                None,
                None,
            )
            .await;
            if let Some(e) =
                v["events"].as_array().unwrap().iter().find(|e| {
                    e["direction"] == "incoming" && e["message"]["kind"] == "mqtt_message"
                })
            {
                assert_eq!(e["message"]["topic"], "server/server-original-topic");
                assert_eq!(e["message"]["payload_redacted"], true);
                assert_eq!(e["message"]["payload_base64"], "");
                assert_eq!(e["message"]["payload_text"], Value::Null);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let (_, saved) = call(&router, "GET", "/api/workspaces/w", None, None).await;
    assert_eq!(
        saved["data"]["collections"][0]["requests"][0]["auth"]["password"],
        "{{password}}"
    );
    let mut updated = saved["data"].clone();
    updated["collections"][0]["requests"] = json!([]);
    let (status, v) = call(
        &router,
        "PUT",
        "/api/workspaces/w",
        None,
        Some(json!({"name":"MQTT","data":updated,"expected_revision":saved["revision"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(
        call(&router, "GET", &format!("/api/sessions/{id}"), None, None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}
#[tokio::test]
#[ignore = "requires mature MQTT broker at MOLEAPI_MQTT_FIXTURE_URL (default18891)"]
async fn actual_mqtt_hosted_owner_isolation_logout_generation_and_old_token_fence() {
    let temp = tempfile::tempdir().unwrap();
    let mut c = config(
        format!(
            "sqlite://{}?mode=rwc",
            temp.path().join("hosted.db").display()
        ),
        true,
    );
    c.allow_private_network = true;
    let router = hosted(c).await.unwrap();
    let owner = register(&router, "mqtt_owner").await;
    let intruder = register(&router, "mqtt_intruder").await;
    let mut data = mqtt_data();
    data["collections"][0]["requests"][0]["protocol"]["will"] = Value::Null;
    data["collections"][0]["requests"][0]["url"] = json!(
        std::env::var("MOLEAPI_MQTT_FIXTURE_URL")
            .unwrap_or_else(|_| "mqtt://127.0.0.1:18891".into())
    );
    let (status, w) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"MQTT","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    let (status, v) = call(
        &router,
        "POST",
        "/api/sessions",
        Some(&owner),
        Some(json!({"workspace_id":"w","request":w["data"]["collections"][0]["requests"][0]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let id = v["id"].as_str().unwrap();
    mqtt_state(&router, Some(&owner), id, "open").await;
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
            Some(json!({"kind":"mqtt_abort"}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
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
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/sessions",
            Some(&owner),
            Some(json!({"workspace_id":"w","request":w["data"]["collections"][0]["requests"][0]}))
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
}

async fn correlation_messages(
    router: &Router,
    id: &str,
    after: u64,
    topic: &str,
    count: usize,
) -> Vec<Value> {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let (status, batch) = call(
                router,
                "GET",
                &format!("/api/sessions/{id}/events?after={after}"),
                None,
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            let messages = batch["events"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|e| {
                    e["message"]["kind"] == "mqtt_message" && e["message"]["topic"] == topic
                })
                .cloned()
                .collect::<Vec<_>>();
            if messages.len() >= count {
                return messages;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap()
}
#[tokio::test]
#[ignore = "requires mature MQTT broker at MOLEAPI_MQTT_FIXTURE_URL (default18891)"]
async fn actual_server_redactor_withholds_decoded_correlation_secrets_and_preserves_public_bytes() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("correlation.db")).await.unwrap();
    let mut data = mqtt_data();
    let r = &mut data["collections"][0]["requests"][0];
    r["url"] = json!(
        std::env::var("MOLEAPI_MQTT_FIXTURE_URL")
            .unwrap_or_else(|_| "mqtt://127.0.0.1:18891".into())
    );
    r["protocol"]["will"] = Value::Null;
    r["protocol"]["subscriptions"] = json!([{"filter":"privacy/#","qos":2}]);
    data["environments"] = json!([{"id":"private","name":"Private","variables":[{"id":"secret","key":"secret","value":"private-secret","enabled":true,"secret":true}]}]);
    let w = save(&router, data).await;
    let mut request = w["data"]["collections"][0]["requests"][0].clone();
    let (status, v) = call(
        &router,
        "POST",
        "/api/sessions",
        None,
        Some(json!({"workspace_id":"w","request":request,"environment_id":"private"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let id = v["id"].as_str().unwrap();
    mqtt_state(&router, None, id, "open").await;
    // Register an explicit arbitrary-byte payload secret for subsequent binary-property matching.
    let binary_secret = [0, 255, 42, 128];
    let(status,v)=call(&router,"POST",&format!("/api/sessions/{id}/send"),None,Some(json!({"kind":"mqtt_publish","message":{"topic":"privacy/register","encoding":"base64","payload_source":STANDARD.encode(binary_secret),"payload_secret":true,"qos":1}}))).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let registered = correlation_messages(&router, id, 0, "privacy/register", 2).await;
    assert!(
        registered
            .iter()
            .all(|e| e["message"]["payload_redacted"] == true)
    );
    for (topic, bytes, redacted) in [
        ("privacy/known", b"\0private-secret\0".to_vec(), true),
        (
            "privacy/unaligned",
            [vec![255, 1], b"private-secret".to_vec(), vec![128, 0]].concat(),
            true,
        ),
        (
            "privacy/flagged",
            [vec![1], binary_secret.to_vec(), vec![2]].concat(),
            true,
        ),
        ("privacy/public", vec![0, 255, 0, 128, 13], false),
    ] {
        let (_, before) = call(
            &router,
            "GET",
            &format!("/api/sessions/{id}/events?after=0"),
            None,
            None,
        )
        .await;
        let after = before["next_cursor"].as_u64().unwrap();
        let encoded = STANDARD.encode(&bytes);
        let(status,v)=call(&router,"POST",&format!("/api/sessions/{id}/send"),None,Some(json!({"kind":"mqtt_publish","message":{"topic":topic,"payload_source":"public payload","encoding":"text","qos":1,"properties":{"correlation_data_base64":encoded}}}))).await;
        assert_eq!(status, StatusCode::OK, "{v}");
        let messages = correlation_messages(&router, id, after, topic, 2).await;
        for e in messages {
            assert_eq!(e["message"]["properties_redacted"], redacted, "{e}");
            assert_eq!(e["message"]["payload_redacted"], false);
            if redacted {
                assert_eq!(
                    e["message"]["properties"]["correlation_data_base64"],
                    Value::Null,
                    "{e}"
                );
            } else {
                assert_eq!(
                    e["message"]["properties"]["correlation_data_base64"],
                    encoded
                );
            }
        }
    }
    // A broker Will reuses Publish properties; its binary correlation data follows the same guard.
    request["protocol"]["will"] = json!({"message":{"topic":"privacy/will","payload_source":"public Will","qos":1,"properties":{"correlation_data_base64":STANDARD.encode(b"\0private-secret\0")}},"delay_interval":0});
    let (status, sender) = call(
        &router,
        "POST",
        "/api/sessions",
        None,
        Some(json!({"workspace_id":"w","request":request,"environment_id":"private"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{sender}");
    let sender_id = sender["id"].as_str().unwrap();
    mqtt_state(&router, None, sender_id, "open").await;
    assert_eq!(
        call(
            &router,
            "POST",
            &format!("/api/sessions/{sender_id}/send"),
            None,
            Some(json!({"kind":"mqtt_abort"}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let will = correlation_messages(&router, id, 0, "privacy/will", 1).await;
    assert_eq!(will[0]["message"]["properties_redacted"], true);
    assert_eq!(
        will[0]["message"]["properties"]["correlation_data_base64"],
        Value::Null
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
