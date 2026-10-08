//! Library-owned sockets and CONNECT/SOCKS tunnels shared by protocol SDKs.
use tokio::io::{AsyncRead, AsyncWrite};
pub trait NetworkStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> NetworkStream for T {}
use anyhow::{Context, Result, ensure};
use hyper_util::rt::TokioIo;
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context as TaskContext, Poll},
};
use tower_service::Service;
use url::Url;

// A tunnel connector receives only the already checked proxy socket. It cannot
// resolve/reconnect behind the destination policy or retry an NTLM exchange.
struct OnceConnector(Option<Box<dyn NetworkStream>>);
impl Service<http::Uri> for OnceConnector {
    type Response = TokioIo<Box<dyn NetworkStream>>;
    type Error = std::io::Error;
    type Future = Pin<Box<dyn Future<Output = std::io::Result<Self::Response>> + Send>>;
    fn poll_ready(&mut self, _: &mut TaskContext<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, _: http::Uri) -> Self::Future {
        let stream = self.0.take();
        Box::pin(async move {
            stream
                .map(TokioIo::new)
                .ok_or_else(|| std::io::Error::other("Proxy socket already consumed"))
        })
    }
}
pub async fn connect_request_socket(
    url: &Url,
    policy: crate::NetworkPolicy,
    verify: bool,
    c: &crate::RequestNetwork,
) -> Result<Box<dyn NetworkStream>> {
    crate::validate_request_network(c, false)?;
    let selected_proxy = crate::request_network::proxy_for(c, url, policy)?;
    let overridden = crate::request_network::has_dns_override(c, url);
    let remote = selected_proxy
        .as_ref()
        .is_some_and(|p| !matches!(p.scheme(), "socks4" | "socks5"));
    let addresses = if remote && !overridden {
        Vec::new()
    } else {
        crate::network_destination(url, policy, Some(c)).await?
    };
    let Some(proxy) = selected_proxy else {
        let tcp = tokio::net::TcpStream::connect(addresses.as_slice()).await?;
        tcp.set_nodelay(true)?;
        return Ok(Box::new(tcp));
    };
    let port = proxy.port().unwrap_or(match proxy.scheme() {
        "https" => 443,
        "http" => 80,
        _ => 1080,
    });
    let host = proxy
        .host_str()
        .context("Proxy host missing")?
        .trim_matches(['[', ']']);
    let proxy_addresses = tokio::net::lookup_host((host, port))
        .await?
        .collect::<Vec<_>>();
    ensure!(!proxy_addresses.is_empty(), "Proxy address missing");
    let tcp = tokio::net::TcpStream::connect(proxy_addresses.as_slice()).await?;
    tcp.set_nodelay(true)?;
    if proxy.scheme().starts_with("socks") {
        let target = if !overridden && matches!(proxy.scheme(), "socks4a" | "socks5h") {
            tokio_socks::TargetAddr::Domain(
                host_for(url)?.into(),
                url.port_or_known_default().context("Target port missing")?,
            )
        } else {
            tokio_socks::TargetAddr::Ip(addresses[0])
        };
        if proxy.scheme().starts_with("socks4") {
            ensure!(
                c.proxy.password.is_empty(),
                "SOCKS4 does not support passwords"
            );
            let stream = tokio_socks::tcp::Socks4Stream::connect_with_userid_and_socket(
                tcp,
                target,
                &c.proxy.username,
            )
            .await?;
            return Ok(Box::new(stream.into_inner()));
        }
        let stream = if c.proxy.username.is_empty() && c.proxy.password.is_empty() {
            tokio_socks::tcp::Socks5Stream::connect_with_socket(tcp, target).await?
        } else {
            tokio_socks::tcp::Socks5Stream::connect_with_password_and_socket(
                tcp,
                target,
                &c.proxy.username,
                &c.proxy.password,
            )
            .await?
        };
        return Ok(Box::new(stream.into_inner()));
    }
    let socket: Box<dyn NetworkStream> = if proxy.scheme() == "https" {
        let mut proxy_settings = c.clone();
        proxy_settings.identity.enabled = false;
        let mut tls = crate::request_network::tls_config(&proxy_settings, verify)?;
        tls.alpn_protocols = vec![b"http/1.1".to_vec()];
        let name = rustls::pki_types::ServerName::try_from(host.to_owned())?;
        Box::new(
            tokio_rustls::TlsConnector::from(Arc::new(tls))
                .connect(name, tcp)
                .await?,
        )
    } else {
        Box::new(tcp)
    };
    let mut definition = hyper_http_proxy::Proxy::new(
        hyper_http_proxy::Intercept::All,
        proxy.as_str().parse::<http::Uri>()?,
    );
    definition.force_connect();
    if !c.proxy.username.is_empty() || !c.proxy.password.is_empty() {
        definition.set_authorization(headers::Authorization::basic(
            &c.proxy.username,
            &c.proxy.password,
        ));
    }
    let mut connector = hyper_http_proxy::ProxyConnector::from_proxy_unsecured(
        OnceConnector(Some(socket)),
        definition,
    );
    // HTTP URI requests a raw tunnel; destination TLS is applied by Connection,
    // where the actual peer certificate remains available for channel binding.
    let authority = url[url::Position::BeforeHost..url::Position::AfterPort].to_owned();
    let port = url.port_or_known_default().context("Target port missing")?;
    let authority = if overridden {
        addresses[0].to_string()
    } else if url.port().is_none() {
        format!("{authority}:{port}")
    } else {
        authority
    };
    let tunnel = connector
        .call(format!("http://{authority}/").parse()?)
        .await
        .context("CONNECT tunnel failed")?;
    Ok(Box::new(TokioIo::new(tunnel)))
}
fn host_for(url: &Url) -> Result<&str> {
    Ok(url
        .host_str()
        .context("Target host missing")?
        .trim_matches(['[', ']']))
}
