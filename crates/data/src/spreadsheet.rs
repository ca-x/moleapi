//! Bounded spreadsheet to Arrow conversion for the isolated Data file worker.
use crate::{MAX_CELL, MAX_COLUMNS, MAX_FILE_ROWS};
use anyhow::{Context, Result, ensure};
use calamine::{Data, HeaderRow, Ods, Range, Reader, Xlsx, open_workbook_from_rs};
use datafusion::arrow::{
    array::{ArrayRef, BooleanArray, Int64Array, StringBuilder},
    datatypes::{DataType, Field, Schema},
    record_batch::RecordBatch,
};
use moleapi_core::{DataConfig, DataFileFormat};
use std::{collections::HashSet, io::Cursor, sync::Arc};

const MAX_EXPANDED: u64 = 64 * 1024 * 1024;
const MAX_ZIP_ENTRIES: usize = 2048;
const MAX_GRID_CELLS: usize = 1_000_000;

fn preflight_archive(bytes: &[u8]) -> Result<()> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).context("Invalid Spreadsheet ZIP archive")?;
    ensure!(
        archive.len() <= MAX_ZIP_ENTRIES,
        "Spreadsheet ZIP entry count exceeds limit"
    );
    let mut names = HashSet::new();
    let mut expanded = 0u64;
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        ensure!(
            entry.name().len() <= 1024 && names.insert(entry.name().to_owned()),
            "Spreadsheet ZIP has duplicate or oversized entry names"
        );
        expanded = expanded
            .checked_add(entry.size())
            .context("Spreadsheet ZIP expanded size overflow")?;
        ensure!(
            expanded <= MAX_EXPANDED,
            "Spreadsheet ZIP declared expanded size exceeds limit"
        );
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ColumnKind {
    Int,
    Bool,
    Text,
}

fn cell_kind(cell: &Data) -> Result<Option<ColumnKind>> {
    Ok(match cell {
        Data::Empty => None,
        Data::Int(_) => Some(ColumnKind::Int),
        Data::Float(value) if value.is_finite() => {
            if value.fract() == 0.0 && value.abs() <= 9_007_199_254_740_991.0 {
                Some(ColumnKind::Int)
            } else {
                Some(ColumnKind::Text)
            }
        }
        Data::Bool(_) => Some(ColumnKind::Bool),
        Data::Float(_) | Data::Error(_) => {
            anyhow::bail!("Spreadsheet contains an invalid or error cell")
        }
        _ => Some(ColumnKind::Text),
    })
}

fn cell_text(cell: &Data) -> Result<Option<String>> {
    let text = match cell {
        Data::Empty => return Ok(None),
        Data::Int(value) => value.to_string(),
        Data::Float(value) if value.is_finite() => value.to_string(),
        Data::Float(_) | Data::Error(_) => {
            anyhow::bail!("Spreadsheet contains an invalid or error cell")
        }
        Data::Bool(value) => value.to_string(),
        Data::String(value) | Data::DateTimeIso(value) | Data::DurationIso(value) => value.clone(),
        Data::DateTime(value) => value
            .as_datetime()
            .map(|date| date.to_string())
            .unwrap_or_else(|| value.to_string()),
    };
    ensure!(
        text.len() <= MAX_CELL,
        "Spreadsheet cell exceeds size limit"
    );
    Ok(Some(text))
}

