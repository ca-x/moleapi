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
    assert!(moleapi_formats::import("moleapi", &private).is_ok());
    let full = export(&w, "moleapi", true).unwrap().content;
    assert!(full.contains("private-reader-qa"));
    assert!(full.contains("private-password-qa"));
}

#[test]
fn nacos_namespace_and_private_credentials_roundtrip_through_export() {
    let mut workspace = workspace();
    let request = &mut workspace.data.collections[0].requests[0];
    request.url = "nacos://registry.example:8848".into();
    let Protocol::Dubbo { config } = &mut request.protocol else {
        unreachable!()
    };
    config.source_version = 2;
    config.source = "nacos".into();
    config.group = "qa".into();
    config.registry_namespace = "moleapi-qa".into();
    config.registry_auth = Some(DubboRegistryAuth {
        username: "private-nacos-reader".into(),
        password: "private-nacos-password".into(),
    });
    let screened = export(&workspace, "moleapi", false).unwrap().content;
    assert!(screened.contains("moleapi-qa"));
    assert!(!screened.contains("private-nacos-reader"));
    assert!(!screened.contains("private-nacos-password"));
    assert!(moleapi_formats::import("moleapi", &screened).is_ok());
    let full = export(&workspace, "moleapi", true).unwrap().content;
    assert!(full.contains("moleapi-qa"));
    assert!(full.contains("private-nacos-reader"));
    assert!(full.contains("private-nacos-password"));
}

#[test]
fn metadata_center_reference_survives_default_backup_without_credentials() {
    let mut workspace = workspace();
    let request = &mut workspace.data.collections[0].requests[0];
    let Protocol::Dubbo { config } = &mut request.protocol else {
        unreachable!()
    };
    config.metadata_center_url = "zookeeper://metadata.example:2181".into();
    config.metadata_application = "provider-app".into();
    config.metadata_auth = Some(DubboRegistryAuth {
        username: "private-metadata-reader".into(),
        password: "private-metadata-password".into(),
    });
    let screened = export(&workspace, "moleapi", false).unwrap().content;
    assert!(screened.contains("zookeeper://metadata.example:2181"));
    assert!(screened.contains("provider-app"));
    assert!(!screened.contains("private-metadata-reader"));
    assert!(!screened.contains("private-metadata-password"));
    assert!(moleapi_formats::import("moleapi", &screened).is_ok());
    let full = export(&workspace, "moleapi", true).unwrap().content;
    assert!(full.contains("private-metadata-reader"));
    assert!(full.contains("private-metadata-password"));
}

#[test]
fn provider_metadata_source_screens_nested_values_in_default_backup() {
    let mut workspace = workspace();
    workspace.data.specifications.push(moleapi_core::Specification {
        id: "metadata".into(), name: "Provider metadata".into(),
        kind: "dubbo-metadata".into(), dialect: "dubbo-3.3.6".into(),
        source: json!({"format":"moleapi-dubbo-metadata-source-v1",
            "url":"zookeeper://metadata.example:2181",
            "document_sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "source_request_id":"r","workspace_revision_at_import":1,
            "raw":"{\"parameters\":{\"token\":\"nested-metadata-token\",\"password\":\"nested-metadata-password\"}}"}).to_string(),
    });
    let screened = export(&workspace, "moleapi", false).unwrap().content;
    assert!(!screened.contains("nested-metadata-token"));
    assert!(!screened.contains("nested-metadata-password"));
    let restored = moleapi_formats::import("moleapi", &screened).unwrap();
    let screened_source: serde_json::Value =
        serde_json::from_str(&restored.data.specifications[0].source).unwrap();
    assert_eq!(screened_source["source_request_id"], "r");
    assert_eq!(screened_source["workspace_revision_at_import"], 1);
    assert_eq!(
        screened_source["document_sha256"],
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    );
    let full = export(&workspace, "moleapi", true).unwrap().content;
    assert!(full.contains("nested-metadata-token"));
    assert!(full.contains("nested-metadata-password"));
    let restored_full = moleapi_formats::import("moleapi", &full).unwrap();
    assert_eq!(
        restored_full.data.specifications[0].source,
        workspace.data.specifications[0].source
    );
}
