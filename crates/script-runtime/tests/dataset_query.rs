use moleapi_core::{DatasetSource, SavedDataset, SavedDatasetView};
use moleapi_script_runtime::dataset_query::{
    QuerySnapshot, query_dataset, query_snapshot, query_view, snapshot_dataset,
};
use serde_json::json;

fn source(format: &str, text: &str) -> SavedDataset {
    SavedDataset {
        id: "sales".into(),
        name: "Sales".into(),
        table_name: "orders".into(),
        description: String::new(),
        secret: true,
        source: Some(DatasetSource {
            format: format.into(),
            source: text.into(),
        }),
        views: vec![],
    }
}

#[test]
fn literal_dataset_sql_uses_bound_parameters_and_exact_integer_text() {
    let dataset = source(
        "json",
        r#"[{"id":9007199254740993,"name":"O'Reilly"},{"id":2,"name":"Other"}]"#,
    );
    let rows = query_dataset(
        &dataset,
        "SELECT id, name FROM orders WHERE name = ?",
        &["O'Reilly".into()],
    )
    .unwrap();
    assert_eq!(
        rows,
        vec![json!({"id":"9007199254740993","name":"O'Reilly"})]
    );
    let rows = query_dataset(
        &dataset,
        "SELECT name FROM orders WHERE name = ?",
        &["' OR 1=1 --".into()],
    )
    .unwrap();
    assert!(rows.is_empty());
}

#[test]
fn literal_dataset_sql_preserves_direct_boolean_null_and_decimal_text() {
    let dataset = source(
        "json",
        r#"[{"flag":true,"empty":null,"price":12.3400},{"flag":false,"empty":null,"price":0.1}]"#,
    );
    let rows = query_dataset(
        &dataset,
        "SELECT flag, empty, price FROM orders ORDER BY price",
        &[],
    )
    .unwrap();
    assert_eq!(rows[0]["flag"], false);
    assert!(rows[0]["empty"].is_null());
    assert_eq!(rows[0]["price"], "0.1");
    assert_eq!(rows[1]["price"], "12.3400");
}

#[test]
fn host_snapshot_preserves_exact_numbers_and_private_values_across_ipc() {
    let dataset = source(
        "json",
        r#"[{"price":12.3400,"secret":{"token":"nested-private","nested_price":1.2300}}]"#,
    );
    let (snapshot, private_values) = snapshot_dataset(&dataset).unwrap();
    let restored: QuerySnapshot =
        serde_json::from_slice(&serde_json::to_vec(&snapshot).unwrap()).unwrap();
    assert_eq!(
        query_snapshot(&restored, "SELECT price FROM orders", &[]).unwrap()[0]["price"],
        "12.3400"
    );
    assert!(private_values.contains("nested-private"));
    assert!(private_values.contains("12.3400"));
    assert!(
        query_snapshot(&restored, "SELECT secret FROM orders", &[]).unwrap()[0]["secret"]
            .as_str()
            .unwrap()
            .contains("1.2300")
    );
}

#[test]
fn saved_view_queries_share_bound_readonly_sql_rules() {
    let mut dataset = source("csv", "id,name\n1,alpha\n2,beta\n");
    dataset.views.push(SavedDatasetView {
        id: "by-name".into(),
        name: "By name".into(),
        sql: Some("SELECT id FROM orders WHERE name = ?".into()),
    });
    let snapshot = snapshot_dataset(&dataset).unwrap().0;
    assert_eq!(
        query_view(&snapshot, "by-name", &["beta".into()]).unwrap(),
        vec![json!({"id":"2"})]
    );
    assert!(query_view(&snapshot, "missing", &[]).is_err());
    assert!(
        query_view(&snapshot, "by-name", &["' OR 1=1 --".into()])
            .unwrap()
            .is_empty()
    );
}

#[test]
fn mixed_boolean_and_numeric_columns_reject_before_type_is_lost() {
    let dataset = source("json", r#"[{"value":true},{"value":2}]"#);
    assert!(snapshot_dataset(&dataset).is_err());
}

#[test]
fn dataset_sql_rejects_writes_multiple_statements_and_runaway_queries() {
    let dataset = source("csv", "id,name\n1,A\n2,B\n");
    for sql in [
        "DELETE FROM orders",
        "SELECT * FROM orders; SELECT * FROM orders",
        "PRAGMA table_info(orders)",
        "WITH x AS (DELETE FROM orders RETURNING *) SELECT * FROM x",
    ] {
        assert!(query_dataset(&dataset, sql, &[]).is_err(), "{sql}");
    }
    assert!(
        query_dataset(
            &dataset,
            "SELECT a.id FROM orders a, orders b, orders c, orders d, orders e, orders f, orders g",
            &[]
        )
        .is_err()
    );
    assert!(query_dataset(&dataset, "SELECT ?", &vec!["x".into(); 65]).is_err());
    assert!(query_dataset(&dataset, &format!("SELECT 1 {}", " ".repeat(65_536)), &[]).is_err());
    assert!(query_dataset(&dataset, "SELECT id AS same, name AS same FROM orders", &[]).is_err());
    assert!(query_dataset(&dataset, "SELECT name FROM sqlite_master", &[]).is_err());
    let start = std::time::Instant::now();
    assert!(query_dataset(&dataset, "WITH RECURSIVE sequence(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM sequence) SELECT max(n) FROM sequence", &[]).is_err());
    assert!(start.elapsed() < std::time::Duration::from_secs(1));
}
