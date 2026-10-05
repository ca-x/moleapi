//! Persisted Data request settings, independent from application storage configuration.
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use url::Url;

pub const MAX_DATA_FILE: usize = 5 * 1024 * 1024;
pub const MAX_SQL_SOURCE: usize = 64 * 1024;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataSource {
    #[default]
    Postgresql,
    Mysql,
    LocalFile,
    RemoteFile,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataFileFormat {
    #[default]
    Csv,
    Json,
    Parquet,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DataConfig {
    pub source: DataSource,
    pub sql: String,
    pub file_name: String,
    pub file_format: DataFileFormat,
    pub file_base64: String,
    pub table_name: String,
    pub csv_header: bool,
    pub read_only: bool,
    pub tls: bool,
    pub ca_pem: String,
}
impl Default for DataConfig {
    fn default() -> Self {
        Self {
            source: DataSource::Postgresql,
            sql: "SELECT 1 AS value".into(),
            file_name: String::new(),
            file_format: DataFileFormat::Csv,
            file_base64: String::new(),
            table_name: "data".into(),
            csv_header: true,
            read_only: true,
            tls: true,
            ca_pem: String::new(),
        }
    }
}
pub fn validate_data_config(config: &DataConfig) -> Result<()> {
    ensure!(
        config.sql.len() <= MAX_SQL_SOURCE && !config.sql.contains('\0'),
        "Data SQL source exceeds limits"
    );
    ensure!(
        !config.table_name.is_empty()
            && config.table_name.len() <= 128
            && !config.table_name.contains('\0'),
        "Invalid Data table name"
    );
    ensure!(
        config.file_name.len() <= 512 && !config.file_name.contains('\0'),
        "Invalid Data file name"
    );
    ensure!(
        config.ca_pem.len() <= 65536,
        "Data CA certificates exceed limit"
    );
    ensure!(
        config.file_base64.len() <= MAX_DATA_FILE.div_ceil(3) * 4,
        "Data file exceeds 5 MiB"
    );
    if !config.file_base64.is_empty() {
        ensure!(
            config.source == DataSource::LocalFile,
            "Only selected local files can contain embedded bytes"
        );
        let bytes = STANDARD
            .decode(&config.file_base64)
            .context("Invalid Data file Base64")?;
        ensure!(bytes.len() <= MAX_DATA_FILE, "Data file exceeds 5 MiB");
    }
    Ok(())
}
/// A single network host only. Driver-specific options cannot override the checked destination.
pub fn data_url(raw: &str, source: DataSource) -> Result<Url> {
    ensure!(
        raw.len() <= 16384 && !raw.contains('\0'),
        "Data connection URL exceeds limits"
    );
    let mut url = Url::parse(raw).context("Invalid Data connection URL")?;
    match source {
        DataSource::Postgresql => ensure!(
            matches!(url.scheme(), "postgres" | "postgresql"),
            "PostgreSQL source requires postgres/postgresql URL"
        ),
        DataSource::Mysql => ensure!(url.scheme() == "mysql", "MySQL source requires mysql URL"),
        DataSource::RemoteFile => ensure!(
            matches!(url.scheme(), "http" | "https"),
            "Remote file requires HTTP(S) URL"
        ),
        DataSource::LocalFile => anyhow::bail!("Selected local files do not use a network URL"),
    }
    let host = url.host_str().context("Data URL requires one host")?;
    ensure!(
        !host.contains(['/', '%', ',']),
        "Data URLs cannot select sockets or multiple hosts"
    );
    ensure!(
        url.fragment().is_none(),
        "Data URL cannot contain a fragment"
    );
    if source == DataSource::RemoteFile {
        ensure!(
            url.username().is_empty() && url.password().is_none(),
            "Remote file URL credentials must use request authentication"
        );
    } else {
        ensure!(
            url.path().len() > 1,
            "Database URL requires a database name"
        );
        let mut option_seen = false;
        for (key, value) in url.query_pairs() {
            ensure!(
                !option_seen,
                "Duplicate Data connection options are unsupported"
            );
            option_seen = true;
            ensure!(
                key == "sslmode" && matches!(value.as_ref(), "disable" | "require" | "verify-full"),
                "Unsupported connection option; use dedicated Data settings"
            );
        }
        if url.port().is_none() {
            url.set_port(Some(if source == DataSource::Mysql {
                3306
            } else {
                5432
            }))
            .map_err(|_| anyhow::anyhow!("Invalid Data port"))?;
        }
    }
    Ok(url)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retains_selected_file_and_sql_but_rejects_hidden_network_options() {
        let config = DataConfig {
            source: DataSource::LocalFile,
            sql: "SELECT '原文' AS text".into(),
            file_base64: STANDARD.encode([0, 255, 65]),
            ..Default::default()
        };
        validate_data_config(&config).unwrap();
        let copied: DataConfig =
            serde_json::from_value(serde_json::to_value(&config).unwrap()).unwrap();
        assert_eq!(copied, config);
        assert_eq!(
            data_url(
                "postgresql://user:pass@db.example/main",
                DataSource::Postgresql
            )
            .unwrap()
            .port(),
            Some(5432)
        );
        for raw in [
            "postgresql://db.example/main?hostaddr=127.0.0.1",
            "postgresql://%2Fvar%2Frun/main",
            "postgresql://one,two/main",
            "postgresql://db.example/main?sslrootcert=/secret",
        ] {
            assert!(data_url(raw, DataSource::Postgresql).is_err(), "{raw}");
        }
    }
}
