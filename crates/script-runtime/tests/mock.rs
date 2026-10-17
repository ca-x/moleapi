use moleapi_core::{RequestSpec, SavedDataset, VariableScopes};
use moleapi_script_runtime::{MockResponseDraft, run_mock};
use serde_json::json;

#[test]
fn mock_script_queries_a_saved_dataset_and_builds_a_bounded_response() {
    let request: RequestSpec = serde_json::from_value(json!({"id":"r","name":"R","method":"GET","url":"http://mock.invalid/example","description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":true,"verify_tls":true,"assertions":[],"examples":[]})).unwrap();
    let dataset: SavedDataset = serde_json::from_value(json!({"id":"d","name":"Rows","source":{"format":"json","source":"[{\"name\":\"alpha\"}]"}})).unwrap();
    let snapshot = moleapi_script_runtime::dataset_query::snapshot_dataset(&dataset)
        .unwrap()
        .0;
    let seed = MockResponseDraft {
        status: 200,
        headers: vec![],
        body: "static".into(),
    };
    let output = run_mock(&["const result=await pm.datasets('d').executeQuery('SELECT name FROM data');for await(const row of result.rows){pm.mock.setStatus(202);pm.mock.setHeader('x-row',row.name);pm.mock.setBody({name:row.name});}".into()], &request, &VariableScopes::default(), &[snapshot], &seed).unwrap();
    let response = output.mock_response.unwrap();
    assert_eq!(response.status, 202);
    assert_eq!(response.headers[0].value, "alpha");
    assert_eq!(response.body, "{\"name\":\"alpha\"}");
    assert_eq!(output.request, request);
}
#[tokio::test]
async fn isolated_mock_worker_changes_a_response_and_rejects_invalid_status() {
    let request: RequestSpec = serde_json::from_value(json!({"id":"r","name":"R","method":"GET","url":"http://mock.invalid/example","description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":true,"verify_tls":true,"assertions":[],"examples":[]})).unwrap();
    let seed = MockResponseDraft {
        status: 200,
        headers: vec![],
        body: "static".into(),
    };
    let executable = std::path::Path::new(env!("CARGO_BIN_EXE_moleapi-script-worker"));
    let output = moleapi_script_runtime::run_worker_with_mock(executable, vec!["pm.mock.setStatus(202);pm.mock.setHeader('x-result','yes');pm.mock.setBody({ok:true});".into()], &request, &VariableScopes::default(), vec![], seed.clone()).await.unwrap();
    assert_eq!(output.mock_response.unwrap().body, "{\"ok\":true}");
    assert!(
        moleapi_script_runtime::run_worker_with_mock(
            executable,
            vec!["pm.mock.setStatus(700);".into()],
            &request,
            &VariableScopes::default(),
            vec![],
            seed
        )
        .await
        .is_err()
    );
}
