use moleapi_core::*;
use moleapi_formats::{export, import};
use serde_json::json;
#[test]
fn dataset_backups_keep_metadata_hide_private_copies_and_restore_exact_source() {
    let private = r#"[ {"auth":{"password":"dataset-private-token"}} ]"#;
    let public = "value\npublic-fixture\n";
    let workspace:Workspace=serde_json::from_value(json!({"id":"w","name":"Datasets","revision":1,"updated_at":"now","data":{"schema_version":1,"collections":[{"id":"c","name":"Requests","description":"dataset-private-token","requests":[]}],"environments":[],"active_environment_id":null,"datasets":[{"id":"private","name":"Private","source":{"format":"json","source":private}},{"id":"public","name":"Public","secret":false,"source":{"format":"csv","source":public}},{"id":"copied","name":"Copied","secret":false,"source":{"format":"csv","source":"value\ndataset-private-token\n"}}]}})).unwrap();
    validate_workspace(&workspace.data).unwrap();
    let safe = export(&workspace, "moleapi", false).unwrap();
    assert!(!safe.content.contains("dataset-private-token"));
    let restored = import("moleapi", &safe.content).unwrap();
    assert_eq!(restored.data.datasets.len(), 3);
    assert!(restored.data.datasets[0].source.is_none());
    assert_eq!(
        restored.data.datasets[1].source.as_ref().unwrap().source,
        public
    );
    assert!(restored.data.datasets[2].source.is_none());
    validate_workspace(&restored.data).unwrap();
    let exact = export(&workspace, "moleapi", true).unwrap();
    let restored = import("moleapi", &exact.content).unwrap();
    assert_eq!(restored.data.datasets, workspace.data.datasets);
    assert!(export(&workspace, "postman", true).is_err());
}
