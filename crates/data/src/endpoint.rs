use anyhow::{Context, Result, ensure};
use moleapi_core::{DataConfig, DataSource, NetworkPolicy, RequestSpec};
use std::net::SocketAddr;
#[derive(Clone)]
pub struct Endpoint {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub addresses: Vec<SocketAddr>,
    pub tls: bool,
    pub verify_tls: bool,
    pub ca_pem: String,
    pub public_url: String,
}
impl Endpoint {
    pub async fn checked(
        request: &RequestSpec,
        config: &DataConfig,
        policy: NetworkPolicy,
    ) -> Result<Self> {
        ensure!(
            matches!(config.source, DataSource::Postgresql | DataSource::Mysql),
            "Database endpoint requires PG/MySQL"
        );
        moleapi_core::validate_data_config(config)?;
        let mut url = moleapi_core::data_url(&request.url, config.source)?;
        let decode = |value: &str| -> Result<String> {
            Ok(percent_encoding::percent_decode_str(value)
                .decode_utf8()?
                .into_owned())
        };
        let (username, password) = match request.auth.kind.as_str() {
            "basic" => (request.auth.username.clone(), request.auth.password.clone()),
            "none" => (
                decode(url.username())?,
                decode(url.password().unwrap_or_default())?,
            ),
            _ => anyhow::bail!("Database authentication accepts username/password only"),
        };
        ensure!(
            !username.is_empty()
                && username.len() <= 512
                && password.len() <= 4096
                && !username.contains('\0')
                && !password.contains('\0'),
            "Invalid/missing Data database credentials"
        );
        let database = decode(url.path().trim_start_matches('/'))?;
        ensure!(
            !database.is_empty() && database.len() <= 512 && !database.contains('\0'),
            "Invalid Data database name"
        );
        let mut tls = config.tls;
        let mut verify_tls = request.verify_tls;
        if let Some((_, mode)) = url.query_pairs().next() {
            match mode.as_ref() {
                "disable" => tls = false,
                "require" => tls = true,
                "verify-full" => {
                    tls = true;
                    verify_tls = true
                }
                _ => unreachable!(),
            }
        }
        let addresses = moleapi_core::checked_destination(&url, policy).await?;
        let host = url
            .host_str()
            .context("Missing Data host")?
            .trim_matches(['[', ']'])
            .to_owned();
        let port = url.port().context("Missing Data port")?;
        let _ = url.set_username("");
        let _ = url.set_password(None);
        Ok(Self {
            host,
            port,
            database,
            username,
            password,
            addresses,
            tls,
            verify_tls,
            ca_pem: config.ca_pem.clone(),
            public_url: url.to_string(),
        })
    }
    pub fn tls_config(&self) -> Result<rustls::ClientConfig> {
        let mut roots = rustls::RootCertStore::empty();
        for certificate in rustls_native_certs::load_native_certs().certs {
            let _ = roots.add(certificate);
        }
        for certificate in moleapi_core::data_ca_certificates(&self.ca_pem)? {
            roots.add(certificate)?;
        }
        let mut config = rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()?
        .with_root_certificates(roots)
        .with_no_client_auth();
        if !self.verify_tls {
            config
                .dangerous()
                .set_certificate_verifier(std::sync::Arc::new(moleapi_core::UnverifiedCertificate));
        }
        Ok(config)
    }
}
