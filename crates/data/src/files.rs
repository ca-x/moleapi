use crate::{
    Cell, Column, MAX_CELL, MAX_COLUMNS, MAX_FILE_ROWS, QueryResult, SchemaColumn, SchemaTable,
};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use datafusion::{
    arrow::{
        array::{Array, BinaryArray, FixedSizeBinaryArray, LargeBinaryArray},
        csv,
        datatypes::DataType,
        json,
        record_batch::RecordBatch,
        util::display::array_value_to_string,
    },
    datasource::MemTable,
    execution::{
        context::{SQLOptions, SessionConfig, SessionContext},
        disk_manager::{DiskManagerBuilder, DiskManagerMode},
        memory_pool::GreedyMemoryPool,
        object_store::ObjectStoreRegistry,
        runtime_env::RuntimeEnvBuilder,
    },
    object_store::ObjectStore,
};
use futures_util::StreamExt;
use moleapi_core::{DataConfig, DataFileFormat, DataSource, MAX_DATA_FILE};
use std::{io::Cursor, sync::Arc, time::Instant};
#[derive(Debug)]
struct NoExternalStores;
impl ObjectStoreRegistry for NoExternalStores {
    fn register_store(
        &self,
        _: &url::Url,
        _: Arc<dyn ObjectStore>,
    ) -> Option<Arc<dyn ObjectStore>> {
        None
    }
    fn get_store(&self, _: &url::Url) -> datafusion::common::Result<Arc<dyn ObjectStore>> {
        Err(datafusion::common::DataFusionError::Execution(
            "External file/network stores are disabled".into(),
        ))
    }
}
pub struct FileSource {
    context: SessionContext,
    schema: SchemaTable,
}
impl FileSource {
    pub fn open(config: &DataConfig, bytes: &[u8]) -> Result<Self> {
        moleapi_core::validate_data_config(config)?;
        ensure!(bytes.len() <= MAX_DATA_FILE, "Data file exceeds 5 MiB");
        let batches = read_batches(config, bytes)?;
        let schema = batches
            .first()
            .context("File contains no readable table schema")?
            .schema();
        ensure!(
            schema.fields().len() <= MAX_COLUMNS,
            "File table exceeds column limit"
        );
        ensure!(
            schema
                .fields()
                .iter()
                .all(|f| f.name().len() <= 512 && f.data_type().to_string().len() <= 2048),
            "File column metadata exceeds limits"
        );
        let total_rows: usize = batches.iter().map(RecordBatch::num_rows).sum();
        let total_bytes: usize = batches.iter().map(RecordBatch::get_array_memory_size).sum();
        ensure!(
            total_rows <= MAX_FILE_ROWS && total_bytes <= 64 * 1024 * 1024,
            "Decoded file table exceeds row/memory limits"
        );
        let runtime = RuntimeEnvBuilder::new()
            .with_memory_pool(Arc::new(GreedyMemoryPool::new(64 * 1024 * 1024)))
            .with_disk_manager_builder(
                DiskManagerBuilder::default().with_mode(DiskManagerMode::Disabled),
            )
            .with_object_store_registry(Arc::new(NoExternalStores))
            .build_arc()?;
        let context = SessionContext::new_with_config_rt(
            SessionConfig::new()
                .with_target_partitions(1)
                .with_batch_size(256),
            runtime,
        );
        context.register_table(
            datafusion::common::TableReference::bare(config.table_name.clone()),
            Arc::new(MemTable::try_new(schema.clone(), vec![batches])?),
        )?;
        let reference = crate::quoted(&config.table_name, DataSource::LocalFile);
        let columns = schema
            .fields()
            .iter()
            .map(|f| SchemaColumn {
                name: f.name().clone(),
                data_type: f.data_type().to_string(),
                nullable: f.is_nullable(),
                reference: format!(
                    "{reference}.{}",
                    crate::quoted(f.name(), DataSource::LocalFile)
                ),
            })
            .collect();
        Ok(Self {
            context,
            schema: SchemaTable {
                schema: String::new(),
                name: config.table_name.clone(),
                reference,
                columns,
            },
        })
    }
    pub fn schema(&self) -> SchemaTable {
        self.schema.clone()
    }
    pub async fn query(&self, sql: &str) -> Result<QueryResult> {
        crate::statement(sql, DataSource::LocalFile, true)?;
        let started = Instant::now();
        let options = SQLOptions::new()
            .with_allow_ddl(false)
            .with_allow_dml(false)
            .with_allow_statements(false);
        let frame = self.context.sql_with_options(sql, options).await?;
        let mut stream = frame.execute_stream().await?;
        let mut result = QueryResult::empty();
        let schema = stream.schema();
        ensure!(
            schema.fields().len() <= MAX_COLUMNS,
            "Query exceeds result column limit"
        );
        result.columns = schema
            .fields()
            .iter()
            .map(|f| Column {
                name: f.name().clone(),
                data_type: f.data_type().to_string(),
                nullable: f.is_nullable(),
            })
            .collect();
        let mut size = 0;
        'batches: while let Some(batch) = stream.next().await {
            let batch = batch?;
            for row in 0..batch.num_rows() {
                let cells = batch
                    .columns()
                    .iter()
                    .map(|a| arrow_cell(a.as_ref(), row))
                    .collect::<Result<Vec<_>>>()?;
                if !result.push(cells, &mut size)? {
                    break 'batches;
                }
            }
        }
        result.elapsed_ms = started.elapsed().as_millis() as u64;
        Ok(result)
    }
}
fn read_batches(config: &DataConfig, bytes: &[u8]) -> Result<Vec<RecordBatch>> {
    let mut output = Vec::new();
    let mut rows = 0;
    let mut memory = 0;
    let mut accept = |batch: RecordBatch| -> Result<()> {
        rows += batch.num_rows();
        memory += batch.get_array_memory_size();
        ensure!(
            rows <= MAX_FILE_ROWS && memory <= 64 * 1024 * 1024,
            "Decoded file table exceeds limits"
        );
        ensure!(
            batch.num_columns() <= MAX_COLUMNS,
            "Decoded file table exceeds column limit"
        );
        output.push(batch);
        Ok(())
    };
    match config.file_format {
        DataFileFormat::Csv => {
            let format = csv::reader::Format::default().with_header(config.csv_header);
            let (schema, _) = format.infer_schema(Cursor::new(bytes), Some(MAX_FILE_ROWS + 1))?;
            ensure!(
                schema.fields().len() <= MAX_COLUMNS,
                "CSV column limit exceeded"
            );
            // Floating inference can round literal decimal strings. Keep them as text;
            // SQL CAST AS DECIMAL provides deliberate precision for numeric operations.
            let schema = datafusion::arrow::datatypes::Schema::new(
                schema
                    .fields()
                    .iter()
                    .map(|f| {
                        if matches!(f.data_type(), DataType::Float32 | DataType::Float64) {
                            f.as_ref().clone().with_data_type(DataType::Utf8)
                        } else {
                            f.as_ref().clone()
                        }
                    })
                    .collect::<Vec<_>>(),
            );
            let schema = Arc::new(schema);
            let reader = csv::ReaderBuilder::new(schema.clone())
                .with_format(format)
                .with_batch_size(256)
                .build(Cursor::new(bytes))?;
            for batch in reader {
                accept(batch?)?;
            }
            if output.is_empty() {
                output.push(RecordBatch::new_empty(schema));
            }
        }
        DataFileFormat::Json => {
            let values = exact_json_rows(bytes)?;
            let schema = Arc::new(json::reader::infer_json_schema_from_iterator(
                values
                    .iter()
                    .map(Ok::<_, datafusion::arrow::error::ArrowError>),
            )?);
            ensure!(
                schema.fields().len() <= MAX_COLUMNS,
                "JSON column limit exceeded"
            );
            let mut lines = Vec::new();
            for value in values {
                serde_json::to_writer(&mut lines, &value)?;
                lines.push(b'\n');
            }
            let reader = json::ReaderBuilder::new(schema.clone())
                .with_batch_size(256)
                .build(Cursor::new(lines))?;
            for batch in reader {
                accept(batch?)?;
            }
            if output.is_empty() {
                output.push(RecordBatch::new_empty(schema));
            }
        }
        DataFileFormat::Parquet => {
            use datafusion::parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
            let reader =
                ParquetRecordBatchReaderBuilder::try_new(bytes::Bytes::copy_from_slice(bytes))?;
            ensure!(
                reader.schema().fields().len() <= MAX_COLUMNS,
                "Parquet column limit exceeded"
            );
            ensure!(
                reader.metadata().file_metadata().num_rows() >= 0
                    && reader.metadata().file_metadata().num_rows() <= MAX_FILE_ROWS as i64,
                "Parquet row limit exceeded"
            );
            let expanded: i64 = reader
                .metadata()
                .row_groups()
                .iter()
                .map(|group| group.total_byte_size())
                .sum();
            ensure!(
                (0..=64 * 1024 * 1024).contains(&expanded),
                "Parquet declared decoded size exceeds limit"
            );
            let schema = reader.schema().clone();
            for batch in reader.with_batch_size(256).build()? {
                accept(batch?)?;
            }
            if output.is_empty() {
                output.push(RecordBatch::new_empty(schema));
            }
        }
    }
    Ok(output)
}
fn exact_json_rows(bytes: &[u8]) -> Result<Vec<serde_json::Value>> {
    use serde_json::{Value, value::RawValue};
    use std::collections::{BTreeMap, BTreeSet};
    type RawRow = BTreeMap<String, Box<RawValue>>;
    let raw: Vec<RawRow> = if bytes.iter().copied().find(|c| !c.is_ascii_whitespace()) == Some(b'[')
    {
        serde_json::from_slice(bytes)?
    } else {
        serde_json::Deserializer::from_slice(bytes)
            .into_iter::<RawRow>()
            .collect::<Result<_, _>>()?
    };
    ensure!(raw.len() <= MAX_FILE_ROWS, "JSON row limit exceeded");
    let numeric =
        |value: &RawValue| matches!(value.get().as_bytes().first(), Some(b'-' | b'0'..=b'9'));
    let text_fields: BTreeSet<_> = raw
        .iter()
        .flat_map(|row| row.iter())
        .filter(|(_, value)| numeric(value) && value.get().parse::<i64>().is_err())
        .map(|(name, _)| name.clone())
        .collect();
    // RawValue keeps numeric lexemes exact without changing serde_json's global
    // Number representation (which would affect other mature protocol SDKs).
    fn exact(value: &RawValue, depth: usize) -> Result<Value> {
        ensure!(depth <= 32, "JSON table nesting exceeds limit");
        Ok(match value.get().as_bytes().first() {
            Some(b'{') => {
                let fields: RawRow = serde_json::from_str(value.get())?;
                Value::Object(
                    fields
                        .into_iter()
                        .map(|(key, value)| Ok((key, exact(&value, depth + 1)?)))
                        .collect::<Result<_>>()?,
                )
            }
            Some(b'[') => {
                let values: Vec<Box<RawValue>> = serde_json::from_str(value.get())?;
                Value::Array(
                    values
                        .iter()
                        .map(|value| exact(value, depth + 1))
                        .collect::<Result<_>>()?,
                )
            }
            Some(b'-' | b'0'..=b'9') => Value::String(value.get().into()),
            _ => serde_json::from_str(value.get())?,
        })
    }
    raw.into_iter()
        .map(|row| {
            ensure!(row.len() <= MAX_COLUMNS, "JSON column limit exceeded");
            Ok(Value::Object(
                row.into_iter()
                    .map(|(name, value)| {
                        let cell = if numeric(&value) && !text_fields.contains(&name) {
                            Value::from(value.get().parse::<i64>()?)
                        } else {
                            exact(&value, 0)?
                        };
                        Ok((name, cell))
                    })
                    .collect::<Result<_>>()?,
            ))
        })
        .collect()
}
fn arrow_cell(array: &dyn Array, row: usize) -> Result<Cell> {
    if array.is_null(row) {
        return Ok(Cell::Null);
    }
    let binary = match array.data_type() {
        DataType::Binary => Some(
            array
                .as_any()
                .downcast_ref::<BinaryArray>()
                .context("Binary type mismatch")?
                .value(row),
        ),
        DataType::LargeBinary => Some(
            array
                .as_any()
                .downcast_ref::<LargeBinaryArray>()
                .context("Large binary type mismatch")?
                .value(row),
        ),
        DataType::FixedSizeBinary(_) => Some(
            array
                .as_any()
                .downcast_ref::<FixedSizeBinaryArray>()
                .context("Fixed binary type mismatch")?
                .value(row),
        ),
        _ => None,
    };
    if let Some(value) = binary {
        return Ok(if value.len() > MAX_CELL {
            Cell::Truncated {
                preview: STANDARD.encode(&value[..MAX_CELL]),
                bytes: value.len(),
            }
        } else {
            Cell::Binary {
                base64: STANDARD.encode(value),
                bytes: value.len(),
            }
        });
    }
    let value = array_value_to_string(array, row)?;
    if value.len() > MAX_CELL {
        let end = value.floor_char_boundary(MAX_CELL);
        return Ok(Cell::Truncated {
            preview: value[..end].into(),
            bytes: value.len(),
        });
    }
    Ok(match array.data_type() {
        DataType::Boolean => Cell::Bool {
            value: value == "true",
        },
        DataType::Int8
        | DataType::Int16
        | DataType::Int32
        | DataType::Int64
        | DataType::UInt8
        | DataType::UInt16
        | DataType::UInt32
        | DataType::UInt64 => Cell::Integer { value },
        DataType::Float16
        | DataType::Float32
        | DataType::Float64
        | DataType::Decimal128(_, _)
        | DataType::Decimal256(_, _) => Cell::Decimal { value },
        _ => Cell::Text { value },
    })
}
