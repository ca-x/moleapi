use crate::{Cell, FileSource};
use moleapi_core::{DataConfig, DataFileFormat, DataSource};
fn config(format: DataFileFormat) -> DataConfig {
    DataConfig {
        source: DataSource::LocalFile,
        file_format: format,
        ..Default::default()
    }
}

const XLSX: &[u8] = include_bytes!("../tests/fixtures/workbook.xlsx");
const ODS: &[u8] = include_bytes!("../tests/fixtures/workbook.ods");
const XLS: &[u8] = include_bytes!("../tests/fixtures/workbook.xls");
const XLSB_ANY: &[u8] = include_bytes!("../tests/fixtures/calamine-any-sheets.xlsb");
const XLSB_ISSUES: &[u8] = include_bytes!("../tests/fixtures/calamine-issues.xlsb");

#[tokio::test]
async fn xlsb_first_and_named_sheets_run_sql_with_typed_and_mixed_values() {
    let mut config = config(DataFileFormat::Xlsb);
    config.csv_header = false;
    let first = FileSource::open(&config, XLSB_ANY).unwrap();
    let rows = first
        .query("SELECT column_1, column_2 FROM data WHERE column_1 = '3'")
        .await
        .unwrap();
    assert_eq!(rows.rows[0][0], Cell::Text { value: "3".into() });
    assert_eq!(rows.rows[0][1], Cell::Integer { value: "4".into() });

    config.sheet_name = "issue2".into();
    let named = FileSource::open(&config, XLSB_ISSUES).unwrap();
    let rows = named
        .query("SELECT column_1, column_2 FROM data WHERE column_1 = 2")
        .await
        .unwrap();
    assert_eq!(rows.rows[0][0], Cell::Integer { value: "2".into() });
    assert_eq!(rows.rows[0][1], Cell::Text { value: "b".into() });

    config.sheet_name = "datatypes".into();
    let mixed = FileSource::open(&config, XLSB_ISSUES).unwrap();
    let rows = mixed
        .query("SELECT column_1 FROM data LIMIT 4")
        .await
        .unwrap();
    assert_eq!(rows.rows[0][0], Cell::Text { value: "1".into() });
    assert_eq!(
        rows.rows[1][0],
        Cell::Text {
            value: "1.5".into()
        }
    );
    assert_eq!(rows.rows[2][0], Cell::Text { value: "ab".into() });
    assert_eq!(
        rows.rows[3][0],
        Cell::Text {
            value: "false".into()
        }
    );
}

#[test]
fn xlsb_rejects_missing_empty_and_malformed_sheets_or_zip() {
    let mut config = config(DataFileFormat::Xlsb);
    let headers = FileSource::open(&config, XLSB_ANY).unwrap();
    assert_eq!(headers.schema().columns[0].name, "1");
    config.sheet_name = "Missing".into();
    assert!(FileSource::open(&config, XLSB_ANY).is_err());
    config.sheet_name = "Hidden".into();
    assert!(FileSource::open(&config, XLSB_ANY).is_err());
    config.sheet_name = "Visible".into();
    assert!(FileSource::open(&config, b"not an XLSB workbook").is_err());
    let oversized = include_bytes!("../tests/fixtures/oversized-entry.xlsx");
    let error = FileSource::open(&config, oversized)
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("expanded size"), "{error}");
}

#[tokio::test]
async fn xls_selected_sheet_runs_real_sql_with_safe_types() {
    let mut config = config(DataFileFormat::Xls);
    let first = FileSource::open(&config, XLS).unwrap();
    assert_eq!(first.schema().columns[0].name, "marker");
    config.sheet_name = "Transactions".into();
    let source = FileSource::open(&config, XLS).unwrap();
    let result = source
        .query("SELECT id, amount, active, note, exact_text, formula FROM data ORDER BY id")
        .await
        .unwrap();
    assert_eq!(result.rows[0][0], Cell::Integer { value: "2".into() });
    assert_eq!(
        result.rows[0][1],
        Cell::Text {
            value: "2.3".into()
        }
    );
    assert_eq!(result.rows[0][2], Cell::Bool { value: true });
    assert_eq!(
        result.rows[0][3],
        Cell::Text {
            value: "café".into()
        }
    );
    assert_eq!(
        result.rows[0][4],
        Cell::Text {
            value: "9007199254740993".into()
        }
    );
    assert_eq!(result.rows[0][5], Cell::Text { value: "".into() }); // xlwt leaves the formula cache empty.
    assert_eq!(result.rows[1][1], Cell::Null);
    assert_eq!(result.rows[1][2], Cell::Bool { value: false });
    let cast = source
        .query("SELECT CAST(amount AS DECIMAL(10,2)) FROM data WHERE id = 2")
        .await
        .unwrap();
    assert_eq!(
        cast.rows[0][0],
        Cell::Decimal {
            value: "2.30".into()
        }
    );
}

