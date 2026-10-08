//! Portable networking source and library-owned TLS/identity conversion.
mod socket;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
pub use socket::{NetworkStream, connect_request_socket};
use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use url::Url;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HttpMode {
    #[default]
    Http1,
    Auto,
    Http2PriorKnowledge,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RequestProxy {
    pub enabled: bool,
    pub url: String,
    pub username: String,
    pub password: String,
    pub bypass: String,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityFormat {
    #[default]
    Pem,
    Pkcs12,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ClientIdentity {
    pub enabled: bool,
    pub format: IdentityFormat,
    pub certificate_pem: String,
    pub key_pem: String,
    pub pkcs12_base64: String,
    pub password: String,
    pub alias: String,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DnsOverride {
    pub hostname: String,
    pub addresses: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RequestNetwork {
    pub http_mode: HttpMode,
    pub proxy: RequestProxy,
    pub built_in_roots: bool,
    pub ca_pem: String,
    pub identity: ClientIdentity,
    pub dns: Vec<DnsOverride>,
    pub connect_timeout_ms: u64,
}
impl Default for RequestNetwork {
    fn default() -> Self {
        Self {
            http_mode: HttpMode::Http1,
            proxy: RequestProxy::default(),
            built_in_roots: true,
            ca_pem: String::new(),
            identity: ClientIdentity::default(),
            dns: Vec::new(),
            connect_timeout_ms: 15000,
        }
    }
}
pub fn validate_request_network(c: &RequestNetwork, templates: bool) -> Result<()> {
    ensure!(
        (100..=120000).contains(&c.connect_timeout_ms),
        "Network connect timeout must be 100..120000 ms"
    );
    ensure!(
        c.ca_pem.len() <= 128 * 1024
            && c.identity.certificate_pem.len() <= 128 * 1024
            && c.identity.key_pem.len() <= 65536
            && c.identity.pkcs12_base64.len() <= 256 * 1024
            && c.identity.password.len() <= 4096
            && c.identity.alias.len() <= 512,
        "Network TLS fields exceed limits"
    );
    ensure!(
        c.proxy.url.len() <= 8192
            && c.proxy.username.len() <= 4096
            && c.proxy.password.len() <= 65536
            && c.proxy.bypass.len() <= 8192,
        "Network proxy fields exceed limits"
    );
    ensure!(c.dns.len() <= 64, "DNS override limit exceeded");
    let mut hosts = std::collections::HashSet::new();
    for entry in &c.dns {
        ensure!(
            entry.hostname.len() <= 253 && entry.addresses.len() <= 16,
            "DNS override exceeds limits"
        );
        if !(templates && entry.hostname.contains("{{")) {
            let host = canonical_host(&entry.hostname)?;
            ensure!(hosts.insert(host), "Duplicate DNS override host");
        }
        for address in &entry.addresses {
            ensure!(address.len() <= 128, "DNS address exceeds limit");
            if !(templates && address.contains("{{")) {
                address
                    .parse::<IpAddr>()
                    .context("DNS overrides require IP addresses")?;
            }
        }
    }
    if !templates && c.proxy.enabled {
        proxy_url(&c.proxy)?;
        ensure!(
            !c.proxy.username.contains(':')
                && !c.proxy.username.chars().any(char::is_control)
                && !c.proxy.password.chars().any(char::is_control),
            "Invalid proxy credentials"
        );
    }
    Ok(())
}
fn canonical_host(raw: &str) -> Result<String> {
    let host =
        url::Host::parse(raw.trim_matches(['[', ']'])).context("Invalid DNS override hostname")?;
    Ok(host.to_string().to_ascii_lowercase())
}
pub fn proxy_url(c: &RequestProxy) -> Result<Url> {
    let url = Url::parse(&c.url).context("Invalid proxy URL")?;
    ensure!(
        matches!(
            url.scheme(),
            "http" | "https" | "socks4" | "socks4a" | "socks5" | "socks5h"
        ) && url.host().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && matches!(url.path(), "" | "/")
            && url.query().is_none()
            && url.fragment().is_none(),
        "Proxy requires a supported scheme/host and no embedded credentials/path"
    );
    Ok(url)
}
pub(crate) fn proxy_for(
    c: &RequestNetwork,
    url: &Url,
    policy: crate::NetworkPolicy,
) -> Result<Option<Url>> {
    if !c.proxy.enabled {
        return Ok(None);
    }
    let proxy = proxy_url(&c.proxy)?;
    ensure!(
        policy.allow_private_network,
        "Proxy routing requires the server administrator to allow private-network access"
    );
    if no_proxy::NoProxy::from(c.proxy.bypass.as_str())
        .matches(url.host_str().unwrap_or_default().trim_matches(['[', ']']))
    {
        return Ok(None);
    }
    Ok(Some(proxy))
}
pub(crate) fn has_dns_override(c: &RequestNetwork, url: &Url) -> bool {
    c.dns.iter().any(|entry| {
        canonical_host(&entry.hostname).is_ok_and(|h| {
            url.host_str()
                .is_some_and(|host| h == host.trim_matches(['[', ']']).to_ascii_lowercase())
        })
    })
}
pub async fn network_destination(
    url: &Url,
    policy: crate::NetworkPolicy,
    c: Option<&RequestNetwork>,
) -> Result<Vec<SocketAddr>> {
    let host = url
        .host_str()
        .context("Destination host missing")?
        .trim_matches(['[', ']']);
    let port = url
        .port_or_known_default()
        .context("Destination port missing")?;
    if let Some(entry) = c.and_then(|c| {
        c.dns.iter().find(|entry| {
            canonical_host(&entry.hostname).is_ok_and(|h| h == host.to_ascii_lowercase())
        })
    }) {
        ensure!(!entry.addresses.is_empty(), "DNS override has no addresses");
        let addresses = entry
            .addresses
            .iter()
            .map(|raw| raw.parse::<IpAddr>().map(|ip| SocketAddr::new(ip, port)))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ensure!(
            policy.allow_private_network || addresses.iter().all(|a| crate::public_ip(a.ip())),
            "Private or reserved DNS override address is blocked"
        );
        return Ok(addresses);
    }
    crate::checked_destination(url, policy).await
}
pub(crate) fn parse_ca(raw: &str) -> Result<Vec<rustls::pki_types::CertificateDer<'static>>> {
    let mut reader = std::io::Cursor::new(raw);
    let certificates = rustls_pemfile::certs(&mut reader)
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("Invalid CA PEM")?;
    ensure!(
        !certificates.is_empty() && certificates.len() <= 32,
        "CA PEM must contain 1..32 certificates"
    );
    Ok(certificates)
}
pub fn identity_pem(c: &ClientIdentity) -> Result<String> {
    if c.format == IdentityFormat::Pem {
        ensure!(
            !c.certificate_pem.is_empty() && !c.key_pem.is_empty(),
            "Client certificate/key is missing; select it again"
        );
        return Ok(format!("{}\n{}", c.certificate_pem, c.key_pem));
    }
    use base64::{Engine, engine::general_purpose::STANDARD};
    let bytes = STANDARD
        .decode(&c.pkcs12_base64)
        .context("Invalid PKCS12 Base64")?;
    ensure!(bytes.len() <= 192 * 1024, "PKCS12 identity exceeds 192 KiB");
    let store = p12_keystore::KeyStore::from_pkcs12(
        &bytes,
        &c.password,
        p12_keystore::Pkcs12ImportPolicy::Strict,
    )
    .map_err(|_| anyhow::anyhow!("PKCS12 decryption or identity parsing failed"))?;
    let chains: Vec<_> = store
        .entries()
        .filter_map(|(alias, entry)| match entry {
            p12_keystore::KeyStoreEntry::PrivateKeyChain(chain) => Some((alias, chain)),
            _ => None,
        })
        .collect();
    let chain = if c.alias.is_empty() {
        ensure!(
            chains.len() == 1,
            "PKCS12 identity is ambiguous; choose an alias"
        );
        chains[0].1
    } else {
        chains
            .iter()
            .find(|(alias, _)| alias.as_str() == c.alias)
            .map(|(_, chain)| *chain)
            .context("PKCS12 identity alias not found")?
    };
    let mut output = String::new();
    for certificate in chain.certs() {
        output.push_str(&pem::encode(&pem::Pem::new(
            "CERTIFICATE",
            certificate.as_der().to_vec(),
        )));
    }
    output.push_str(&pem::encode(&pem::Pem::new(
        "PRIVATE KEY",
        chain.key().as_der().to_vec(),
    )));
    Ok(output)
}
pub fn request_tls_config(c: &RequestNetwork, verify: bool) -> Result<rustls::ClientConfig> {
    validate_request_network(c, false)?;
    tls_config(c, verify)
}
pub(crate) fn tls_config(c: &RequestNetwork, verify: bool) -> Result<rustls::ClientConfig> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut roots = rustls::RootCertStore::empty();
    if c.built_in_roots {
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    }
    if !c.ca_pem.is_empty() {
        for cert in parse_ca(&c.ca_pem)? {
            roots.add(cert).context("Invalid CA certificate")?;
        }
    }
    let builder = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .with_root_certificates(roots);
    let mut config = if c.identity.enabled {
        let material = identity_pem(&c.identity)?;
        let certificates = parse_ca(&material)?;
        let key = rustls_pemfile::private_key(&mut std::io::Cursor::new(&material))?
            .context("Client private key missing")?;
        builder
            .with_client_auth_cert(certificates, key)
            .context("Invalid client certificate/key pair")?
    } else {
        builder.with_no_client_auth()
    };
    if !verify {
        config
            .dangerous()
            .set_certificate_verifier(Arc::new(crate::UnverifiedCertificate));
    }
    Ok(config)
}
/// Validated policy/TLS inputs shared by SDKs using different reqwest versions.
pub struct PreparedRequestNetwork {
    pub addresses: Vec<SocketAddr>,
    pub proxy: Option<Url>,
    pub tls: rustls::ClientConfig,
}
pub async fn prepare_request_network(
    url: &Url,
    policy: crate::NetworkPolicy,
    verify: bool,
    c: Option<&RequestNetwork>,
) -> Result<PreparedRequestNetwork> {
    let default = RequestNetwork::default();
    let c = c.unwrap_or(&default);
    validate_request_network(c, false)?;
    let selected_proxy = proxy_for(c, url, policy)?;
    if let Some(proxy) = &selected_proxy {
        ensure!(
            !(proxy.scheme() == "https" && c.identity.enabled),
            "HTTPS proxy with client identity requires a transport with separate proxy TLS configuration"
        );
        ensure!(
            !has_dns_override(c, url) || matches!(proxy.scheme(), "socks4" | "socks5"),
            "Remote-resolving proxy cannot preserve DNS overrides; bypass this host or use a locally resolving SOCKS proxy"
        );
    }
    let remote = selected_proxy
        .as_ref()
        .is_some_and(|p| !matches!(p.scheme(), "socks4" | "socks5"));
    let addresses = if remote {
        Vec::new()
    } else {
        network_destination(url, policy, Some(c)).await?
    };
    let mut tls = tls_config(c, verify)?;
    tls.alpn_protocols = match c.http_mode {
        HttpMode::Http1 => vec![b"http/1.1".to_vec()],
        _ => vec![b"h2".to_vec(), b"http/1.1".to_vec()],
    };
    Ok(PreparedRequestNetwork {
        addresses,
        proxy: selected_proxy,
        tls,
    })
}
pub async fn checked_request_client_builder(
    url: &Url,
    policy: crate::NetworkPolicy,
    verify: bool,
    c: Option<&RequestNetwork>,
) -> Result<reqwest::ClientBuilder> {
    let default = RequestNetwork::default();
    let c = c.unwrap_or(&default);
    let prepared = prepare_request_network(url, policy, verify, Some(c)).await?;
    let addresses = prepared.addresses;
    let host = url
        .host_str()
        .context("Destination host missing")?
        .trim_matches(['[', ']']);
    let mut builder = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .connect_timeout(Duration::from_millis(c.connect_timeout_ms))
        .resolve_to_addrs(host, &addresses);
    match c.http_mode {
        HttpMode::Http1 => builder = builder.http1_only(),
        HttpMode::Auto => {}
        HttpMode::Http2PriorKnowledge => builder = builder.http2_prior_knowledge(),
    }
    builder = builder.use_preconfigured_tls(prepared.tls);
    if let Some(url) = prepared.proxy {
        let mut proxy = reqwest::Proxy::all(url)?;
        if !c.proxy.username.is_empty() || !c.proxy.password.is_empty() {
            proxy = proxy.basic_auth(&c.proxy.username, &c.proxy.password);
        }
        builder = builder.proxy(proxy);
    }
    Ok(builder)
}
pub async fn checked_request_client(
    url: &Url,
    policy: crate::NetworkPolicy,
    verify: bool,
    c: Option<&RequestNetwork>,
) -> Result<reqwest::Client> {
    checked_request_client_builder(url, policy, verify, c)
        .await?
        .build()
        .context("Request network client could not be built")
}

/// Sensitive source forms only: never decrypt an untrusted PFX for redaction.
pub fn network_private_sources(c: &RequestNetwork) -> Vec<String> {
    let mut values = vec![
        c.proxy.username.clone(),
        c.proxy.password.clone(),
        c.identity.certificate_pem.clone(),
        c.identity.key_pem.clone(),
        c.identity.pkcs12_base64.clone(),
        c.identity.password.clone(),
        c.identity.alias.clone(),
    ];
    if !c.proxy.username.is_empty() || !c.proxy.password.is_empty() {
        use base64::Engine;
        let credentials = format!("{}:{}", c.proxy.username, c.proxy.password);
        values.push(base64::engine::general_purpose::STANDARD.encode(&credentials));
        values.push(credentials);
    }
    for raw in [&c.identity.certificate_pem, &c.identity.key_pem] {
        if let Ok(blocks) = pem::parse_many(raw) {
            use base64::Engine;
            for block in blocks {
                values.push(base64::engine::general_purpose::STANDARD.encode(block.contents()));
                values.push(pem::encode(&block));
                values.push(pem::encode(&block).replace("\r\n", "\n"));
            }
        }
    }
    values.retain(|v| !v.is_empty());
    values
}
