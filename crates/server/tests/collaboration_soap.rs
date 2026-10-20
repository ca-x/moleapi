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
        moleapi_server::sqlite_database_url(&dir.path().join("soap.db")).unwrap(),
        true,
    );
    settings.allow_private_network = true;
    let router = hosted(settings).await.unwrap();
    let owner = register(&router, "soapowner").await;
    let editor = register(&router, "soapeditor").await;
    let other = register(&router, "soapother").await;
    let viewer = register(&router, "soapviewer").await;
    let foreign = register(&router, "soapforeign").await;
    let mut data = example_data();
    data["script_mode"] = "v2".into();
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = url.into();
    request["examples"] = json!([]);
    request["protocol"] = json!({"kind":"soap","version":"1.1","action":"Echo"});
    request["method"] = "POST".into();
    request["body_kind"] = "text".into();
    request["body"] = envelope("1.1", "{{who}}").into();
    request["auth"] =
        json!({"kind":"bearer","token":"{{fixture_token}}","username":"","password":""});
    request["timeout_ms"] = json!(30000);
    request["pre_request_script"] =
        "pm.request.headers.add({key:'X-Shared-Soap',value:'owner-script'});".into();
    data["global_variables"] = json!([{"id":"v","key":"who","value":"owner-value","enabled":true},{"id":"s","key":"fixture_token","value":"fixture-token","enabled":true,"secret":true}]);
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"SOAP","data":data})),
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
fn envelope(version: &str, name: &str) -> String {
    let ns = if version == "1.1" {
        "http://schemas.xmlsoap.org/soap/envelope/"
    } else {
        "http://www.w3.org/2003/05/soap-envelope"
    };
    format!(
        "<s:Envelope xmlns:s=\"{ns}\"><s:Body><Echo xmlns=\"urn:moleapi:soap\"><name>{name}</name></Echo></s:Body></s:Envelope>"
    )
}
fn bundle(version: &str) -> String {
    json!({"entry_file":"service.wsdl","files":[{"path":"service.wsdl","content":if version=="1.1"{include_str!("fixtures/soap/spyne11.wsdl")}else{include_str!("fixtures/soap/spyne12.wsdl")}}]}).to_string()
}
#[tokio::test]
async fn member_soap_candidates_are_staged_saved_request_bound_and_privacy_screened() {
    let shared = setup("http://127.0.0.1:1").await;
    let input = json!({"request_id":"r","expected_revision":shared.saved["revision"],"name":"Shared WSDL","source":bundle("1.1")});
    for (token, expected) in [
        (&shared.viewer, StatusCode::FORBIDDEN),
        (&shared.foreign, StatusCode::NOT_FOUND),
    ] {
        assert_eq!(
            call(
                &shared.router,
                "POST",
                &format!("{}/soap/import", shared.base),
                Some(token),
                Some(input.clone())
            )
            .await
            .0,
            expected
        );
    }
    let (status, candidate) = call(
        &shared.router,
        "POST",
        &format!("{}/soap/import", shared.base),
        Some(&shared.editor),
        Some(input),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{candidate}");
    assert_eq!(candidate["specification"]["source"], bundle("1.1"));
    assert!(
        !candidate["schema"]["services"][0]["ports"][0]["operations"][0]["template"]
            .as_str()
            .unwrap()
            .is_empty()
    );
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
    let request = shared.saved["data"]["collections"][0]["requests"][0].clone();
    let (status,result)=call(&shared.router,"POST",&format!("{}/execute-draft",shared.base),Some(&shared.editor),Some(json!({"request":request,"collection_id":"c","expected_workspace_revision":shared.saved["revision"]}))).await;
    assert!(
        !result["error"]
            .as_str()
            .unwrap_or("")
            .contains("Draft execution supports"),
        "{status}: {result}"
    );
}

async fn revise(shared: &mut Shared) {
    let (status,value)=call(&shared.router,"PUT",&shared.base,Some(&shared.editor),Some(json!({"name":shared.saved["name"],"data":shared.saved["data"],"expected_revision":shared.saved["revision"]}))).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    shared.saved = value;
}
#[tokio::test]
#[ignore = "requires documented real Spyne SOAP1.1/1.2 fixtures on18897/18898"]
async fn real_shared_soap_url_candidates_cas_schema_calls_faults_and_canonical_source() {
    for (version, port) in [("1.1", 18897), ("1.2", 18898)] {
        let mut shared = setup(&format!("http://127.0.0.1:{port}/")).await;
        shared.saved["data"]["collections"][0]["requests"][0]["protocol"]["version"] =
            version.into();
        shared.saved["data"]["collections"][0]["requests"][0]["body"] =
            envelope(version, "{{who}}").into();
        revise(&mut shared).await;
        let input = json!({"request_id":"r","expected_revision":shared.saved["revision"],"name":"Real shared WSDL","url":format!("http://127.0.0.1:{port}/?wsdl"),"verify_tls":true,"timeout_ms":30000});
        let (status, candidate) = call(
            &shared.router,
            "POST",
            &format!("{}/soap/import-url", shared.base),
            Some(&shared.editor),
            Some(input),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{candidate}");
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
        let (spec, schema) = (&candidate["specification"], &candidate["schema"]);
        assert_eq!(call(&shared.router,"POST",&format!("{}/soap/schema",shared.base),Some(&shared.editor),Some(json!({"request_id":"r","specification_id":spec["id"],"expected_revision":shared.saved["revision"]}))).await.0,StatusCode::NOT_FOUND);
        shared.saved["data"]["specifications"] = json!([spec]);
        let r = &mut shared.saved["data"]["collections"][0]["requests"][0];
        r["specification_id"] = spec["id"].clone();
        let service = &schema["services"][0];
        let p = &service["ports"][0];
        let o = &p["operations"][0];
        r["protocol"] = json!({"kind":"soap","version":version,"service":service["name"],"port":p["name"],"operation":o["name"],"action":o["action"]});
        revise(&mut shared).await;
        let (status,loaded)=call(&shared.router,"POST",&format!("{}/soap/schema",shared.base),Some(&shared.other),Some(json!({"request_id":"r","specification_id":spec["id"],"expected_revision":shared.saved["revision"]}))).await;
        assert_eq!(status, StatusCode::OK, "{loaded}");
        assert_eq!(loaded["specification"]["source"], spec["source"]);
        let (status, saved_call) = call(
            &shared.router,
            "POST",
            &format!("{}/requests/r/execute", shared.base),
            Some(&shared.editor),
            Some(json!({})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{saved_call}");
        assert_eq!(saved_call["status"], 200, "{saved_call}");
        assert!(
            saved_call["body"]
                .as_str()
                .unwrap()
                .contains("Hello owner-value"),
            "{saved_call}"
        );
        for (name, target_status) in [("temporary call", 200), ("fault", 500)] {
            let mut request = shared.saved["data"]["collections"][0]["requests"][0].clone();
            request["body"] = envelope(version, name).into();
            let (status,result)=call(&shared.router,"POST",&format!("{}/execute-draft",shared.base),Some(&shared.editor),Some(json!({"request":request,"collection_id":"c","expected_workspace_revision":shared.saved["revision"]}))).await;
            assert_eq!(status, StatusCode::OK, "{result}");
            assert_eq!(result["status"], target_status, "{result}");
            if target_status == 500 {
                assert_eq!(
                    result["soap_fault"]["reason"], "Fixture rejected name",
                    "{result}"
                );
            } else {
                assert!(
                    result["body"]
                        .as_str()
                        .unwrap()
                        .contains("Hello temporary call"),
                    "{result}"
                );
            }
        }
        let mut request = shared.saved["data"]["collections"][0]["requests"][0].clone();
        request["auth"]["kind"] = "none".into();
        let (_,unauthorized)=call(&shared.router,"POST",&format!("{}/execute-draft",shared.base),Some(&shared.editor),Some(json!({"request":request,"collection_id":"c","expected_workspace_revision":shared.saved["revision"]}))).await;
        assert_eq!(unauthorized["status"], 401, "{unauthorized}");
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
        let (_, audit) = call(
            &shared.router,
            "GET",
            &format!("{}/audit", shared.base),
            Some(&shared.owner),
            None,
        )
        .await;
        for action in [
            "soap.import_url",
            "soap.schema.read",
            "request.execute",
            "request.draft.execute",
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
            "fixture-token",
            "Hello",
            "Envelope",
            "definitions",
            "temporary call",
        ] {
            assert!(!audit.to_string().contains(content), "{audit}");
        }
    }
}
#[tokio::test]
async fn shared_soap_source_privacy_and_binding_rejects_canonical_collection_secret_and_overrides()
{
    let mut shared = setup("http://127.0.0.1:1").await;
    shared.saved["data"]["collections"][0]["variables"] = json!([{"id":"private","key":"private","value":"collection-private","secret":true,"enabled":true}]);
    revise(&mut shared).await;
    let source = bundle("1.1").replace(
        "name=\\\"Application\\\"",
        "name=\\\"collection-privat&#101;\\\"",
    );
    assert!(source.contains("privat&#101;"));
    let (status,value)=call(&shared.router,"POST",&format!("{}/soap/import",shared.base),Some(&shared.editor),Some(json!({"request_id":"r","expected_revision":shared.saved["revision"],"name":"Candidate","source":source}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{value}");
    assert!(
        value["error"].as_str().unwrap().contains("private scoped"),
        "{value}"
    );
    for (request_id, revision, expected) in [
        (
            "foreign",
            shared.saved["revision"].as_i64().unwrap(),
            StatusCode::NOT_FOUND,
        ),
        ("r", 0, StatusCode::CONFLICT),
    ] {
        assert_eq!(call(&shared.router,"POST",&format!("{}/soap/import",shared.base),Some(&shared.editor),Some(json!({"request_id":request_id,"expected_revision":revision,"name":"Candidate","source":bundle("1.1")}))).await.0,expected);
    }
    let request = shared.saved["data"]["collections"][0]["requests"][0].clone();
    let (status,value)=call(&shared.router,"POST",&format!("{}/execute-draft",shared.base),Some(&shared.editor),Some(json!({"request":request,"collection_id":"c","expected_workspace_revision":shared.saved["revision"],"locals":[{"scope":"environment","key":"who","value":"override"}]}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{value}");
    assert!(
        value["error"]
            .as_str()
            .unwrap()
            .contains("canonical workspace scopes")
    );
}

#[tokio::test]
async fn shared_soap_pending_source_and_finite_calls_drop_sockets_after_role_revision_or_logout() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    for operation in ["source", "saved", "draft"] {
        for boundary in ["role", "revision", "logout"] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
            let (closed_tx, closed_rx) = tokio::sync::oneshot::channel();
            let peer = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut header = Vec::new();
                let mut b = [0];
                while !header.ends_with(b"\r\n\r\n") {
                    assert_eq!(socket.read(&mut b).await.unwrap(), 1);
                    header.push(b[0]);
                }
                let wire = String::from_utf8_lossy(&header).to_ascii_lowercase();
                if operation != "source" {
                    assert!(wire.contains("x-shared-soap: owner-script"));
                    assert!(wire.contains("authorization: bearer fixture-token"));
                } else {
                    assert!(!wire.contains("authorization:"));
                }
                // Consume any SOAP request body before watching for client EOF.
                let length = String::from_utf8_lossy(&header)
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|v| v.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                let mut body = vec![0; length];
                socket.read_exact(&mut body).await.unwrap();
                socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/xml\r\nContent-Length: 65535\r\n\r\n").await.unwrap();
                ready_tx.send(()).unwrap();
                let result = socket.read(&mut b).await;
                let _ = closed_tx.send(result);
            });
            let mut shared = setup(&format!("http://{address}/")).await;
            let (url, input) = match operation {
                "source" => (
                    format!("{}/soap/import-url", shared.base),
                    json!({"request_id":"r","expected_revision":shared.saved["revision"],"name":"Held source","url":format!("http://{address}/"),"verify_tls":true,"timeout_ms":30000}),
                ),
                "draft" => (
                    format!("{}/execute-draft", shared.base),
                    json!({"request":shared.saved["data"]["collections"][0]["requests"][0],"collection_id":"c","expected_workspace_revision":shared.saved["revision"]}),
                ),
                _ => (format!("{}/requests/r/execute", shared.base), json!({})),
            };
            let router = shared.router.clone();
            let actor = shared.editor.clone();
            let work = tokio::spawn(async move {
                call(&router, "POST", &url, Some(&actor), Some(input)).await
            });
            tokio::time::timeout(Duration::from_secs(3), ready_rx)
                .await
                .unwrap()
                .unwrap();
            let expected = match boundary {
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
                        .find(|m| m["username"] == "soapeditor")
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
                "revision" => {
                    revise(&mut shared).await;
                    StatusCode::CONFLICT
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
            let (status, value) = tokio::time::timeout(Duration::from_secs(2), work)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(status, expected, "{operation}/{boundary}: {value}");
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
async fn shared_soap_can_save_empty_endpoint_before_wsdl_import_but_cannot_execute_it() {
    let shared = setup("").await;
    let (status,candidate)=call(&shared.router,"POST",&format!("{}/soap/import",shared.base),Some(&shared.editor),Some(json!({"request_id":"r","expected_revision":shared.saved["revision"],"name":"Endpoint discovery","source":bundle("1.1")}))).await;
    assert_eq!(status, StatusCode::OK, "{candidate}");
    let (status, response) = call(
        &shared.router,
        "POST",
        &format!("{}/requests/r/execute", shared.base),
        Some(&shared.editor),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{response}");
    assert!(
        response["error"].as_str().unwrap().contains("URL"),
        "{response}"
    );
}
