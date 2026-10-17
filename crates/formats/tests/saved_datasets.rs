use moleapi_core::*;
use moleapi_formats::{export, import};
use serde_json::json;
#[test]
fn dataset_backups_keep_metadata_hide_private_copies_and_restore_exact_source() {
    let private = r#"[ {"auth":{"password":"dataset-private-token"}} ]"#;
    let public = "value\npublic-fixture\n";
    let workspace:Workspace=serde_json::from_value(json!({"id":"w","name":"Datasets","revision":1,"updated_at":"now","data":{"schema_version":1,"collections":[{"id":"c","name":"Requests","description":"dataset-private-token","requests":[]}],"environments":[],"active_environment_id":null,"datasets":[{"id":"private","name":"Private","source":{"format":"json","source":private},"views":[{"id":"v1","name":"Secret view","sql":"SELECT * FROM data WHERE password = 'dataset-private-token'"}]},{"id":"public","name":"Public","secret":false,"source":{"format":"csv","source":public},"views":[{"id":"v2","name":"Public view","sql":"SELECT * FROM data"}]},{"id":"copied","name":"Copied","secret":false,"source":{"format":"csv","source":"value\ndataset-private-token\n"}}]}})).unwrap();
    validate_workspace(&workspace.data).unwrap();
    let safe = export(&workspace, "moleapi", false).unwrap();
    assert!(!safe.content.contains("dataset-private-token"));
    let restored = import("moleapi", &safe.content).unwrap();
    assert_eq!(restored.data.datasets.len(), 3);
    assert!(restored.data.datasets[0].source.is_none());
    assert!(restored.data.datasets[0].views[0].sql.is_none());
    assert_eq!(
        restored.data.datasets[1].source.as_ref().unwrap().source,
        public
    );
    assert!(restored.data.datasets[2].source.is_none());
    assert_eq!(
        restored.data.datasets[1].views[0].sql.as_deref(),
        Some("SELECT * FROM data")
    );
    validate_workspace(&restored.data).unwrap();
    let exact = export(&workspace, "moleapi", true).unwrap();
    let restored = import("moleapi", &exact.content).unwrap();
    assert_eq!(restored.data.datasets, workspace.data.datasets);
    assert!(export(&workspace, "postman", true).is_err());
}

#[test]
fn query_dataset_requires_saved_read_only_data_request_and_default_backup_withholds_link() {
    let mut imported = import("har", include_str!("fixtures/capture.har")).unwrap();
    let request = &mut imported.data.collections[0].requests[0];
    request.protocol = Protocol::Data {
        config: Box::new(DataConfig {
            source: DataSource::RemoteFile,
            sql: "SELECT * FROM data".into(),
            ..DataConfig::default()
        }),
    };
    request.headers.clear();
    let request_id = request.id.clone();
    imported.data.datasets.push(SavedDataset {
        id: "live".into(),
        name: "Live rows".into(),
        table_name: "data".into(),
        description: String::new(),
        secret: true,
        source: None,
        views: vec![],
        query: Some(SavedQueryDataset { request_id }),
    });
    let workspace = Workspace {
        id: "w".into(),
        name: "Live".into(),
        revision: 1,
        updated_at: "now".into(),
        data: imported.data,
    };
    validate_workspace(&workspace.data).unwrap();
    let safe = export(&workspace, "moleapi", false).unwrap();
    let restored = import("moleapi", &safe.content).unwrap();
    assert!(restored.data.datasets[0].query.is_none());
    let full = export(&workspace, "moleapi", true).unwrap();
    let restored = import("moleapi", &full.content).unwrap();
    assert_eq!(
        restored.data.datasets[0].query,
        workspace.data.datasets[0].query
    );

    let mut invalid = workspace.data.clone();
    invalid.datasets[0].query.as_mut().unwrap().request_id = "missing".into();
    assert!(validate_workspace(&invalid).is_err());
    invalid = workspace.data.clone();
    invalid.collections[0].requests[0].protocol = Protocol::Http;
    assert!(validate_workspace(&invalid).is_err());
    invalid = workspace.data.clone();
    if let Protocol::Data { config } = &mut invalid.collections[0].requests[0].protocol {
        config.read_only = false;
    }
    assert!(validate_workspace(&invalid).is_err());
}
