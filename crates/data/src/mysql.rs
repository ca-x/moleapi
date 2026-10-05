use crate::{Cell, Column, Endpoint, MAX_CELL, MAX_COLUMNS, QueryResult};
use anyhow::{Result, ensure};
use mysql_async::{
    Conn, Opts, OptsBuilder, SslOpts, TxOpts, Value,
    consts::{ColumnFlags, ColumnType},
    prelude::Queryable,
};
use std::time::Instant;
#[derive(Clone)]
pub struct MysqlCancel {
    options: Opts,
    id: u32,
}
impl MysqlCancel {
    pub async fn cancel(&self) -> Result<()> {
        let mut connection = Conn::new(self.options.clone()).await?;
        // ID comes only from the mature driver's authenticated handshake, never user SQL.
        connection
            .query_drop(format!("KILL QUERY {}", self.id))
            .await?;
        connection.disconnect().await?;
        Ok(())
    }
}
pub struct MysqlSource {
    connection: Conn,
    options: Opts,
}
impl MysqlSource {
    pub async fn connect(endpoint: Endpoint) -> Result<Self> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let ssl = if endpoint.tls {
            // Validate explicit PEM using the shared adapter before passing buffers to the SDK.
            endpoint.tls_config()?;
            let mut ssl = SslOpts::default()
                .with_danger_accept_invalid_certs(!endpoint.verify_tls)
                .with_danger_skip_domain_validation(!endpoint.verify_tls);
            if !endpoint.ca_pem.is_empty() {
                ssl = ssl.with_root_certs(vec![endpoint.ca_pem.as_bytes().to_vec().into()]);
            }
            Some(ssl)
        } else {
            None
        };
        let options: Opts = OptsBuilder::default()
            .ip_or_hostname(endpoint.host)
            .tcp_port(endpoint.port)
            .resolved_ips(Some(
                endpoint
                    .addresses
                    .iter()
                    .map(|a| a.ip())
                    .collect::<Vec<_>>(),
            ))
            .prefer_socket(false)
            .socket(None::<String>)
            .compression(None)
            .user(Some(endpoint.username))
            .pass(Some(endpoint.password))
            .db_name(Some(endpoint.database))
            .max_allowed_packet(Some(1024 * 1024))
            .ssl_opts(ssl)
            .into();
        let connection = Conn::new(options.clone()).await?;
        Ok(Self {
            connection,
            options,
        })
    }
    pub fn is_closed(&self) -> bool {
        self.connection.is_disconnected()
    }
    pub fn cancellation(&self) -> MysqlCancel {
        MysqlCancel {
            options: self.options.clone(),
            id: self.connection.id(),
        }
    }
    pub async fn query(&mut self, sql: &str, read_only: bool) -> Result<QueryResult> {
        crate::statement(sql, moleapi_core::DataSource::Mysql, read_only)?;
        let started = Instant::now();
        let mut transaction_options = TxOpts::default();
        transaction_options.with_readonly(read_only);
        let mut transaction = self
            .connection
            .start_transaction(transaction_options)
            .await?;
        let result = collect(&mut transaction, sql, crate::MAX_ROWS, crate::MAX_RESULT).await;
        match &result {
            Ok(value) if !value.truncated => transaction.commit().await?,
            Ok(_) => {
                // Draining a capped stream can block or surface KILL errors and discard
                // captured rows. Dispose its transport synchronously; do not claim rollback.
                drop(transaction);
                self.connection.abort();
            }
            Err(_) => {
                drop(transaction);
                self.connection.abort();
            }
        }
        let mut result = result?;
        result.elapsed_ms = started.elapsed().as_millis() as u64;
        Ok(result)
    }
    pub async fn metadata(&mut self, sql: &str) -> Result<QueryResult> {
        // Schema is bounded independently. A capped metadata stream must not
        // leave pending rows on the user's reusable query connection.
        let mut metadata = Conn::new(self.options.clone()).await?;
        let result = collect(&mut metadata, sql, crate::MAX_SCHEMA_COLUMNS, 1024 * 1024).await;
        metadata.abort();
        result
    }
}
async fn collect<Q: Queryable + Send>(
    connection: &mut Q,
    sql: &str,
    row_limit: usize,
    byte_limit: usize,
) -> Result<QueryResult> {
    let mut stream = connection.query_iter(sql).await?;
    let columns = stream.columns_ref().to_vec();
    ensure!(
        columns.len() <= MAX_COLUMNS,
        "Data result column limit exceeded"
    );
    let mut result = QueryResult::empty();
    result.columns = columns
        .iter()
        .map(|c| Column {
            name: c.name_str().into_owned(),
            data_type: format!("{:?}", c.column_type())
                .trim_start_matches("MYSQL_TYPE_")
                .into(),
            nullable: !c.flags().contains(ColumnFlags::NOT_NULL_FLAG),
        })
        .collect();
    let mut size = 0usize;
    while let Some(row) = stream.next().await? {
        if result.rows.len() >= row_limit {
            result.cap("Data row limit reached");
            break;
        }
        let values = row
            .unwrap()
            .into_iter()
            .zip(&columns)
            .map(|(value, column)| mysql_cell(value, column))
            .collect::<Result<Vec<_>>>()?;
        let bytes = serde_json::to_vec(&values)?.len();
        if bytes > 512 * 1024 || size.saturating_add(bytes) > byte_limit {
            result.cap("Data returned byte/row-size limit reached");
            break;
        }
        size += bytes;
        result.rows.push(values);
    }
    result.rows_affected = stream.affected_rows();
    // Pending rows on a capped query are not silently treated as a complete result.
    // The caller requests cancellation before rollback/connection disposal.
    drop(stream);
    Ok(result)
}
fn mysql_cell(value: Value, column: &mysql_async::Column) -> Result<Cell> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    Ok(match value {
        Value::NULL => Cell::Null,
        Value::Int(value) => Cell::Integer {
            value: value.to_string(),
        },
        Value::UInt(value) => Cell::Integer {
            value: value.to_string(),
        },
        Value::Float(value) => Cell::Decimal {
            value: value.to_string(),
        },
        Value::Double(value) => Cell::Decimal {
            value: value.to_string(),
        },
        Value::Bytes(bytes) => {
            let kind = column.column_type();
            let is_numeric = kind.is_numeric_type();
            let is_decimal = matches!(
                kind,
                ColumnType::MYSQL_TYPE_DECIMAL
                    | ColumnType::MYSQL_TYPE_NEWDECIMAL
                    | ColumnType::MYSQL_TYPE_FLOAT
                    | ColumnType::MYSQL_TYPE_DOUBLE
            );
            // TextProtocol deliberately returns Bytes for numeric/temporal cells.
            // Charset 63 by itself cannot distinguish a BIGINT or DATE from a blob.
            let is_binary = column.character_set() == 63
                && matches!(
                    kind,
                    ColumnType::MYSQL_TYPE_VARCHAR
                        | ColumnType::MYSQL_TYPE_VAR_STRING
                        | ColumnType::MYSQL_TYPE_STRING
                        | ColumnType::MYSQL_TYPE_TINY_BLOB
                        | ColumnType::MYSQL_TYPE_MEDIUM_BLOB
                        | ColumnType::MYSQL_TYPE_LONG_BLOB
                        | ColumnType::MYSQL_TYPE_BLOB
                        | ColumnType::MYSQL_TYPE_GEOMETRY
                        | ColumnType::MYSQL_TYPE_BIT
                        | ColumnType::MYSQL_TYPE_VECTOR
                );
            if bytes.len() > MAX_CELL {
                Cell::Truncated {
                    preview: if is_binary {
                        STANDARD.encode(&bytes[..MAX_CELL])
                    } else {
                        String::from_utf8_lossy(&bytes[..MAX_CELL]).into_owned()
                    },
                    bytes: bytes.len(),
                }
            } else if is_binary || std::str::from_utf8(&bytes).is_err() {
                Cell::Binary {
                    base64: STANDARD.encode(&bytes),
                    bytes: bytes.len(),
                }
            } else {
                let value = String::from_utf8(bytes)?;
                if is_decimal {
                    Cell::Decimal { value }
                } else if is_numeric {
                    Cell::Integer { value }
                } else {
                    Cell::Text { value }
                }
            }
        }
        Value::Date(y, m, d, h, minute, s, micro) => Cell::Text {
            value: format!("{y:04}-{m:02}-{d:02} {h:02}:{minute:02}:{s:02}.{micro:06}"),
        },
        Value::Time(negative, days, h, m, s, micro) => Cell::Text {
            value: format!(
                "{}{hours:02}:{m:02}:{s:02}.{micro:06}",
                if negative { "-" } else { "" },
                hours = days as u64 * 24 + h as u64
            ),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mysql_text_protocol_uses_column_types_for_numbers_dates_and_binary() {
        let column = |kind| mysql_async::Column::new(kind).with_character_set(63);
        for (kind, text, expected) in [
            (
                ColumnType::MYSQL_TYPE_LONGLONG,
                "9007199254740993",
                Cell::Integer {
                    value: "9007199254740993".into(),
                },
            ),
            (
                ColumnType::MYSQL_TYPE_NEWDECIMAL,
                "1.23000000000000000001",
                Cell::Decimal {
                    value: "1.23000000000000000001".into(),
                },
            ),
            (
                ColumnType::MYSQL_TYPE_DOUBLE,
                "1.25",
                Cell::Decimal {
                    value: "1.25".into(),
                },
            ),
            (
                ColumnType::MYSQL_TYPE_DATE,
                "2026-10-05",
                Cell::Text {
                    value: "2026-10-05".into(),
                },
            ),
        ] {
            assert_eq!(
                mysql_cell(Value::Bytes(text.as_bytes().to_vec()), &column(kind)).unwrap(),
                expected
            );
        }
        assert_eq!(
            mysql_cell(
                Value::Bytes(vec![0, 255, 65]),
                &column(ColumnType::MYSQL_TYPE_VAR_STRING)
            )
            .unwrap(),
            Cell::Binary {
                base64: "AP9B".into(),
                bytes: 3
            }
        );
    }
}
