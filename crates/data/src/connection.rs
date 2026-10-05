use crate::{
    Endpoint, MYSQL_SCHEMA_SQL, MysqlCancel, MysqlSource, PG_SCHEMA_SQL, PgCancel, PgSource,
    QueryResult, SchemaTable, run_file_worker, schema_tables,
};
use anyhow::{Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use moleapi_core::{DataConfig, DataSource, NetworkPolicy, Protocol, RequestSpec};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub source: DataSource,
    pub public_url: String,
    pub tls: bool,
    pub tls_verified: bool,
    pub file_bytes: Option<usize>,
}
pub enum Cancellation {
    Pg(Box<PgCancel>),
    Mysql(MysqlCancel),
    File,
}
impl Cancellation {
    pub async fn request(&self) -> Result<()> {
        match self {
            Self::Pg(token) => token.cancel().await,
            Self::Mysql(token) => token.cancel().await,
            Self::File => Ok(()),
        }
    }
}
enum Backend {
    Pg(Box<PgSource>),
    Mysql(Box<MysqlSource>),
    File {
        config: Box<DataConfig>,
        bytes: Vec<u8>,
        worker: PathBuf,
        schema: Box<SchemaTable>,
    },
}
pub struct Connection {
    backend: Backend,
    info: ConnectionInfo,
}
impl Connection {
    pub async fn open(
        request: &RequestSpec,
        config: &DataConfig,
        policy: NetworkPolicy,
        worker: &Path,
    ) -> Result<Self> {
        moleapi_core::validate_data_config(config)?;
        let (backend, info) = match config.source {
            DataSource::Postgresql | DataSource::Mysql => {
                let endpoint = Endpoint::checked(request, config, policy).await?;
                let info = ConnectionInfo {
                    source: config.source,
                    public_url: endpoint.public_url.clone(),
                    tls: endpoint.tls,
                    tls_verified: endpoint.tls && endpoint.verify_tls,
                    file_bytes: None,
                };
                let backend = if config.source == DataSource::Postgresql {
                    Backend::Pg(Box::new(PgSource::connect(endpoint).await?))
                } else {
                    Backend::Mysql(Box::new(MysqlSource::connect(endpoint).await?))
                };
                (backend, info)
            }
            DataSource::LocalFile | DataSource::RemoteFile => {
                let bytes = if config.source == DataSource::LocalFile {
                    ensure!(
                        !config.file_base64.is_empty(),
                        "Select a local Data file first"
                    );
                    STANDARD.decode(&config.file_base64)?
                } else {
                    moleapi_core::data_url(&request.url, DataSource::RemoteFile)?;
                    let mut fetch = request.clone();
                    fetch.protocol = Protocol::Http;
                    fetch.method = "GET".into();
                    fetch.body_kind = "none".into();
                    fetch.body.clear();
                    if !fetch
                        .headers
                        .iter()
                        .any(|h| h.enabled && h.key.eq_ignore_ascii_case("accept-encoding"))
                    {
                        fetch.headers.push(moleapi_core::Pair {
                            id: uuid::Uuid::new_v4().to_string(),
                            key: "Accept-Encoding".into(),
                            value: "identity".into(),
                            enabled: true,
                            secret: None,
                            local_value: None,
                        });
                    }
                    let response = moleapi_core::execute(&fetch, None, policy).await?;
                    ensure!(
                        (200..300).contains(&response.status),
                        "Remote Data file returned HTTP {}",
                        response.status
                    );
                    ensure!(!response.truncated, "Remote Data file exceeds 5 MiB");
                    match response.body_base64 {
                        Some(value) => STANDARD.decode(value)?,
                        None => response.body.into_bytes(),
                    }
                };
                let output = run_file_worker(
                    worker,
                    config,
                    &bytes,
                    None,
                    Duration::from_millis(request.timeout_ms),
                )
                .await?;
                let mut saved = config.clone();
                saved.file_base64.clear();
                let info = ConnectionInfo {
                    source: config.source,
                    public_url: if config.source == DataSource::LocalFile {
                        config.file_name.clone()
                    } else {
                        request.url.clone()
                    },
                    tls: config.source == DataSource::RemoteFile
                        && request.url.starts_with("https://"),
                    tls_verified: config.source == DataSource::RemoteFile
                        && request.url.starts_with("https://")
                        && request.verify_tls,
                    file_bytes: Some(bytes.len()),
                };
                (
                    Backend::File {
                        config: Box::new(saved),
                        bytes,
                        worker: worker.to_owned(),
                        schema: Box::new(output.schema),
                    },
                    info,
                )
            }
        };
        Ok(Self { backend, info })
    }
    pub fn is_closed(&self) -> bool {
        match &self.backend {
            Backend::Pg(source) => source.is_closed(),
            Backend::Mysql(source) => source.is_closed(),
            Backend::File { .. } => false,
        }
    }
    pub fn info(&self) -> ConnectionInfo {
        self.info.clone()
    }
    pub fn cancellation(&self) -> Cancellation {
        match &self.backend {
            Backend::Pg(source) => Cancellation::Pg(Box::new(source.cancellation())),
            Backend::Mysql(source) => Cancellation::Mysql(source.cancellation()),
            Backend::File { .. } => Cancellation::File,
        }
    }
    pub async fn schema(&mut self) -> Result<(Vec<SchemaTable>, bool)> {
        match &mut self.backend {
            Backend::Pg(source) => schema_tables(
                source.metadata(PG_SCHEMA_SQL).await?,
                DataSource::Postgresql,
            ),
            Backend::Mysql(source) => {
                schema_tables(source.metadata(MYSQL_SCHEMA_SQL).await?, DataSource::Mysql)
            }
            Backend::File { schema, .. } => Ok((vec![schema.as_ref().clone()], false)),
        }
    }
    pub async fn query(
        &mut self,
        sql: &str,
        read_only: bool,
        timeout: Duration,
    ) -> Result<QueryResult> {
        match &mut self.backend {
            Backend::Pg(source) => source.query(sql, read_only).await,
            Backend::Mysql(source) => source.query(sql, read_only).await,
            Backend::File {
                config,
                bytes,
                worker,
                ..
            } => {
                ensure!(read_only, "Imported file tables are read-only");
                run_file_worker(worker, config, bytes, Some(sql), timeout)
                    .await?
                    .result
                    .ok_or_else(|| anyhow::anyhow!("File SQL worker returned no query result"))
            }
        }
    }
}
