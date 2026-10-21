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
    let dir = tempfile::tempdir().unwrap();
    let mut settings = config(
        moleapi_server::sqlite_database_url(&dir.path().join("mqtt.db")).unwrap(),
        true,
    );
    settings.allow_private_network = true;
    let router = hosted(settings).await.unwrap();
    let owner = register(&router, "mqttowner").await;
    let editor = register(&router, "mqtteditor").await;
    let other = register(&router, "mqttother").await;
    let viewer = register(&router, "mqttviewer").await;
    let foreign = register(&router, "mqttforeign").await;
    let mut data = example_data();
    data["script_mode"] = "v2".into();
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = url.into();
    request["examples"] = json!([]);
    request["protocol"] = json!({"kind":"mqtt","version":"5","client_id":"shared-member-fixture","subscriptions":[{"filter":"shared/{{topic}}/#","qos":2,"enabled":true}],"message":{"topic":"shared/{{topic}}/saved","payload_source":"saved canonical","encoding":"text"}});
    request["auth"] =
        json!({"kind":"basic","username":"{{username}}","password":"{{password}}","token":""});
    request["pre_request_script"] = "pm.variables.set('scriptValue','pre-script-owner');".into();
    request["timeout_ms"] = json!(60000);
    data["global_variables"] = json!([{"id":"v","key":"topic","value":"owner-value","enabled":true},{"id":"u","key":"username","value":"fixture-user","enabled":true},{"id":"s","key":"password","value":"fixture-password","enabled":true,"secret":true},{"id":"p","key":"private","value":"private-owner-payload","enabled":true,"secret":true}]);
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"MQTT","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let mut collision = data;
    collision["global_variables"][0]["value"] = "wrong-member".into();
    collision["global_variables"][2]["value"] = "wrong-token".into();
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
async fn create(shared: &Shared) -> (StatusCode, Value) {
    call(
        &shared.router,
        "POST",
        &format!("{}/sessions", shared.base),
        Some(&shared.editor),
        Some(json!({"request_id":"r","expected_revision":shared.saved["revision"]})),
    )
    .await
}
async fn until(shared: &Shared, path: &str, predicate: impl Fn(&Value) -> bool) -> Value {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let (status, value) =
                call(&shared.router, "GET", path, Some(&shared.editor), None).await;
            assert_eq!(status, StatusCode::OK, "{value}");
            if predicate(&value) {
                break value;
            }
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
    })
    .await
    .unwrap()
}
async fn send(shared: &Shared, path: &str, message: Value) {
    let (status, value) = call(
        &shared.router,
        "POST",
        &format!("{path}/send"),
        Some(&shared.editor),
        Some(message),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
}
async fn event(shared: &Shared, path: &str, predicate: impl Fn(&Value) -> bool) -> Value {
    let batch = until(shared, &format!("{path}/events"), |b| {
        b["events"].as_array().unwrap().iter().any(&predicate)
    })
    .await;
    batch["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| predicate(e))
        .unwrap()
        .clone()
}
#[tokio::test]
async fn member_mqtt_admission_is_saved_revision_creator_bound_and_rejects_other_commands() {
    let shared = setup("mqtt://127.0.0.1:1").await;
    let input = json!({"request_id":"r","expected_revision":shared.saved["revision"]});
    assert_eq!(
        call(
            &shared.router,
            "POST",
            &format!("{}/sessions", shared.base),
            Some(&shared.viewer),
            Some(input)
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &shared.router,
            "POST",
            &format!("{}/sessions", shared.base),
            Some(&shared.editor),
            Some(json!({"request_id":"r","expected_revision":0}))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(
            &shared.router,
            "POST",
            &format!("{}/sessions", shared.base),
            Some(&shared.foreign),
            Some(json!({"request_id":"r","expected_revision":shared.saved["revision"]}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, opened) = create(&shared).await;
    assert_eq!(status, StatusCode::OK, "{opened}");
    let path = format!(
        "{}/sessions/{}",
        shared.base,
        opened["id"].as_str().unwrap()
    );
    for token in [&shared.other, &shared.viewer, &shared.foreign] {
        assert_eq!(
            call(&shared.router, "GET", &path, Some(token), None)
                .await
                .0,
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        call(
            &shared.router,
            "POST",
            &format!("{path}/send"),
            Some(&shared.editor),
            Some(json!({"kind":"text","text":"not-mqtt"}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &shared.router,
            "POST",
            &format!("{path}/close"),
            Some(&shared.editor),
            Some(json!({}))
        )
        .await
        .0,
        StatusCode::OK
    );
}

struct Broker {
    child: std::process::Child,
    _directory: tempfile::TempDir,
    url: String,
}
impl Broker {
    async fn start() -> Self {
        let binary = std::path::PathBuf::from(
            std::env::var("MOLEAPI_MOSQUITTO_BIN").expect("isolated Mosquitto binary"),
        );
        let directory = tempfile::tempdir().unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let password = directory.path().join("password");
        assert!(
            std::process::Command::new(binary.with_file_name("mosquitto_passwd"))
                .args(["-b", "-c"])
                .arg(&password)
                .args(["fixture-user", "fixture-password"])
                .status()
                .unwrap()
                .success()
        );
        let config = directory.path().join("broker.conf");
        std::fs::write(&config, format!("listener {port} 127.0.0.1\nallow_anonymous false\npersistence false\npassword_file {}\n", password.display())).unwrap();
        let mut child = std::process::Command::new(&binary)
            .arg("-c")
            .arg(config)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                assert!(child.try_wait().unwrap().is_none(), "broker exited");
                if tokio::net::TcpStream::connect(("127.0.0.1", port))
                    .await
                    .is_ok()
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(15)).await;
            }
        })
        .await
        .unwrap();
        Self {
            child,
            _directory: directory,
            url: format!("mqtt://127.0.0.1:{port}"),
        }
    }
}
impl Drop for Broker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
async fn revise(shared: &mut Shared) {
    let (status, value) = call(&shared.router, "PUT", &shared.base, Some(&shared.editor), Some(json!({"name":shared.saved["name"],"data":shared.saved["data"],"expected_revision":shared.saved["revision"]}))).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    shared.saved = value;
}
#[tokio::test]
#[ignore = "requires isolated mature Mosquitto via MOLEAPI_MOSQUITTO_BIN"]
async fn real_member_mqtt_versions_qos_binary_retained_subscriptions_owner_privacy_and_lifecycle() {
    let broker = Broker::start().await;
    for version in ["3.1.1", "5"] {
        let mut shared = setup(&broker.url).await;
        shared.saved["data"]["collections"][0]["requests"][0]["protocol"]["version"] =
            version.into();
        shared.saved["data"]["collections"][0]["requests"][0]["protocol"]["will"] = json!({"message":{"topic":"shared/owner-value/will","payload_source":"fixture will","encoding":"text","qos":1},"delay_interval":0});
        revise(&mut shared).await;
        let mut observer = shared.saved["data"]["collections"][0]["requests"][0].clone();
        observer["pre_request_script"] = "".into();
        observer["protocol"]["client_id"] = "fixture-observer".into();
        observer["protocol"]["will"] = Value::Null;
        observer["protocol"]["subscriptions"] = json!([{"filter":"#","qos":2,"enabled":true}]);
        let (status, observed) = call(
            &shared.router,
            "POST",
            "/api/sessions",
            Some(&shared.owner),
            Some(json!({"workspace_id":"w","request":observer})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{observed}");
        let observer_path = format!("/api/sessions/{}", observed["id"].as_str().unwrap());
        owner_event(&shared, &observer_path, |e| {
            e["message"]["operation"] == "subscribe" && e["message"]["status"] == "acknowledged"
        })
        .await;
        let (status, summary) = create(&shared).await;
        assert_eq!(status, StatusCode::OK, "{summary}");
        let path = format!(
            "{}/sessions/{}",
            shared.base,
            summary["id"].as_str().unwrap()
        );
        until(&shared, &path, |s| s["state"] == "open").await;
        event(&shared, &path, |e| {
            e["message"]["operation"] == "subscribe" && e["message"]["status"] == "acknowledged"
        })
        .await;
        let (_, initial) = call(
            &shared.router,
            "GET",
            &format!("{path}/events"),
            Some(&shared.editor),
            None,
        )
        .await;
        assert!(
            initial["events"]
                .as_array()
                .unwrap()
                .iter()
                .all(|e| e["message"]["kind"] != "mqtt_message")
        );
        for qos in 0..=2 {
            let topic = format!("shared/owner-value/qos{qos}");
            send(&shared, &path, json!({"kind":"mqtt_publish","message":{"topic":format!("shared/{{{{topic}}}}/qos{qos}"),"payload_source":format!("qos{qos}:{{{{topic}}}}/{{{{scriptValue}}}}"),"encoding":"text","qos":qos}})).await;
            let incoming = event(&shared, &path, |e| {
                e["direction"] == "incoming" && e["message"]["topic"] == topic
            })
            .await;
            // Pre-script updates are privacy-screened in the sender; independently inspect actual broker delivery.
            assert_eq!(incoming["message"]["payload_redacted"], true, "{incoming}");
            let wire = owner_event(&shared, &observer_path, |e| {
                e["direction"] == "incoming" && e["message"]["topic"] == topic
            })
            .await;
            assert_eq!(
                wire["message"]["payload_text"],
                format!("qos{qos}:owner-value/pre-script-owner"),
                "{wire}"
            );
            assert_eq!(incoming["message"]["qos"], qos);
        }
        if version == "5" {
            send(&shared,&path,json!({"kind":"mqtt_publish","message":{"topic":"shared/{{topic}}/properties","payload_source":"{\"temperature\":21.5}","encoding":"json","qos":1,"properties":{"content_type":"application/json","payload_format_indicator":1,"user_properties":[{"key":"source","value":"{{topic}}","secret":false}]}}})).await;
            let properties = owner_event(&shared, &observer_path, |e| {
                e["direction"] == "incoming"
                    && e["message"]["topic"] == "shared/owner-value/properties"
            })
            .await;
            assert_eq!(
                properties["message"]["properties"]["content_type"], "application/json",
                "{properties}"
            );
            assert_eq!(
                properties["message"]["properties"]["user_properties"],
                json!([{ "key": "source", "value": "owner-value" }]),
                "{properties}"
            );
        }
        for token in [&shared.other, &shared.viewer, &shared.foreign] {
            assert_eq!(
                call(
                    &shared.router,
                    "POST",
                    &format!("{path}/send"),
                    Some(token),
                    Some(json!({"kind":"mqtt_abort"}))
                )
                .await
                .0,
                StatusCode::NOT_FOUND
            );
        }
        send(&shared,&path,json!({"kind":"mqtt_subscribe","subscription":{"filter":"temporary/#","qos":2,"enabled":true}})).await;
        send(&shared,&path,json!({"kind":"mqtt_publish","message":{"topic":"temporary/binary","payload_source":"AP8H","encoding":"base64","qos":2,"retain":true}})).await;
        let binary = event(&shared, &path, |e| {
            e["direction"] == "incoming" && e["message"]["topic"] == "temporary/binary"
        })
        .await;
        assert_eq!(binary["message"]["payload_base64"], "AP8H");
        send(
            &shared,
            &path,
            json!({"kind":"mqtt_unsubscribe","filter":"temporary/#"}),
        )
        .await;
        event(&shared, &path, |e| {
            e["message"]["operation"] == "unsubscribe" && e["message"]["status"] == "acknowledged"
        })
        .await;
        send(&shared,&path,json!({"kind":"mqtt_subscribe","subscription":{"filter":"temporary/binary","qos":2,"enabled":true}})).await;
        let retained = event(&shared, &path, |e| {
            e["direction"] == "incoming"
                && e["message"]["topic"] == "temporary/binary"
                && e["message"]["retain"] == true
        })
        .await;
        assert_eq!(retained["message"]["payload_base64"], "AP8H");
        send(&shared,&path,json!({"kind":"mqtt_publish","message":{"topic":"temporary/binary","payload_source":"","encoding":"base64","qos":2,"retain":true}})).await;
        send(&shared,&path,json!({"kind":"mqtt_publish","message":{"topic":"shared/{{topic}}/private","payload_source":"{{private}}","encoding":"text","qos":1}})).await;
        let private = event(&shared, &path, |e| {
            e["direction"] == "incoming" && e["message"]["topic"] == "shared/owner-value/private"
        })
        .await;
        assert_eq!(private["message"]["payload_redacted"], true);
        assert!(!private.to_string().contains("private-owner-payload"));
        let (_, canonical) = call(
            &shared.router,
            "GET",
            "/api/workspaces/w",
            Some(&shared.owner),
            None,
        )
        .await;
        assert_eq!(canonical["data"], shared.saved["data"]);
        assert_eq!(canonical["revision"], shared.saved["revision"]);
        send(&shared, &path, json!({"kind":"mqtt_abort"})).await;
        until(&shared, &path, |s| s["state"] == "closed").await;
        let will = owner_event(&shared, &observer_path, |e| {
            e["message"]["topic"] == "shared/owner-value/will"
        })
        .await;
        assert_eq!(will["message"]["payload_text"], "fixture will");
        let (_, audit) = call(
            &shared.router,
            "GET",
            &format!("{}/audit", shared.base),
            Some(&shared.owner),
            None,
        )
        .await;
        let (_, actor) = call(
            &shared.router,
            "GET",
            "/api/auth/me",
            Some(&shared.editor),
            None,
        )
        .await;
        assert!(
            audit
                .as_array()
                .unwrap()
                .iter()
                .filter(|e| e["action"]
                    .as_str()
                    .is_some_and(|a| a.starts_with("protocol.mqtt.")))
                .all(|e| e["actor_id"] == actor["id"])
        );
        for action in [
            "protocol.mqtt.publish",
            "protocol.mqtt.subscribe",
            "protocol.mqtt.unsubscribe",
            "protocol.mqtt.abort",
        ] {
            assert!(
                audit
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|e| e["action"] == action),
                "{audit}"
            );
        }
        for content in [
            "owner-value",
            "fixture-password",
            "private-owner-payload",
            "AP8H",
            "temporary/binary",
        ] {
            assert!(!audit.to_string().contains(content), "{audit}");
        }
        let (_, summary) = create(&shared).await;
        let graceful = format!(
            "{}/sessions/{}",
            shared.base,
            summary["id"].as_str().unwrap()
        );
        until(&shared, &graceful, |s| s["state"] == "open").await;
        let (_, before) = call(
            &shared.router,
            "GET",
            &format!("{observer_path}/events"),
            Some(&shared.owner),
            None,
        )
        .await;
        let after = before["next_cursor"].as_u64().unwrap();
        assert_eq!(
            call(
                &shared.router,
                "POST",
                &format!("{graceful}/close"),
                Some(&shared.editor),
                Some(json!({}))
            )
            .await
            .0,
            StatusCode::OK
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
        let (_, after_close) = call(
            &shared.router,
            "GET",
            &format!("{observer_path}/events?after={after}"),
            Some(&shared.owner),
            None,
        )
        .await;
        assert!(
            after_close["events"]
                .as_array()
                .unwrap()
                .iter()
                .all(|e| e["message"]["topic"] != "shared/owner-value/will"),
            "{after_close}"
        );
        let (_, summary) = create(&shared).await;
        let path = format!(
            "{}/sessions/{}",
            shared.base,
            summary["id"].as_str().unwrap()
        );
        until(&shared, &path, |s| s["state"] == "open").await;
        revise(&mut shared).await;
        assert!(matches!(
            call(&shared.router, "GET", &path, Some(&shared.editor), None)
                .await
                .0,
            StatusCode::CONFLICT | StatusCode::NOT_FOUND
        ));
        let (_, summary) = create(&shared).await;
        let path = format!(
            "{}/sessions/{}",
            shared.base,
            summary["id"].as_str().unwrap()
        );
        until(&shared, &path, |s| s["state"] == "open").await;
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
        assert_eq!(
            call(&shared.router, "GET", &path, Some(&shared.editor), None)
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
        // Fresh creator after original editor logout: role change still fences the actual MQTT session.
        let (status, opened) = call(
            &shared.router,
            "POST",
            &format!("{}/sessions", shared.base),
            Some(&shared.other),
            Some(json!({"request_id":"r","expected_revision":shared.saved["revision"]})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{opened}");
        let other_path = format!(
            "{}/sessions/{}",
            shared.base,
            opened["id"].as_str().unwrap()
        );
        let (_, members) = call(
            &shared.router,
            "GET",
            &format!("{}/members", shared.base),
            Some(&shared.owner),
            None,
        )
        .await;
        let other = members
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["username"] == "mqttother")
            .unwrap();
        assert_eq!(
            call(
                &shared.router,
                "PATCH",
                &format!(
                    "{}/members/{}",
                    shared.base,
                    other["member_id"].as_str().unwrap()
                ),
                Some(&shared.owner),
                Some(json!({"role":"viewer","expected_revision":other["revision"]}))
            )
            .await
            .0,
            StatusCode::OK
        );
        assert_eq!(
            call(
                &shared.router,
                "GET",
                &other_path,
                Some(&shared.other),
                None
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            call(
                &shared.router,
                "POST",
                &format!("{observer_path}/close"),
                Some(&shared.owner),
                Some(json!({}))
            )
            .await
            .0,
            StatusCode::OK
        );
    }
}

async fn owner_event(shared: &Shared, path: &str, predicate: impl Fn(&Value) -> bool) -> Value {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let (status, batch) = call(
                &shared.router,
                "GET",
                &format!("{path}/events"),
                Some(&shared.owner),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{batch}");
            if let Some(event) = batch["events"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| predicate(e))
            {
                return event.clone();
            }
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
    })
    .await
    .unwrap()
}
