use moleapi_core::{DatasetSource, SavedDataset};
use moleapi_script_runtime::dataset_query::query_dataset;
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
}
