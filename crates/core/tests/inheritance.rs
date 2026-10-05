use moleapi_core::*;
use serde_json::json;
fn data() -> WorkspaceData {
    serde_json::from_value(json!({"schema_version":1,"auth":{"kind":"bearer","token":"workspace","username":"","password":""},"collections":[
        {"id":"root","name":"Collection","description":"","auth":{"kind":"bearer","token":"{{token}}","username":"","password":""},"variables":[{"id":"v1","key":"token","value":"root","enabled":true}],"requests":[]},
        {"id":"folder","parent_id":"root","name":"Folder","description":"","variables":[{"id":"v2","key":"token","value":"leaf","enabled":true}],"requests":[{"id":"request","name":"Request","method":"GET","url":"https://example.test","description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"inherit","token":"","username":"","password":""},"timeout_ms":3000,"verify_tls":true,"follow_redirects":true,"assertions":[],"examples":[]}]}
    ],"environments":[],"active_environment_id":null})).unwrap()
}
#[test]
fn inherited_source_is_live_and_does_not_copy_credentials_into_drafts() {
    let mut d = data();
    validate_workspace(&d).unwrap();
    let c = &d.collections[1];
    let r = &c.requests[0];
    let scopes = VariableScopes::new(&d, Some(c), None, &[], &[], false).unwrap();
    let auth = inherited_authentication(&d, Some(c), r, Some(&scopes.effective())).unwrap();
    assert_eq!(auth.source, AuthenticationSource::Collection("root".into()));
    assert_eq!(auth.auth.token, "{{token}}");
    let prepared =
        inherit_request_authentication(&d, Some(c), r, Some(&scopes.effective())).unwrap();
    assert_eq!(
        resolve_request(&prepared, Some(&scopes.effective()))
            .unwrap()
            .auth
            .token,
        "leaf"
    );
    d.collections[0].auth.as_mut().unwrap().token = "changed-parent".into();
    assert_eq!(
        inherit_request_authentication(
            &d,
            Some(&d.collections[1]),
            &d.collections[1].requests[0],
            None
        )
        .unwrap()
        .auth
        .token,
        "changed-parent"
    );
    assert_eq!(d.collections[1].requests[0].auth.kind, "inherit");
    assert!(d.collections[1].requests[0].auth.token.is_empty());
}
#[test]
fn explicit_none_stops_inheritance_and_missing_parents_do_not_fall_back() {
    let mut d = data();
    d.collections[1].auth = Some(
        serde_json::from_value(json!({"kind":"none","token":"","username":"","password":""}))
            .unwrap(),
    );
    assert_eq!(
        inherited_authentication(
            &d,
            Some(&d.collections[1]),
            &d.collections[1].requests[0],
            None
        )
        .unwrap()
        .auth
        .kind,
        "none"
    );
    d.collections[1].parent_id = Some("foreign-workspace".into());
    assert!(validate_workspace(&d).is_err());
}
#[test]
fn cycles_depth_and_duplicate_source_ids_are_rejected() {
    let mut d = data();
    d.collections[0].parent_id = Some("folder".into());
    assert!(validate_workspace(&d).is_err());
    let mut d = data();
    d.collections[1].id = "root".into();
    assert!(validate_workspace(&d).is_err());
    let mut d = data();
    for index in 0..MAX_COLLECTION_DEPTH {
        let mut c = d.collections[0].clone();
        c.id = format!("deep{index}");
        c.parent_id = Some(if index == 0 {
            "folder".into()
        } else {
            format!("deep{}", index - 1)
        });
        d.collections.push(c);
    }
    assert!(validate_workspace(&d).is_err());
}
#[test]
fn workspace_default_and_dynamic_inherit_selector_follow_resolved_precedence() {
    let mut d = data();
    d.collections[0].auth = None;
    let c = &d.collections[1];
    let mut r = c.requests[0].clone();
    r.auth.kind = "{{mode}}".into();
    let e:Environment=serde_json::from_value(json!({"id":"env","name":"Env","variables":[{"id":"mode","key":"mode","value":"inherit","enabled":true}]})).unwrap();
    let auth = inherited_authentication(&d, Some(c), &r, Some(&e)).unwrap();
    assert_eq!(auth.source, AuthenticationSource::Workspace);
    assert_eq!(auth.auth.token, "workspace");
    assert!(
        inherited_authentication(
            &d,
            Some(c),
            &r,
            Some(&Environment {
                id: "e".into(),
                name: "e".into(),
                variables: vec![]
            })
        )
        .is_err()
    );
}

#[tokio::test]
async fn standalone_execution_requires_workspace_resolution_for_inherit() {
    let d = data();
    let mut r = d.collections[1].requests[0].clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    r.url = format!("http://{}/", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            axum::Router::new().route("/", axum::routing::get(|| async { "ok" })),
        )
        .await
        .unwrap()
    });
    let policy = NetworkPolicy {
        allow_private_network: true,
    };
    assert!(request_headers(&r).is_err());
    assert!(execute(&r, None, policy).await.is_err());
    assert!(execute_bytes(&r, vec![], policy).await.is_err());
    r.auth.kind = "none".into();
    assert_eq!(execute(&r, None, policy).await.unwrap().status, 200);
    server.abort();
}

#[test]
fn retained_source_only_folder_variables_do_not_change_native_execution() {
    let mut d = data();
    d.collections[1].variables_enabled = Some(false);
    let scope = VariableScopes::new(&d, Some(&d.collections[1]), None, &[], &[], false).unwrap();
    assert_eq!(scope.collection["token"], "root");
    d.collections[1].variables_enabled = Some(true);
    let scope = VariableScopes::new(&d, Some(&d.collections[1]), None, &[], &[], false).unwrap();
    assert_eq!(scope.collection["token"], "leaf");
}

#[test]
fn stdio_mcp_inheritance_cannot_silently_ignore_parent_http_credentials() {
    let mut d = data();
    d.collections[1].requests[0].protocol =
        serde_json::from_value(json!({"kind":"mcp","transport":"stdio"})).unwrap();
    assert!(validate_workspace(&d).is_err());
    d.collections[1].requests[0].auth.kind = "none".into();
    validate_workspace(&d).unwrap();
}
