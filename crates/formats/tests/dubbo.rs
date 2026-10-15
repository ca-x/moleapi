#![recursion_limit = "256"]
use moleapi_core::{DubboRegistryAuth, Protocol, Workspace};
use moleapi_formats::export;
use serde_json::json;

fn workspace() -> Workspace {
    serde_json::from_value(json!({
      "id":"w","name":"Classic","revision":1,"updated_at":"now",
      "data":{"schema_version":1,"collections":[{"id":"c","name":"API","description":"","requests":[{
        "id":"r","name":"Call","method":"GET","url":"dubbo://example.com:20880","description":"",
        "query":[],"headers":[],"body_kind":"none","body":"",
        "auth":{"kind":"none","token":"","username":"","password":""},
        "timeout_ms":5000,"follow_redirects":false,"verify_tls":true,"assertions":[],"examples":[],
        "protocol":{"kind":"dubbo","source_version":1,"transport":"classic","serialization":"hessian2",
          "service":"fixture.EchoService","method":"echo","version":"1.0.0","group":"",
          "arguments":[{"id":"one","type_name":"java.lang.String","value_source":"\"private-dubbo-value\"","secret":true},
                       {"id":"two","type_name":"java.util.Map","value_source":"{\"token\":\"private-nested-token\"}","secret":false}],
          "attachments":[{"id":"secret","key":"token","value":"private-attachment-value","enabled":true,"secret":true}]}
      }]}],"environments":[],"active_environment_id":null}
    })).unwrap()
}

#[test]
fn native_backup_preserves_explicit_dubbo_source_and_screens_default_values() {
    let w = workspace();
    let before = serde_json::to_value(&w).unwrap();
    let private = export(&w, "moleapi", false).unwrap().content;
    assert!(!private.contains("private-dubbo-value"));
    assert!(!private.contains("private-attachment-value"));
    assert!(!private.contains("private-nested-token"));
    assert!(private.contains("fixture.EchoService"));
    let full = export(&w, "moleapi", true).unwrap().content;
    assert!(
        full.contains("private-dubbo-value")
            && full.contains("private-attachment-value")
            && full.contains("private-nested-token")
    );
    assert_eq!(serde_json::to_value(w).unwrap(), before);
}

#[test]
fn external_exports_refuse_classic_dubbo_without_mislabeling_it_as_http() {
    let w = workspace();
    for format in ["postman", "openapi"] {
        let error = export(&w, format, true).err().unwrap().to_string();
        assert!(
            error.contains("专用协议") || error.contains("protocol"),
            "{error}"
        );
    }
}

#[test]
fn registry_credentials_are_private_in_default_backup_and_exact_in_explicit_export() {
    let mut w = workspace();
    let request = &mut w.data.collections[0].requests[0];
    request.url = "zookeeper://registry.example:2181".into();
    let Protocol::Dubbo { config } = &mut request.protocol else {
        unreachable!()
    };
    config.source_version = 2;
    config.source = "zookeeper".into();
    config.registry_auth = Some(DubboRegistryAuth {
        username: "private-reader-qa".into(),
        password: "private-password-qa".into(),
    });
    let private = export(&w, "moleapi", false).unwrap().content;
    assert!(!private.contains("private-reader-qa"));
    assert!(!private.contains("private-password-qa"));
    assert!(private.contains("zookeeper://registry.example:2181"));
    let full = export(&w, "moleapi", true).unwrap().content;
    assert!(full.contains("private-reader-qa"));
    assert!(full.contains("private-password-qa"));
}
