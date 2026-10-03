use anyhow::{Context, Result, ensure};
use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};
use url::Url;
#[derive(Clone, Copy)]
pub struct NetworkPolicy {
    pub allow_private_network: bool,
}

pub fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(a == 0
                || a == 10
                || a == 127
                || a >= 224
                || (a == 100 && (64..=127).contains(&b))
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192 && (b == 168 || b == 0 || (b == 88 && c == 99)))
                || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
                || (a == 203 && b == 0 && c == 113))
        }
        IpAddr::V6(ip) => {
            if let Some(mapped) = ip.to_ipv4_mapped() {
                return public_ip(IpAddr::V4(mapped));
            }
            let seg = ip.segments();
            // Only global unicast; also reject documentation and transition ranges.
            (seg[0] & 0xe000) == 0x2000
                && seg[0] != 0x2002
                && !(seg[0] == 0x2001 && (seg[1] <= 0x1ff || seg[1] == 0xdb8))
                && !(seg[0] == 0x3fff && (seg[1] & 0xf000) == 0)
        }
    }
}

pub fn protocol_url(raw: &str, websocket: bool) -> Result<Url> {
    let url = Url::parse(raw).context("Invalid request URL")?;
    ensure!(
        if websocket {
            matches!(url.scheme(), "ws" | "wss")
        } else {
            matches!(url.scheme(), "http" | "https")
        },
        "URL scheme is incompatible with request protocol"
    );
    ensure!(url.host_str().is_some(), "URL requires a host");
    ensure!(
        url.username().is_empty() && url.password().is_none(),
        "URL credentials are not supported; use request authentication"
    );
    Ok(url)
}

/// Resolve every address, reject unsafe destinations, and pin the result for this hop.
pub async fn checked_destination(url: &Url, policy: NetworkPolicy) -> Result<Vec<SocketAddr>> {
    let host = url
        .host_str()
        .context("URL requires host")?
        .trim_matches(['[', ']']);
    let port = url.port_or_known_default().context("URL requires port")?;
    let ips: Vec<_> = tokio::net::lookup_host((host, port))
        .await
        .context("DNS lookup failed")?
        .collect();
    ensure!(!ips.is_empty(), "DNS returned no addresses");
    ensure!(
        policy.allow_private_network || ips.iter().all(|a| public_ip(a.ip())),
        "Private or reserved network address is blocked"
    );
    Ok(ips)
}

pub async fn checked_client(
    url: &Url,
    policy: NetworkPolicy,
    verify_tls: bool,
) -> Result<reqwest::Client> {
    let ips = checked_destination(url, policy).await?;
    let host = url
        .host_str()
        .context("URL requires host")?
        .trim_matches(['[', ']']);
    Ok(reqwest::Client::builder()
        .no_proxy()
        .http1_only()
        .redirect(reqwest::redirect::Policy::none())
        .danger_accept_invalid_certs(!verify_tls)
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .resolve_to_addrs(host, &ips)
        .connect_timeout(Duration::from_secs(15))
        .build()?)
}

pub fn valid_url(raw: &str) -> Result<Url> {
    let url = Url::parse(raw).context("Invalid request URL")?;
    ensure!(
        matches!(url.scheme(), "http" | "https"),
        "Only HTTP and HTTPS are supported"
    );
    ensure!(url.host_str().is_some(), "URL requires a host");
    ensure!(
        url.username().is_empty() && url.password().is_none(),
        "URL credentials are not supported; use request authentication"
    );
    Ok(url)
}
