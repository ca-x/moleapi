use serde_json::{Value, json};
use std::{path::Path, process::Output, time::Duration};
pub async fn cli(args: &[&str]) -> Output {
    tokio::time::timeout(
        Duration::from_secs(30),
        tokio::process::Command::new(env!("CARGO_BIN_EXE_moleapi-cli"))
            .args(args)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .unwrap()
    .unwrap()
}
pub fn result(output: Output, code: i32) -> Value {
    assert_eq!(
        output.status.code(),
        Some(code),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
pub async fn serve(router: axum::Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (url, handle)
}
pub fn source(url: &str) -> Value {
    let mut first = json!({"id":"r","name":"Token","method":"GET","url":format!("{url}/token?row={{{{row}}}}"),"timeout_ms":5000,
        "extractions":[{"id":"extract","name":"Token","kind":"jsonpath","target":"$.token","scope":"environment","key":"token","required":true}],
        "post_response_script":"pm.test('extracted',()=>pm.expect(pm.environment.get('token')).to.equal(pm.response.json().token));"});
    let mut second = json!({"id":"echo","name":"Echo","method":"GET","url":format!("{url}/echo?row={{{{row}}}}"),"timeout_ms":5000,
        "headers":[{"id":"token","key":"X-Token","value":"{{token}}","enabled":true}],
        "post_response_script":"pm.test('row',()=>pm.expect(pm.response.json().row).to.equal(pm.iterationData.get('row')));"});
    let defaults = json!({"description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"follow_redirects":true,"verify_tls":true,"assertions":[],"examples":[]});
    for request in [&mut first, &mut second] {
        for (key, value) in defaults.as_object().unwrap() {
            request
                .as_object_mut()
                .unwrap()
                .entry(key.clone())
                .or_insert_with(|| value.clone());
        }
    }
    json!({"name":"CI","data":{"schema_version":1,"collections":[{"id":"c","name":"Smoke","description":"","requests":[first,second]}],
        "environments":[{"id":"dev","name":"Development","variables":[]},{"id":"prod","name":"Production","variables":[]}],
        "active_environment_id":"prod",
        "datasets":[{"id":"rows","name":"Rows","source":{"format":"json","source":"[{\"row\":\"one\"},{\"row\":\"two\"}]"}}],
        "scenarios":[{"id":"s","name":"Scenario","collection_id":"c","steps":[{"id":"a","request_id":"r"},{"id":"b","request_id":"echo"}]}]}})
}
pub fn write(path: &Path, value: &Value) {
    std::fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}