pub(crate) fn read_spreadsheet_batches(
    config: &DataConfig,
    bytes: &[u8],
) -> Result<Vec<RecordBatch>> {
    preflight_archive(bytes)?;
    let range: Range<Data> = match config.file_format {
        DataFileFormat::Xlsx => {
            let mut workbook: Xlsx<_> = open_workbook_from_rs(Cursor::new(bytes))?;
            workbook.with_header_row(HeaderRow::Row(0));
            let names = workbook.sheet_names();
            let name = selected_sheet(&names, &config.sheet_name)?;
            workbook.worksheet_range(name)?
        }
        DataFileFormat::Ods => {
            let mut workbook: Ods<_> = open_workbook_from_rs(Cursor::new(bytes))?;
            workbook.with_header_row(HeaderRow::Row(0));
            let names = workbook.sheet_names();
            let name = selected_sheet(&names, &config.sheet_name)?;
            workbook.worksheet_range(name)?
        }
        _ => anyhow::bail!("Unsupported spreadsheet format"),
    };
    if config.file_format == DataFileFormat::Ods {
        for row in range.rows() {
            for cell in row {
                if let Data::Float(value) = cell {
                    ensure!(
                        value.fract() != 0.0 || value.abs() <= 9_007_199_254_740_991.0,
                        "ODS numeric integer exceeds exact range; store it as text"
                    );
                }
            }
        }
    }
    let (height, width) = range.get_size();
    ensure!(
        width > 0 && width <= MAX_COLUMNS,
        "Spreadsheet column count exceeds limit"
    );
    let header_rows = usize::from(config.csv_header);
    ensure!(
        height >= header_rows && height - header_rows <= MAX_FILE_ROWS,
        "Spreadsheet row count exceeds limit"
    );
    ensure!(
        height
            .checked_mul(width)
            .is_some_and(|cells| cells <= MAX_GRID_CELLS),
        "Spreadsheet grid size exceeds limit"
    );
    let mut rows = range.rows();
    let headings = if config.csv_header {
        let header = rows.next().context("Spreadsheet header row missing")?;
        let mut seen = HashSet::new();
        header
            .iter()
            .map(|cell| {
                let value = cell_text(cell)?.context("Spreadsheet header is empty")?;
                ensure!(
                    !value.trim().is_empty()
                        && value.len() <= 512
                        && seen.insert(value.to_lowercase()),
                    "Spreadsheet header is empty, duplicate or oversized"
                );
                Ok(value)
            })
            .collect::<Result<Vec<_>>>()?
    } else {
        (1..=width)
            .map(|column| format!("column_{column}"))
            .collect()
    };
    let rows = rows.collect::<Vec<_>>();
    let mut kinds = vec![None; width];
    let mut nullable = vec![rows.is_empty(); width];
    for row in &rows {
        for column in 0..width {
            match row.get(column).map(cell_kind).transpose()?.flatten() {
                None => nullable[column] = true,
                Some(kind) => {
                    kinds[column] = Some(match kinds[column] {
                        None => kind,
                        Some(previous) if previous == kind => kind,
                        _ => ColumnKind::Text,
                    });
                }
            }
        }
    }
    let fields = headings
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            let data_type = match kinds[index].unwrap_or(ColumnKind::Text) {
                ColumnKind::Int => DataType::Int64,
                ColumnKind::Bool => DataType::Boolean,
                ColumnKind::Text => DataType::Utf8,
            };
            Field::new(name, data_type, nullable[index])
        })
        .collect::<Vec<_>>();
    let schema = Arc::new(Schema::new(fields));
    let mut batches = Vec::new();
    for chunk in rows.chunks(256) {
        let mut columns = Vec::<ArrayRef>::with_capacity(width);
        for (index, kind) in kinds.iter().enumerate() {
            let array: ArrayRef = match kind.unwrap_or(ColumnKind::Text) {
                ColumnKind::Int => Arc::new(Int64Array::from(
                    chunk
                        .iter()
                        .map(|row| match row.get(index) {
                            Some(Data::Int(value)) => Ok(Some(*value)),
                            Some(Data::Float(value)) if value.is_finite() => {
                                Ok(Some(*value as i64))
                            }
                            Some(Data::Empty) | None => Ok(None),
                            _ => anyhow::bail!("Spreadsheet integer column contains another type"),
                        })
                        .collect::<Result<Vec<_>>>()?,
                )),
                ColumnKind::Bool => Arc::new(BooleanArray::from(
                    chunk
                        .iter()
                        .map(|row| match row.get(index) {
                            Some(Data::Bool(value)) => Ok(Some(*value)),
                            Some(Data::Empty) | None => Ok(None),
                            _ => anyhow::bail!("Spreadsheet boolean column contains another type"),
                        })
                        .collect::<Result<Vec<_>>>()?,
                )),
                ColumnKind::Text => {
                    let mut builder = StringBuilder::new();
                    for row in chunk {
                        match row.get(index).map(cell_text).transpose()?.flatten() {
                            Some(value) => builder.append_value(value),
                            None => builder.append_null(),
                        }
                    }
                    Arc::new(builder.finish())
                }
            };
            columns.push(array);
        }
        batches.push(RecordBatch::try_new(schema.clone(), columns)?);
    }
    if batches.is_empty() {
        batches.push(RecordBatch::new_empty(schema));
    }
    Ok(batches)
}

fn selected_sheet<'a>(names: &'a [String], selected: &'a str) -> Result<&'a str> {
    ensure!(!names.is_empty(), "Spreadsheet has no worksheets");
    if selected.is_empty() {
        Ok(&names[0])
    } else {
        ensure!(
            names.iter().any(|name| name == selected),
            "Spreadsheet worksheet not found"
        );
        Ok(selected)
    }
}
