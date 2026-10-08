mod request_network;
pub use request_network::*;
mod asap_auth;
pub use asap_auth::*;
mod edgegrid_auth;
pub use edgegrid_auth::*;
mod ntlm_auth;
pub use ntlm_auth::{NtlmAuth, validate_ntlm};
mod oauth1_auth;
pub use oauth1_auth::*;
mod cookies;
pub use cookies::*;
mod hawk_auth;
pub use hawk_auth::*;
mod aws_auth;
pub use aws_auth::*;
mod oauth2_auth;
pub use oauth2_auth::*;
mod inheritance;
pub use inheritance::*;
mod authentication;
pub use authentication::*;
mod request_body;
pub use request_body::{
    BinaryBody, MAX_BODY_SOURCE, MultipartBody, MultipartPart, MultipartValue,
    validate_structured_body,
};
mod tls;
pub use tls::UnverifiedCertificate;
mod data;
pub use data::*;
mod tcp;
pub use tcp::*;
mod a2a;
pub use a2a::*;
mod mcp;
pub use mcp::*;
mod soap;
pub use soap::*;
mod mqtt;
pub use mqtt::*;
mod assertions;
mod grpc;
pub use grpc::*;
mod graphql;
pub use graphql::*;
mod interpolation;
mod models;
mod policy;
mod redaction;
mod transport;
mod validation;
mod variables;
pub use assertions::*;
pub use interpolation::{resolve_grpc_source, resolve_mqtt_source, resolve_request, resolve_value};
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