#[test]
fn xls_rejects_bad_sheets_headers_and_unsafe_numbers() {
    let mut config = config(DataFileFormat::Xls);
    config.sheet_name = "Missing".into();
    assert!(FileSource::open(&config, XLS).is_err());
    config.sheet_name = "Duplicate Headers".into();
    assert!(FileSource::open(&config, XLS).is_err());
    config.sheet_name = "Blank Headers".into();
    assert!(FileSource::open(&config, XLS).is_err());
    config.sheet_name = "Unsafe Number".into();
    let error = FileSource::open(&config, XLS).err().unwrap().to_string();
    assert!(error.contains("exact range"), "{error}");
    config.sheet_name = "Unsafe Date".into();
    let error = FileSource::open(&config, XLS).err().unwrap().to_string();
    assert!(error.contains("exact range"), "{error}");
    config.sheet_name = "Leading Blank".into();
    let error = FileSource::open(&config, XLS).err().unwrap().to_string();
    assert!(error.contains("header is empty"), "{error}");
    config.sheet_name = "Transactions".into();
    assert!(FileSource::open(&config, b"not an XLS workbook").is_err());
    config.csv_header = false;
    let no_header = FileSource::open(&config, XLS).unwrap();
    assert_eq!(no_header.schema().columns[0].name, "column_1");
}

#[test]
fn xls_rejects_an_oversized_unselected_sheet_before_parsing() {
    let bytes = include_bytes!("../tests/fixtures/oversized-grid.xls");
    let config = config(DataFileFormat::Xls);
    let error = FileSource::open(&config, bytes).err().unwrap().to_string();
    assert!(error.contains("grid size"), "{error}");
}

#[test]
fn xls_rejects_aggregate_unselected_sheet_expansion() {
    let bytes = include_bytes!("../tests/fixtures/aggregate-grid.xls");
    let mut config = config(DataFileFormat::Xls);
    config.csv_header = false;
    let error = FileSource::open(&config, bytes).err().unwrap().to_string();
    assert!(error.contains("workbook grid size"), "{error}");
}

#[tokio::test]
async fn ods_selected_sheet_runs_sql_with_cached_values_and_safe_types() {
    let mut config = config(DataFileFormat::Ods);
    let first = FileSource::open(&config, ODS).unwrap();
    assert_eq!(first.schema().columns[0].name, "marker");
    config.sheet_name = "Transactions".into();
    let source = FileSource::open(&config, ODS).unwrap();
    let result = source
        .query("SELECT id, amount, active, note, formula FROM data ORDER BY id")
        .await
        .unwrap();
    assert_eq!(result.rows[0][0], Cell::Integer { value: "2".into() });
    assert_eq!(
        result.rows[0][1],
        Cell::Text {
            value: "2.3".into()
        }
    );
    assert_eq!(result.rows[0][2], Cell::Bool { value: true });
    assert_eq!(
        result.rows[0][3],
        Cell::Text {
            value: "café".into()
        }
    );
    assert_eq!(
        result.rows[0][4],
        Cell::Text {
            value: "4.6".into()
        }
    );
    assert_eq!(result.rows[1][1], Cell::Null);
    assert_eq!(result.rows[1][2], Cell::Bool { value: false });
    config.sheet_name = "Exact Text".into();
    let exact = FileSource::open(&config, ODS).unwrap();
    let value = exact.query("SELECT value FROM data").await.unwrap();
    assert_eq!(
        value.rows[0][0],
        Cell::Text {
            value: "9007199254740993".into()
        }
    );
}

#[test]
fn ods_rejects_missing_sheet_bad_headers_unsafe_number_and_invalid_zip() {
    let mut config = config(DataFileFormat::Ods);
    config.sheet_name = "Missing".into();
    assert!(FileSource::open(&config, ODS).is_err());
    config.sheet_name = "Duplicate Headers".into();
    assert!(FileSource::open(&config, ODS).is_err());
    config.sheet_name = "Blank Headers".into();
    assert!(FileSource::open(&config, ODS).is_err());
    config.sheet_name = "Unsafe Number".into();
    let error = FileSource::open(&config, ODS).err().unwrap().to_string();
    assert!(error.contains("exact range"), "{error}");
    config.sheet_name = "Transactions".into();
    assert!(FileSource::open(&config, b"not an ODS workbook").is_err());
    let invalid_mimetype = include_bytes!("../tests/fixtures/invalid-mimetype.ods");
    assert!(FileSource::open(&config, invalid_mimetype).is_err());
    let oversized = include_bytes!("../tests/fixtures/oversized-entry.xlsx");
    let error = FileSource::open(&config, oversized)
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("expanded size"), "{error}");
}

