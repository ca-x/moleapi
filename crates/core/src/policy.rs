use anyhow::{Context, Result, ensure};
use std::net::IpAddr;
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
