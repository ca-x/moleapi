use crate::{Cell, Column, Endpoint, MAX_CELL, MAX_COLUMNS, QueryResult};
use anyhow::{Result, ensure};
use futures_util::StreamExt;
use std::time::Instant;
use tokio_postgres::{
    Client, Config, NoTls, SimpleQueryMessage, config::SslMode, tls::MakeTlsConnect,
};
use tokio_postgres_rustls::MakeRustlsConnect;
#[derive(Clone)]
pub struct PgCancel {
    token: tokio_postgres::CancelToken,
    endpoint: Endpoint,
}
impl PgCancel {
    pub async fn cancel(&self) -> Result<()> {
        let stream = tokio::net::TcpStream::connect(self.endpoint.addresses.as_slice()).await?;
        let stream = crate::io::BudgetSocket::new(stream);
        if self.endpoint.tls {
            let mut factory = MakeRustlsConnect::new(self.endpoint.tls_config()?);
            let tls =
                <MakeRustlsConnect as MakeTlsConnect<crate::io::BudgetSocket>>::make_tls_connect(
                    &mut factory,
                    &self.endpoint.host,
                )?;
            self.token.cancel_query_raw(stream, tls).await?;
        } else {
            self.token.cancel_query_raw(stream, NoTls).await?;
        }
        Ok(())
    }
}
pub struct PgSource {
    client: Client,
    task: tokio::task::JoinHandle<()>,
    cancel: PgCancel,
    closed: bool,
}
impl Drop for PgSource {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl PgSource {
    pub async fn connect(endpoint: Endpoint) -> Result<Self> {
        let mut config = Config::new();
        config
            .user(&endpoint.username)
            .password(&endpoint.password)
            .dbname(&endpoint.database)
            .host(&endpoint.host)
            .port(endpoint.port)
            .ssl_mode(if endpoint.tls {
                SslMode::Require
            } else {
                SslMode::Disable
            });
        let stream = tokio::net::TcpStream::connect(endpoint.addresses.as_slice()).await?;
        stream.set_nodelay(true)?;
        let stream = crate::io::BudgetSocket::new(stream);
        let (client, task) = if endpoint.tls {
            let mut factory = MakeRustlsConnect::new(endpoint.tls_config()?);
            let tls =
                <MakeRustlsConnect as MakeTlsConnect<crate::io::BudgetSocket>>::make_tls_connect(
                    &mut factory,
                    &endpoint.host,
                )?;
            let (client, connection) = config.connect_raw(stream, tls).await?;
            (
                client,
                tokio::spawn(async move {
                    let _ = connection.await;
                }),
            )
        } else {
            let (client, connection) = config.connect_raw(stream, NoTls).await?;
            (
                client,
                tokio::spawn(async move {
                    let _ = connection.await;
                }),
            )
        };
        let cancel = PgCancel {
            token: client.cancel_token(),
            endpoint,
        };
        Ok(Self {
            client,
            task,
            cancel,
            closed: false,
        })
    }
    pub fn cancellation(&self) -> PgCancel {
        self.cancel.clone()
    }
    pub fn is_closed(&self) -> bool {
        self.closed || self.client.is_closed()
    }
    pub async fn query(&mut self, sql: &str, read_only: bool) -> Result<QueryResult> {
        crate::statement(sql, moleapi_core::DataSource::Postgresql, read_only)?;
        let started = Instant::now();
        self.client
            .batch_execute(if read_only {
                "BEGIN READ ONLY"
            } else {
                "BEGIN"
            })
            .await?;
        let result = self.collect(sql, crate::MAX_ROWS, crate::MAX_RESULT).await;
        match &result {
            Ok(value) if !value.truncated => self.client.batch_execute("COMMIT").await?,
            Ok(_) => {
                // Preserve captured rows without waiting for a pending result drain.
                // Abort the SDK transport; the session reports closure without claiming
                // that any write has definitely rolled back.
                self.closed = true;
                self.task.abort();
            }
            Err(_) => self.client.batch_execute("ROLLBACK").await?,
        }
        let mut result = result?;
        result.elapsed_ms = started.elapsed().as_millis() as u64;
        Ok(result)
    }
    pub async fn metadata(&self, sql: &str) -> Result<QueryResult> {
        // Use the same checked endpoint on an independently disposable SDK
        // transport so truncated schema rows cannot poison the query connection.
        let metadata = Self::connect(self.cancel.endpoint.clone()).await?;
        metadata
            .collect(sql, crate::MAX_SCHEMA_COLUMNS, 1024 * 1024)
            .await
    }
    async fn collect(&self, sql: &str, row_limit: usize, byte_limit: usize) -> Result<QueryResult> {
        let statement = self.client.prepare(sql).await?;
        ensure!(
            statement.columns().len() <= MAX_COLUMNS,
            "Data result column limit exceeded"
        );
        let types = statement
            .columns()
            .iter()
            .map(|c| c.type_().clone())
            .collect::<Vec<_>>();
        let mut result = QueryResult::empty();
        result.columns = statement
            .columns()
            .iter()
            .map(|c| Column {
                name: c.name().into(),
                data_type: c.type_().name().into(),
                nullable: true,
            })
            .collect();
        let stream = self.client.simple_query_raw(sql).await?;
        futures_util::pin_mut!(stream);
        let mut size = 0usize;
        while let Some(message) = stream.next().await {
            match message? {
                SimpleQueryMessage::Row(row) => {
                    if result.rows.len() >= row_limit {
                        result.cap("Data row limit reached");
                        break;
                    }
                    let mut values = Vec::new();
                    for (i, kind) in types.iter().enumerate() {
                        values.push(pg_cell(row.try_get(i)?, kind)?);
                    }
                    let bytes = serde_json::to_vec(&values)?.len();
                    if bytes > 512 * 1024 || size.saturating_add(bytes) > byte_limit {
                        result.cap("Data returned byte/row-size limit reached");
                        break;
                    }
                    size += bytes;
                    result.rows.push(values);
                }
                SimpleQueryMessage::CommandComplete(n) => result.rows_affected = n,
                SimpleQueryMessage::RowDescription(_) => {}
                _ => {}
            }
        }
        Ok(result)
    }
}
fn pg_cell(value: Option<&str>, kind: &tokio_postgres::types::Type) -> Result<Cell> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use tokio_postgres::types::Type;
    let Some(value) = value else {
        return Ok(Cell::Null);
    };
    if value.len() > MAX_CELL {
        return Ok(Cell::Truncated {
            preview: value[..value.floor_char_boundary(MAX_CELL)].into(),
            bytes: value.len(),
        });
    }
    Ok(match *kind {
        Type::BOOL => Cell::Bool {
            value: value == "t" || value == "true",
        },
        Type::INT2 | Type::INT4 | Type::INT8 | Type::OID => Cell::Integer {
            value: value.into(),
        },
        Type::FLOAT4 | Type::FLOAT8 | Type::NUMERIC => Cell::Decimal {
            value: value.into(),
        },
        Type::BYTEA => {
            let bytes =
                hex::decode(value.strip_prefix("\\x").ok_or_else(|| {
                    anyhow::anyhow!("Unsupported PostgreSQL bytea text encoding")
                })?)?;
            Cell::Binary {
                base64: STANDARD.encode(&bytes),
                bytes: bytes.len(),
            }
        }
        _ => Cell::Text {
            value: value.into(),
        },
    })
}