#[tokio::test]
async fn ods_header_uses_physical_first_row_and_can_be_disabled() {
    let mut config = config(DataFileFormat::Ods);
    config.sheet_name = "Leading Blank".into();
    let error = FileSource::open(&config, ODS).err().unwrap().to_string();
    assert!(error.contains("header is empty"), "{error}");
    config.csv_header = false;
    let source = FileSource::open(&config, ODS).unwrap();
    assert_eq!(source.schema().columns[0].name, "column_1");
    let result = source.query("SELECT COUNT(*) FROM data").await.unwrap();
    assert_eq!(result.rows[0][0], Cell::Integer { value: "3".into() });
}

#[tokio::test]
async fn xlsx_selected_sheet_preserves_types_nulls_and_cached_formula() {
    let mut config = config(DataFileFormat::Xlsx);
    config.sheet_name = "Transactions".into();
    let source = FileSource::open(&config, XLSX).unwrap();
    assert_eq!(
        source
            .schema()
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        [
            "id",
            "amount",
            "active",
            "label",
            "exact_text",
            "calculated",
            "when"
        ]
    );
    let result = source
        .query("SELECT id, amount, active, label, exact_text, calculated FROM data ORDER BY id")
        .await
        .unwrap();
    assert_eq!(result.rows[0][0], Cell::Integer { value: "1".into() });
    assert_eq!(
        result.rows[0][1],
        Cell::Text {
            value: "1.23".into()
        }
    );
    assert_eq!(result.rows[0][2], Cell::Bool { value: true });
    assert_eq!(
        result.rows[0][4],
        Cell::Text {
            value: "9007199254740993".into()
        }
    );
    assert_eq!(
        result.rows[0][5],
        Cell::Text {
            value: "2.46".into()
        }
    );
    let date = source
        .query("SELECT \"when\" FROM data WHERE id = 1")
        .await
        .unwrap();
    assert_eq!(
        date.rows[0][0],
        Cell::Text {
            value: "2024-02-29 00:00:00".into()
        }
    );
    assert_eq!(result.rows[1][3], Cell::Null);
    assert_eq!(
        result.rows[1][4],
        Cell::Text {
            value: "1.23000000000000000001".into()
        }
    );
    let cast = source
        .query("SELECT CAST(amount AS DECIMAL(10,2)) FROM data WHERE id = 1")
        .await
        .unwrap();
    assert_eq!(
        cast.rows[0][0],
        Cell::Decimal {
            value: "1.23".into()
        }
    );
}

#[test]
fn xlsx_sheet_selection_headers_and_archive_limits_are_explicit() {
    let mut config = config(DataFileFormat::Xlsx);
    let first = FileSource::open(&config, XLSX).unwrap();
    assert_eq!(first.schema().columns[0].name, "marker");
    config.sheet_name = "Missing".into();
    assert!(FileSource::open(&config, XLSX).is_err());
    config.sheet_name = "Duplicate Headers".into();
    assert!(FileSource::open(&config, XLSX).is_err());
    config.sheet_name = "Case Colliding Headers".into();
    assert!(FileSource::open(&config, XLSX).is_err());
    config.sheet_name = "Blank Headers".into();
    assert!(FileSource::open(&config, XLSX).is_err());
    config.sheet_name = "Error Cells".into();
    let error = FileSource::open(&config, XLSX).err().unwrap().to_string();
    assert!(error.contains("invalid or error cell"), "{error}");
    config.sheet_name = "Duplicate Headers".into();
    config.csv_header = false;
    let no_header = FileSource::open(&config, XLSX).unwrap();
    assert_eq!(no_header.schema().columns[0].name, "column_1");
    assert!(FileSource::open(&config, b"not a workbook").is_err());
    let oversized = include_bytes!("../tests/fixtures/oversized-entry.xlsx");
    let error = FileSource::open(&config, oversized)
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("expanded size"), "{error}");
}

