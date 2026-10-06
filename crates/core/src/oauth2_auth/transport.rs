use crate::{NetworkPolicy, OAuth2Auth};
use anyhow::{Context, Result, ensure};
use std::time::Duration;
/// Bounded, pinned transport supplied to the mature SDK. Redirects are never followed.
pub async fn oauth2_http(
    config: &OAuth2Auth,
    request: oauth2::HttpRequest,
    policy: NetworkPolicy,
    verify_tls: bool,
) -> std::result::Result<oauth2::HttpResponse, std::io::Error> {
    async fn fetch(
        config: &OAuth2Auth,
        request: oauth2::HttpRequest,
        policy: NetworkPolicy,
        verify_tls: bool,
    ) -> Result<oauth2::HttpResponse> {
        ensure!(
            request.body().len() <= 256 * 1024,
            "OAuth2 request exceeds limit"
        );
        let url = crate::valid_url(&request.uri().to_string())?;
        let client = crate::checked_client(&url, policy, verify_tls).await?;
        let (parts, body) = request.into_parts();
        let mut builder = client
            .request(
                reqwest::Method::from_bytes(parts.method.as_str().as_bytes())?,
                url,
            )
            .headers(parts.headers)
            .body(body);
        for row in config.token_headers.iter().filter(|p| p.enabled) {
            builder = builder.header(&row.key, &row.value);
        }
        let response = builder.send().await?;
        let status = response.status();
        ensure!(
            !status.is_redirection(),
            "OAuth2 endpoint redirects are unsupported"
        );
        ensure!(
            response
                .content_length()
                .is_none_or(|size| size <= 1024 * 1024),
            "OAuth2 response exceeds limit"
        );
        let mut builder = oauth2::http::Response::builder().status(status);
        *builder
            .headers_mut()
            .context("OAuth2 response headers unavailable")? = response.headers().clone();
        let mut bytes = Vec::new();
        use futures_util::StreamExt;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            ensure!(
                bytes.len().saturating_add(chunk.len()) <= 1024 * 1024,
                "OAuth2 response exceeds limit"
            );
            bytes.extend_from_slice(&chunk);
        }
        Ok(builder.body(bytes)?)
    }
    match tokio::time::timeout(
        Duration::from_secs(30),
        fetch(config, request, policy, verify_tls),
    )
    .await
    {
        Ok(Ok(response)) => Ok(response),
        _ => Err(std::io::Error::other(
            "OAuth2 endpoint request failed or exceeded its limits",
        )),
    }
}
pub(super) struct HttpClient {
    pub(super) config: OAuth2Auth,
    pub(super) policy: NetworkPolicy,
    pub(super) verify_tls: bool,
}
impl<'c> oauth2::AsyncHttpClient<'c> for HttpClient {
    type Error = std::io::Error;
    type Future = futures_util::future::BoxFuture<'c, Result<oauth2::HttpResponse, std::io::Error>>;
    fn call(&'c self, request: oauth2::HttpRequest) -> Self::Future {
        Box::pin(oauth2_http(
            &self.config,
            request,
            self.policy,
            self.verify_tls,
        ))
    }
}
