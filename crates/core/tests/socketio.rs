use moleapi_core::*;
use serde_json::json;
fn request() -> RequestSpec {
    serde_json::from_value(json!({"id":"r","name":"Socket.IO","method":"GET","url":"wss://example.com","description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":false,"verify_tls":true,"assertions":[],"examples":[],"protocol":{"kind":"socketio","namespace":"/{{namespace}}","path":"/{{path}}/","auth_source":"{\"token\":\"{{token}}\",\"nested\":{\"value\":\"{{token}}\"}}","listeners":["{{event}}"],"event":"{{laterEvent}}","arguments_source":"[invalid {{laterMessage}}","attachments_base64":["{{laterBinary}}"]}})).unwrap()
}
#[test]
fn structural_auth_interpolation_preserves_json_quotes_and_ignores_saved_outbound_drafts() {
    let request = request();
    let environment: Environment = serde_json::from_value(json!({"id":"env","name":"env","variables":[{"id":"v1","key":"token","value":"quote\"slash\\newline\n","enabled":true},{"id":"v2","key":"namespace","value":"admin","enabled":true},{"id":"v3","key":"path","value":"custom","enabled":true},{"id":"v4","key":"event","value":"echo","enabled":true}]})).unwrap();
    let resolved = resolve_request(&request, Some(&environment)).unwrap();
    let Protocol::Socketio {
        namespace,
        path,
        auth_source,
        listeners,
        event,
        arguments_source,
        attachments_base64,
        ..
    } = resolved.protocol
    else {
        unreachable!()
    };
    assert_eq!(namespace, "/admin");
    assert_eq!(path, "/custom/");
    assert_eq!(listeners, ["echo"]);
    let auth: serde_json::Value = serde_json::from_str(&auth_source).unwrap();
    assert_eq!(auth["token"], "quote\"slash\\newline\n");
    assert_eq!(auth["nested"]["value"], auth["token"]);
    assert_eq!(event, "{{laterEvent}}");
    assert_eq!(arguments_source, "[invalid {{laterMessage}}");
    assert_eq!(attachments_base64, ["{{laterBinary}}"]);
    assert_eq!(
        serde_json::to_value(&request.protocol).unwrap()["auth_source"],
        "{\"token\":\"{{token}}\",\"nested\":{\"value\":\"{{token}}\"}}"
    );
}
#[test]
fn draft_defaults_and_execution_configuration_bounds() {
    let minimal: Protocol = serde_json::from_value(json!({"kind":"socketio"})).unwrap();
    let value = serde_json::to_value(minimal).unwrap();
    assert_eq!(value["namespace"], "/");
    assert_eq!(value["path"], "/socket.io/");
    assert_eq!(value["auth_source"], "{}");
    assert_eq!(value["arguments_source"], "[]");
    assert!(!value["request_ack"].as_bool().unwrap());
    for event in [
        "connect",
        "connect_error",
        "disconnect",
        "disconnecting",
        "newListener",
        "removeListener",
        "",
        "bad\nname",
    ] {
        assert!(validate_socketio_event(event).is_err());
    }
    for event in ["error", "open", "close", "custom:event/with spaces"] {
        assert!(
            validate_socketio_event(event).is_ok(),
            "Ordinary application event: {event}"
        );
    }
    let mut req = request();
    req.protocol = serde_json::from_value(json!({"kind":"socketio","namespace":"","path":"","auth_source":"{broken","event":"","arguments_source":"[broken"})).unwrap();
    assert!(validate_request(&req, true).is_ok());
    assert!(validate_request(&req, false).is_err());
    req.protocol =
        serde_json::from_value(json!({"kind":"socketio","listeners":vec!["echo";65]})).unwrap();
    assert!(validate_request(&req, true).is_err());
    req.protocol = serde_json::from_value(json!({"kind":"socketio","auth_source":"[]"})).unwrap();
    assert!(validate_request(&req, false).is_err());
}