#[tokio::test]
async fn xlsx_header_means_physical_first_row_and_leading_empty_rows_are_retained() {
    let mut config = config(DataFileFormat::Xlsx);
    config.sheet_name = "Leading Blank".into();
    let error = FileSource::open(&config, XLSX).err().unwrap().to_string();
    assert!(error.contains("header is empty"), "{error}");
    config.csv_header = false;
    let source = FileSource::open(&config, XLSX).unwrap();
    let result = source.query("SELECT COUNT(*) FROM data").await.unwrap();
    assert_eq!(result.rows[0][0], Cell::Integer { value: "3".into() });
}
#[tokio::test]
async fn csv_real_filter_aggregation_decimal_cast_and_schema() {
    let source = FileSource::open(
        &config(DataFileFormat::Csv),
        b"id,name,amount\n1,first,1.23000000000000000001\n2,second,4.50\n3,third,2.00\n",
    )
    .unwrap();
    assert_eq!(
        source
            .schema()
            .columns
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        ["id", "name", "amount"]
    );
    let result = source
        .query("SELECT name, CAST(amount AS DECIMAL(38,20)) AS amount FROM data WHERE id = 1")
        .await
        .unwrap();
    assert_eq!(
        result.rows[0][0],
        Cell::Text {
            value: "first".into()
        }
    );
    assert_eq!(
        result.rows[0][1],
        Cell::Decimal {
            value: "1.23000000000000000001".into()
        }
    );
    let aggregate = source
        .query("SELECT SUM(id) AS total FROM data WHERE id >= 2")
        .await
        .unwrap();
    assert_eq!(aggregate.rows[0][0], Cell::Integer { value: "5".into() });
    assert!(
        source
            .query("CREATE EXTERNAL TABLE stolen STORED AS CSV LOCATION '/etc/passwd'")
            .await
            .is_err()
    );
    assert!(
        source
            .query("SELECT * FROM read_csv('/etc/passwd')")
            .await
            .is_err()
    );
    assert!(source.query("UPDATE data SET id=4").await.is_err());
}
#[tokio::test]
async fn json_preserves_null_large_integer_and_caps_rows() {
    let source = FileSource::open(
        &config(DataFileFormat::Json),
        br#"[{"n":9007199254740993,"text":"first"},{"n":null,"text":"second"}]"#,
    )
    .unwrap();
    let result = source
        .query("SELECT n, text FROM data ORDER BY text")
        .await
        .unwrap();
    assert_eq!(result.columns.len(), 2);
    assert_ne!(result.columns[0].name, result.columns[1].name);
    assert!(
        source
            .query("SELECT n AS same, text AS same FROM data")
            .await
            .is_err()
    );
    assert_eq!(
        result.rows[0][0],
        Cell::Integer {
            value: "9007199254740993".into()
        }
    );
    assert_eq!(result.rows[1][0], Cell::Null);
    let result = source
        .query("SELECT a.text FROM data a CROSS JOIN generate_series(1,2000)")
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 1000);
    assert!(result.truncated);
}
#[tokio::test]
async fn actual_parquet_binary_schema_and_rows_are_decoded_by_arrow() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use datafusion::{
        arrow::{
            array::{BinaryArray, Int64Array},
            datatypes::{DataType, Field, Schema},
            record_batch::RecordBatch,
        },
        parquet::arrow::ArrowWriter,
    };
    use std::sync::Arc;
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int64, false),
        Field::new("bytes", DataType::Binary, true),
    ]));
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(Int64Array::from(vec![1, 2])),
            Arc::new(BinaryArray::from(vec![Some(&[0, 255, 65][..]), None])),
        ],
    )
    .unwrap();
    let mut bytes = Vec::new();
    let mut writer = ArrowWriter::try_new(&mut bytes, schema, None).unwrap();
    writer.write(&batch).unwrap();
    writer.close().unwrap();
    let source = FileSource::open(&config(DataFileFormat::Parquet), &bytes).unwrap();
    let result = source
        .query("SELECT bytes FROM data ORDER BY id")
        .await
        .unwrap();
    assert_eq!(
        result.rows[0][0],
        Cell::Binary {
            base64: STANDARD.encode([0, 255, 65]),
            bytes: 3
        }
    );
    assert_eq!(result.rows[1][0], Cell::Null);
}

#[tokio::test]
async fn json_decimal_and_mixed_numeric_lexemes_remain_exact() {
    let source = FileSource::open(
        &config(DataFileFormat::Json),
        br#"[{"amount":1.23000000000000000001,"mixed":9007199254740993},{"amount":null,"mixed":1.5}]"#,
    ).unwrap();
    let result = source
        .query("SELECT amount, mixed FROM data WHERE amount IS NOT NULL")
        .await
        .unwrap();
    assert_eq!(
        result.rows[0][0],
        Cell::Text {
            value: "1.23000000000000000001".into()
        }
    );
    assert_eq!(
        result.rows[0][1],
        Cell::Text {
            value: "9007199254740993".into()
        }
    );
    let result = source
        .query("SELECT CAST(amount AS DECIMAL(38,20)) FROM data WHERE amount IS NOT NULL")
        .await
        .unwrap();
    assert_eq!(
        result.rows[0][0],
        Cell::Decimal {
            value: "1.23000000000000000001".into()
        }
    );
}
