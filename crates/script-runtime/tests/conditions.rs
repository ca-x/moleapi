use moleapi_core::{RequestSpec, VariableScopes};
use moleapi_script_runtime::condition_worker;
use serde_json::json;
fn request() -> RequestSpec {
    serde_json::from_value(json!({"id":"r","name":"r","method":"GET","url":"https://example.test","description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":true,"verify_tls":true,"assertions":[],"examples":[]})).unwrap()
}
#[tokio::test]
async fn conditions_return_host_boolean_and_reject_async_or_nonboolean_results() {
    let worker = std::path::Path::new(env!("CARGO_BIN_EXE_moleapi-script-worker"));
    let mut scopes = VariableScopes::default();
    scopes.temporary.insert("ready".into(), "yes".into());
    for (expression, expected) in [
        ("pm.variables.get('ready') === 'yes'", true),
        ("false", false),
        ("(pm.variables.set('issued','private-value'),true)", true),
    ] {
        let output = condition_worker(worker, expression, &request(), None, &scopes)
            .await
            .unwrap();
        assert_eq!(output.condition_result, Some(expected));
        assert!(!scopes.temporary.contains_key("issued"));
    }
    for expression in [
        "1",
        "'true'",
        "Promise.resolve(true)",
        "(()=>{Promise.resolve();return true})()",
    ] {
        assert!(
            condition_worker(worker, expression, &request(), None, &scopes)
                .await
                .is_err(),
            "{expression}"
        );
    }
}
