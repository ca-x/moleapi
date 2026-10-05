use crate::{Cell, MAX_SCHEMA_COLUMNS, MAX_TABLES, QueryResult, SchemaColumn, SchemaTable};
use anyhow::{Result, ensure};
use moleapi_core::DataSource;
use std::collections::BTreeMap;
pub const PG_SCHEMA_SQL: &str = "SELECT table_schema, table_name, column_name, data_type, is_nullable FROM information_schema.columns WHERE table_schema NOT IN ('pg_catalog','information_schema') ORDER BY table_schema,table_name,ordinal_position";
pub const MYSQL_SCHEMA_SQL: &str = "SELECT TABLE_SCHEMA, TABLE_NAME, COLUMN_NAME, DATA_TYPE, IS_NULLABLE FROM information_schema.COLUMNS WHERE TABLE_SCHEMA=DATABASE() ORDER BY TABLE_SCHEMA,TABLE_NAME,ORDINAL_POSITION";
pub fn schema_tables(result: QueryResult, source: DataSource) -> Result<(Vec<SchemaTable>, bool)> {
    let mut tables: BTreeMap<(String, String), SchemaTable> = BTreeMap::new();
    let mut truncated = result.truncated;
    for (count, row) in result.rows.into_iter().enumerate() {
        ensure!(row.len() == 5, "Invalid schema metadata column count");
        let text = |i: usize| -> Result<String> {
            match &row[i] {
                Cell::Text { value } => Ok(value.clone()),
                _ => anyhow::bail!("Invalid schema metadata cell type"),
            }
        };
        let schema = text(0)?;
        let table = text(1)?;
        let column = text(2)?;
        ensure!(
            [&schema, &table, &column].iter().all(|s| s.len() <= 512),
            "Schema identifier exceeds size limit"
        );
        let key = (schema.clone(), table.clone());
        if !tables.contains_key(&key) && tables.len() >= MAX_TABLES {
            truncated = true;
            break;
        }
        if count >= MAX_SCHEMA_COLUMNS {
            truncated = true;
            break;
        }
        let reference = format!(
            "{}.{}",
            crate::quoted(&schema, source),
            crate::quoted(&table, source)
        );
        let entry = tables.entry(key).or_insert_with(|| SchemaTable {
            schema,
            name: table,
            reference: reference.clone(),
            columns: Vec::new(),
        });
        entry.columns.push(SchemaColumn {
            name: column.clone(),
            data_type: text(3)?,
            nullable: text(4)?.eq_ignore_ascii_case("YES"),
            reference: format!("{reference}.{}", crate::quoted(&column, source)),
        });
    }
    Ok((tables.into_values().collect(), truncated))
}
