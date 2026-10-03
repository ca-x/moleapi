mod assertions;
mod interpolation;
mod models;
mod policy;
mod redaction;
mod transport;
mod validation;
mod variables;
pub use assertions::*;
pub use interpolation::resolve_request;
pub use models::*;
pub use policy::*;
pub use redaction::*;
pub use transport::*;
pub use validation::*;
pub use variables::*;
pub const MAX_BODY: usize = 5 * 1024 * 1024;
#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpolation::interpolate;
    use std::collections::HashMap;
    #[test]
    fn rejects_special_addresses() {
        for ip in [
            "127.0.0.1",
            "10.0.0.1",
            "169.254.169.254",
            "100.64.0.1",
            "198.18.0.1",
            "192.0.2.1",
            "::1",
            "::ffff:127.0.0.1",
            "2001:db8::1",
            "2002:7f00:1::",
        ] {
            assert!(!public_ip(ip.parse().unwrap()), "{ip}");
        }
        assert!(public_ip("8.8.8.8".parse().unwrap()));
    }
    #[test]
    fn variables_fail_closed() {
        assert!(interpolate("{{missing}}", &HashMap::new()).is_err());
        assert!(interpolate("{{unterminated", &HashMap::new()).is_err());
    }
    #[test]
    fn redacts_query() {
        let url = redact_url("https://example.com/?api_key=secret&search=hello", None);
        assert!(!url.contains("secret"));
        assert!(url.contains("hello"));
    }
}
