use crate::{Cell, FileSource};
use moleapi_core::{DataConfig, DataFileFormat, DataSource};
fn config(format: DataFileFormat) -> DataConfig {
    DataConfig {
        source: DataSource::LocalFile,
        file_format: format,
        ..Default::default()
    }
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
