use anyhow::{Context, Result, ensure};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper_util::rt::TokioIo;
use std::{sync::Arc, time::Duration};
use tokio::io::{AsyncRead, AsyncWrite};
use url::Url;
pub(super) trait Stream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Stream for T {}
pub(crate) struct Connection {
    sender: hyper::client::conn::http1::SendRequest<Full<Bytes>>,
    task: tokio::task::JoinHandle<()>,
    pub binding: Option<Vec<u8>>,
}
impl Drop for Connection {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Connection {
    pub async fn connect(
        url: &Url,
        policy: crate::NetworkPolicy,
        verify_tls: bool,
        channel_binding: bool,
        network: Option<&crate::RequestNetwork>,
    ) -> Result<Self> {
        let defaults = crate::RequestNetwork::default();
        let network = network.unwrap_or(&defaults);
        crate::validate_request_network(network, false)?;
        ensure!(
            network.http_mode == crate::HttpMode::Http1,
            "NTLM requires HTTP1"
        );
        let timeout = Duration::from_millis(network.connect_timeout_ms);
        let tcp = tokio::time::timeout(
            timeout,
            super::network::connect(url, policy, verify_tls, network),
        )
        .await
        .context("NTLM connection timed out")??;
        let (stream, binding): (Box<dyn Stream>, Option<Vec<u8>>) = if url.scheme() == "https" {
            let mut config = crate::request_network::tls_config(network, verify_tls)?;
            config.alpn_protocols = vec![b"http/1.1".to_vec()];
            let host = url
                .host_str()
                .context("NTLM host missing")?
                .trim_matches(['[', ']'])
                .to_owned();
            let name =
                rustls::pki_types::ServerName::try_from(host).context("Invalid NTLM TLS name")?;
            let tls = tokio::time::timeout(
                timeout,
                tokio_rustls::TlsConnector::from(Arc::new(config)).connect(name, tcp),
            )
            .await
            .context("NTLM TLS handshake timed out")?
            .context("NTLM TLS handshake failed")?;
            let binding = if channel_binding {
                let certificate = tls
                    .get_ref()
                    .1
                    .peer_certificates()
                    .and_then(|certs| certs.first())
                    .context("NTLM TLS peer certificate missing")?;
                Some(certificate_binding(certificate.as_ref())?)
            } else {
                None
            };
            (Box::new(tls), binding)
        } else {
            (Box::new(tcp), None)
        };
        let (sender, connection) = hyper::client::conn::http1::Builder::new()
            .max_headers(128)
            .handshake(TokioIo::new(stream))
            .await
            .context("NTLM HTTP1 handshake failed")?;
        let task = tokio::spawn(async move {
            let _ = connection.await;
        });
        Ok(Self {
            sender,
            task,
            binding,
        })
    }
    pub async fn send(&mut self, request: reqwest::Request) -> Result<reqwest::Response> {
        let mut url = request.url().clone();
        url.set_fragment(None);
        let absolute: http::Uri = url.as_str().parse()?;
        let authority = absolute
            .authority()
            .context("NTLM request authority missing")?;
        let target = absolute.path_and_query().map(|v| v.as_str()).unwrap_or("/");
        let bytes = request
            .body()
            .map(|b| {
                b.as_bytes()
                    .context("NTLM requires materialized body bytes")
            })
            .transpose()?
            .unwrap_or(&[]);
        let mut outbound = http::Request::builder()
            .method(request.method().clone())
            .uri(target)
            .version(http::Version::HTTP_11)
            .body(Full::new(Bytes::copy_from_slice(bytes)))?;
        *outbound.headers_mut() = request.headers().clone();
        outbound
            .headers_mut()
            .insert("host", authority.as_str().parse()?);
        let response = self
            .sender
            .send_request(outbound)
            .await
            .context("NTLM HTTP request failed; connection is not retried")?;
        let (parts, body) = response.into_parts();
        let body = reqwest::Body::wrap_stream(body.into_data_stream());
        Ok(http::Response::from_parts(parts, body).into())
    }
}
fn certificate_binding(der: &[u8]) -> Result<Vec<u8>> {
    use sha2::Digest;
    use x509_parser::{parse_x509_certificate, signature_algorithm::SignatureAlgorithm};
    ensure!(
        der.len() <= 1024 * 1024,
        "NTLM TLS certificate exceeds limit"
    );
    let (_, certificate) =
        parse_x509_certificate(der).map_err(|_| anyhow::anyhow!("Invalid NTLM TLS certificate"))?;
    let algorithm = &certificate.signature_algorithm;
    let oid = algorithm.algorithm.to_id_string();
    let hash = if oid == "1.2.840.113549.1.1.10" {
        if let SignatureAlgorithm::RSASSA_PSS(params) = SignatureAlgorithm::try_from(algorithm)
            .map_err(|_| anyhow::anyhow!("Invalid NTLM TLS PSS parameters"))?
        {
            params
                .hash_algorithm()
                .map(|v| v.algorithm.to_id_string())
                .unwrap_or("1.3.14.3.2.26".into())
        } else {
            unreachable!()
        }
    } else {
        oid
    };
    let digest = match hash.as_str() {
        "1.2.840.113549.1.1.12" | "1.2.840.10045.4.3.3" | "2.16.840.1.101.3.4.2.2" => {
            sha2::Sha384::digest(der).to_vec()
        }
        "1.2.840.113549.1.1.13" | "1.2.840.10045.4.3.4" | "2.16.840.1.101.3.4.2.3" => {
            sha2::Sha512::digest(der).to_vec()
        }
        "1.2.840.113549.1.1.11"
        | "1.2.840.10045.4.3.2"
        | "2.16.840.1.101.3.4.2.1"
        | "1.2.840.113549.1.1.5"
        | "1.3.14.3.2.26"
        | "1.2.840.113549.1.1.4"
        | "1.2.840.10045.4.1"
        | "1.2.840.10040.4.3" => sha2::Sha256::digest(der).to_vec(),
        "1.2.840.10045.4.3.1" | "2.16.840.1.101.3.4.2.4" => sha2::Sha224::digest(der).to_vec(),
        _ => anyhow::bail!("Unsupported NTLM TLS certificate signature hash"),
    };
    let mut binding = b"tls-server-end-point:".to_vec();
    binding.extend(digest);
    Ok(binding)
}
