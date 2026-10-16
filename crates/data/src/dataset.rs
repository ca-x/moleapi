//! Convert a complete typed Data result to the existing bounded runner source.
use crate::{Cell, QueryResult};
use anyhow::{Result, ensure};
use moleapi_core::{DATASET_LIMIT, DatasetSource};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub fn dataset_source(result: &QueryResult) -> Result<DatasetSource> {
    ensure!(!result.truncated, "Data result is truncated");
    ensure!(
        (1..=100).contains(&result.rows.len()),
        "Data dataset requires 1 to 100 rows"
    );
    ensure!(
        (1..=100).contains(&result.columns.len()),
        "Data dataset requires 1 to 100 columns"
    );
    let mut names = BTreeSet::new();
    for column in &result.columns {
        ensure!(
            !column.name.trim().is_empty() && names.insert(&column.name),
            "Data dataset columns must have unique nonempty names"
        );
    }
    let mut rows = Vec::with_capacity(result.rows.len());
    for cells in &result.rows {
        ensure!(
            cells.len() == result.columns.len(),
            "Data dataset row and column counts differ"
        );
        let mut row = BTreeMap::new();
        for (column, cell) in result.columns.iter().zip(cells) {
            let value = match cell {
                Cell::Null => Value::Null,
                Cell::Bool { value } => Value::Bool(*value),
                Cell::Text { value } | Cell::Integer { value } | Cell::Decimal { value } => {
                    Value::String(value.clone())
                }
                Cell::Binary { .. } | Cell::Truncated { .. } => {
                    anyhow::bail!("Binary or truncated Data cells cannot become dataset values")
                }
            };
            row.insert(column.name.clone(), value);
        }
        rows.push(row);
    }
    let source = serde_json::to_string(&rows)?;
    ensure!(source.len() <= DATASET_LIMIT, "Data dataset exceeds 1 MiB");
    let source = DatasetSource {
        format: "json".into(),
        source,
    };
    source.parse()?;
    Ok(source)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Column;

    fn result(rows: Vec<Vec<Cell>>) -> QueryResult {
        QueryResult {
            columns: ["id", "amount", "ok"]
                .map(|name| Column {
                    name: name.into(),
                    data_type: "TEXT".into(),
                    nullable: true,
                })
                .to_vec(),
            rows,
            rows_affected: 0,
            elapsed_ms: 0,
            truncated: false,
            limit_reason: None,
        }
    }

    #[test]
    fn preserves_exact_numeric_text_and_rejects_partial_results() {
        let complete = result(vec![vec![
            Cell::Integer {
                value: "9007199254740993".into(),
            },
            Cell::Decimal {
                value: "2.30".into(),
            },
            Cell::Bool { value: true },
        ]]);
        let source = dataset_source(&complete).unwrap();
        let parsed = source.parse().unwrap();
        assert_eq!(parsed.rows[0]["id"], "9007199254740993");
        assert_eq!(parsed.rows[0]["amount"], "2.30");
        assert_eq!(parsed.rows[0]["ok"], true);
        let mut bad = complete.clone();
        bad.truncated = true;
        assert!(dataset_source(&bad).is_err());
        bad = complete.clone();
        bad.rows[0][0] = Cell::Binary {
            base64: "AA==".into(),
            bytes: 1,
        };
        assert!(dataset_source(&bad).is_err());
        bad = complete.clone();
        bad.columns[1].name = "id".into();
        assert!(dataset_source(&bad).is_err());
        bad = complete;
        bad.rows.push(vec![Cell::Null]);
        assert!(dataset_source(&bad).is_err());
        let too_many = result(vec![vec![Cell::Null, Cell::Null, Cell::Null]; 101]);
        assert!(dataset_source(&too_many).is_err());
        let too_large = result(vec![
            vec![
                Cell::Text {
                    value: "x".repeat(20_000),
                },
                Cell::Null,
                Cell::Null,
            ];
            100
        ]);
        assert!(dataset_source(&too_large).is_err());
    }
}
