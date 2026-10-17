//! Isolated in-memory SQL over one saved literal dataset.
use anyhow::{Context, Result, ensure};
use moleapi_core::SavedDataset;
use rusqlite::{
    Connection,
    hooks::{AuthAction, Authorization},
    limits::Limit,
    params_from_iter,
    types::Value as SqlValue,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, value::RawValue};
use sqlparser::{ast::Statement, dialect::SQLiteDialect, parser::Parser};
use std::collections::BTreeMap;
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

const MAX_SQL: usize = 64 * 1024;
const MAX_PARAMS: usize = 64;
const MAX_ROWS: usize = 100;
const MAX_COLUMNS: usize = 100;
const MAX_RESULT_BYTES: usize = 1024 * 1024;
const MAX_OPS: usize = 100_000;

fn identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuerySnapshot {
    pub id: String,
    pub table_name: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<QueryCell>>,
    pub views: Vec<QueryView>,
    #[serde(default)]
    pub live_query: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueryView {
    pub id: String,
    pub sql: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum QueryCell {
    Null,
    Boolean(bool),
    Integer(i64),
    Text(String),
}

fn query_cell(value: &Value, raw_value: Option<&RawValue>) -> Result<QueryCell> {
    Ok(match value {
        Value::Null => QueryCell::Null,
        Value::Bool(value) => QueryCell::Boolean(*value),
        Value::Number(value) => {
            let exact = raw_value.map_or_else(|| value.to_string(), |raw| raw.get().to_owned());
            exact
                .parse::<i64>()
                .map_or_else(|_| QueryCell::Text(exact), QueryCell::Integer)
        }
        Value::String(value) => QueryCell::Text(value.clone()),
        value => QueryCell::Text(match raw_value {
            Some(raw) => raw.get().to_owned(),
            None => serde_json::to_string(value)?,
        }),
    })
}

pub fn snapshot_dataset(
    dataset: &SavedDataset,
) -> Result<(QuerySnapshot, std::collections::BTreeSet<String>)> {
    let source = dataset
        .source
        .as_ref()
        .context("Saved dataset source is missing")?;
    let data = source.parse()?;
    ensure!(
        data.columns.iter().all(|name| !name.contains('\0')),
        "Dataset SQL columns cannot contain NUL"
    );
    for name in &data.columns {
        let values = data
            .rows
            .iter()
            .filter_map(|row| row.get(name))
            .filter(|value| !value.is_null())
            .collect::<Vec<_>>();
        ensure!(
            !values.iter().any(|value| value.is_boolean())
                || values.iter().all(|value| value.is_boolean()),
            "Mixed boolean dataset columns cannot preserve SQL result types"
        );
    }
    let mut private_values = moleapi_core::dataset_private_values(&data.rows)?;
    let raw_rows = if source.format == "json" {
        Some(serde_json::from_str::<Vec<BTreeMap<String, Box<RawValue>>>>(&source.source)?)
    } else {
        None
    };
    let rows = data
        .rows
        .iter()
        .enumerate()
        .map(|(row_index, row)| {
            data.columns
                .iter()
                .map(|name| {
                    let value = row.get(name).unwrap_or(&Value::Null);
                    let raw = raw_rows
                        .as_ref()
                        .and_then(|rows| rows.get(row_index))
                        .and_then(|row| row.get(name))
                        .map(Box::as_ref);
                    query_cell(value, raw)
                })
                .collect::<Result<Vec<_>>>()
        })
        .collect::<Result<Vec<_>>>()?;
    for row in &rows {
        for cell in row {
            if let QueryCell::Text(value) = cell
                && !value.is_empty()
            {
                private_values.insert(value.clone());
            }
        }
    }
    ensure!(
        private_values.len() <= 5000
            && private_values.iter().map(String::len).sum::<usize>()
                <= 4 * moleapi_core::MAX_VARIABLE_BYTES,
        "Dataset private values exceed execution limit"
    );
    Ok((
        QuerySnapshot {
            id: dataset.id.clone(),
            table_name: dataset.table_name.clone(),
            columns: data.columns,
            rows,
            views: dataset
                .views
                .iter()
                .map(|view| QueryView {
                    id: view.id.clone(),
                    sql: view.sql.clone(),
                })
                .collect(),
            live_query: false,
        },
        private_values,
    ))
}

pub fn query_dataset(dataset: &SavedDataset, sql: &str, params: &[String]) -> Result<Vec<Value>> {
    query_snapshot(&snapshot_dataset(dataset)?.0, sql, params)
}

pub fn query_view(dataset: &QuerySnapshot, view_id: &str, params: &[String]) -> Result<Vec<Value>> {
    let view = dataset
        .views
        .iter()
        .find(|view| view.id == view_id)
        .context("Saved view ID is unavailable")?;
    let sql = view
        .sql
        .as_deref()
        .context("Saved view SQL is unavailable")?;
    query_snapshot(dataset, sql, params)
}

pub fn live_query_marker(dataset: &SavedDataset) -> QuerySnapshot {
    QuerySnapshot {
        id: dataset.id.clone(),
        table_name: dataset.table_name.clone(),
        columns: vec![],
        rows: vec![],
        views: dataset
            .views
            .iter()
            .map(|view| QueryView {
                id: view.id.clone(),
                sql: view.sql.clone(),
            })
            .collect(),
        live_query: true,
    }
}

pub fn query_snapshot(dataset: &QuerySnapshot, sql: &str, params: &[String]) -> Result<Vec<Value>> {
    ensure!(
        !dataset.live_query,
        "Live query-backed datasets require an on-demand script bridge"
    );
    ensure!(
        !dataset.columns.is_empty()
            && dataset.columns.len() <= MAX_COLUMNS
            && !dataset.rows.is_empty()
            && dataset.rows.len() <= MAX_ROWS
            && dataset
                .rows
                .iter()
                .all(|row| row.len() == dataset.columns.len()),
        "Invalid dataset query snapshot"
    );
    ensure!(
        !sql.trim().is_empty() && sql.len() <= MAX_SQL,
        "Invalid dataset SQL length"
    );
    ensure!(
        params.len() <= MAX_PARAMS,
        "Dataset query exceeds 64 parameters"
    );
    let statements = Parser::parse_sql(&SQLiteDialect {}, sql).context("Invalid dataset SQL")?;
    ensure!(
        statements.len() == 1 && matches!(statements[0], Statement::Query(_)),
        "Dataset SQL must be one read-only SELECT or CTE"
    );
    let connection = Connection::open_in_memory()?;
    connection.set_limit(Limit::SQLITE_LIMIT_LENGTH, (MAX_RESULT_BYTES + 1) as i32);
    connection.set_limit(Limit::SQLITE_LIMIT_COLUMN, MAX_COLUMNS as i32);
    connection.execute_batch("PRAGMA temp_store=MEMORY")?;
    let schema = dataset
        .columns
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let is_boolean = dataset
                .rows
                .iter()
                .filter_map(|row| row.get(index))
                .filter(|value| !matches!(value, QueryCell::Null))
                .all(|value| matches!(value, QueryCell::Boolean(_)));
            format!(
                "{} {}",
                identifier(name),
                if is_boolean { "BOOLEAN" } else { "" }
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    connection.execute(
        &format!(
            "CREATE TABLE {} ({schema})",
            identifier(&dataset.table_name)
        ),
        [],
    )?;
    let placeholders = vec!["?"; dataset.columns.len()].join(",");
    let insert = format!(
        "INSERT INTO {} VALUES ({placeholders})",
        identifier(&dataset.table_name)
    );
    let mut insert_stmt = connection.prepare(&insert)?;
    for row in &dataset.rows {
        let values = row
            .iter()
            .map(|cell| match cell {
                QueryCell::Null => SqlValue::Null,
                QueryCell::Boolean(value) => SqlValue::Integer(i64::from(*value)),
                QueryCell::Integer(value) => SqlValue::Integer(*value),
                QueryCell::Text(value) => SqlValue::Text(value.clone()),
            })
            .collect::<Vec<_>>();
        insert_stmt.execute(params_from_iter(values))?;
    }
    drop(insert_stmt);
    connection.set_limit(Limit::SQLITE_LIMIT_SQL_LENGTH, MAX_SQL as i32);
    connection.set_limit(Limit::SQLITE_LIMIT_VARIABLE_NUMBER, MAX_PARAMS as i32);
    let counter = Arc::new(AtomicUsize::new(0));
    let tracker = counter.clone();
    let deadline = Instant::now() + Duration::from_millis(200);
    connection.progress_handler(
        100,
        Some(move || {
            tracker.fetch_add(100, Ordering::Relaxed) >= MAX_OPS || Instant::now() >= deadline
        }),
    );
    let table_name = dataset.table_name.clone();
    connection.authorizer(Some(
        move |context: rusqlite::hooks::AuthContext<'_>| match context.action {
            AuthAction::Select | AuthAction::Recursive => Authorization::Allow,
            AuthAction::Read {
                table_name: table, ..
            } if table.eq_ignore_ascii_case(&table_name) => Authorization::Allow,
            AuthAction::Function { function_name }
                if !function_name.eq_ignore_ascii_case("load_extension") =>
            {
                Authorization::Allow
            }
            _ => Authorization::Deny,
        },
    ));
    let mut statement = connection
        .prepare(sql)
        .context("Cannot prepare dataset SELECT")?;
    ensure!(
        statement.readonly() && statement.column_count() <= MAX_COLUMNS,
        "Dataset query is not read-only or has too many columns"
    );
    let column_names = statement
        .column_names()
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<Vec<_>>();
    ensure!(
        column_names.iter().all(|name| !name.is_empty()),
        "Dataset query columns must have names"
    );
    ensure!(
        column_names
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == column_names.len(),
        "Dataset query column names must be unique"
    );
    let bool_columns = statement
        .columns()
        .iter()
        .map(|column| column.decl_type() == Some("BOOLEAN"))
        .collect::<Vec<_>>();
    let mut result = Vec::new();
    let mut bytes = 0usize;
    let mut rows = statement.query(params_from_iter(params.iter()))?;
    while let Some(row) = rows.next()? {
        ensure!(result.len() < MAX_ROWS, "Dataset query exceeds 100 rows");
        let mut object = Map::new();
        for (index, name) in column_names.iter().enumerate() {
            let value = match row.get_ref(index)? {
                rusqlite::types::ValueRef::Null => Value::Null,
                rusqlite::types::ValueRef::Integer(value) if bool_columns[index] => {
                    Value::Bool(value != 0)
                }
                rusqlite::types::ValueRef::Integer(value) => Value::String(value.to_string()),
                rusqlite::types::ValueRef::Real(value) => Value::String(value.to_string()),
                rusqlite::types::ValueRef::Text(value) => {
                    Value::String(String::from_utf8(value.to_vec())?)
                }
                rusqlite::types::ValueRef::Blob(_) => {
                    anyhow::bail!("Binary dataset query cells are unsupported")
                }
            };
            object.insert(name.clone(), value);
        }
        let value = Value::Object(object);
        bytes = bytes.saturating_add(serde_json::to_vec(&value)?.len());
        ensure!(
            bytes <= MAX_RESULT_BYTES,
            "Dataset query result exceeds 1 MiB"
        );
        result.push(value);
    }
    Ok(result)
}
