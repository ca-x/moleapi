use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Cell {
    Null,
    Text { value: String },
    Bool { value: bool },
    Integer { value: String },
    Decimal { value: String },
    Binary { base64: String, bytes: usize },
    Truncated { preview: String, bytes: usize },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueryResult {
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Cell>>,
    pub rows_affected: u64,
    pub elapsed_ms: u64,
    pub truncated: bool,
    pub limit_reason: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SchemaTable {
    pub schema: String,
    pub name: String,
    pub reference: String,
    pub columns: Vec<SchemaColumn>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SchemaColumn {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub reference: String,
}
pub const MAX_ROWS: usize = 1000;
pub const MAX_CELL: usize = 64 * 1024;
pub const MAX_RESULT: usize = 6 * 1024 * 1024;
pub const MAX_COLUMNS: usize = 256;
pub const MAX_TABLES: usize = 512;
pub const MAX_SCHEMA_COLUMNS: usize = 16384;
pub const MAX_FILE_ROWS: usize = 50000;
impl QueryResult {
    pub fn empty() -> Self {
        Self {
            columns: vec![],
            rows: vec![],
            rows_affected: 0,
            elapsed_ms: 0,
            truncated: false,
            limit_reason: None,
        }
    }
    pub fn push(&mut self, row: Vec<Cell>, bytes: &mut usize) -> anyhow::Result<bool> {
        if self.rows.len() >= MAX_ROWS {
            self.cap("Returned row limit reached");
            return Ok(false);
        }
        let size = serde_json::to_vec(&row)?.len();
        if size > 512 * 1024 {
            self.cap("One returned row exceeds session chunk limits");
            return Ok(false);
        }
        if bytes.saturating_add(size) > MAX_RESULT {
            self.cap("Returned byte limit reached");
            return Ok(false);
        }
        *bytes += size;
        self.rows.push(row);
        Ok(true)
    }
    pub fn cap(&mut self, reason: &str) {
        self.truncated = true;
        self.limit_reason = Some(reason.into());
    }
}
