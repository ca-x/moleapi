use anyhow::{Context, Result, bail, ensure};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request},
};
use serde_json::Value;
use std::{path::Path, time::Duration};
use tower::ServiceExt;
pub enum Backend {
    Local(Router),
    Remote {
        base: reqwest::Url,
        client: reqwest::Client,
        token: Option<String>,
    },
}
impl Backend {
    pub async fn local(path: &Path) -> Result<Self> {
        Ok(Self::Local(
            moleapi_server::offline_with_worker(path, &std::env::current_exe()?).await?,
        ))
    }
    pub fn remote(raw: &str, token: Option<String>, timeout: u64) -> Result<Self> {
        let mut base = reqwest::Url::parse(raw).context("Invalid service URL")?;
        ensure!(
            matches!(base.scheme(), "http" | "https")
                && base.host_str().is_some()
                && base.username().is_empty()
                && base.password().is_none()
                && base.query().is_none()
                && base.fragment().is_none(),
            "Service URL must be HTTP(S), without URL credentials/query/fragment"
        );
        if !base.path().ends_with('/') {
            let path = format!("{}/", base.path());
            base.set_path(&path);
        }
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(timeout))
            .build()
            .context("Cannot initialize remote client")?;
        Ok(Self::Remote {
            base,
            client,
            token,
        })
    }
    pub async fn request(
        &self,
        method: &str,
        segments: &[&str],
        query: &[(&str, &str)],
        body: Option<Value>,
    ) -> Result<Value> {
        let mut path = reqwest::Url::parse("http://local.invalid/api/").unwrap();
        {
            let mut parts = path.path_segments_mut().unwrap();
            parts.pop_if_empty();
            for segment in segments {
                ensure!(
                    !segment.is_empty() && !matches!(*segment, "." | "..") && segment.len() <= 256,
                    "Invalid API resource identifier"
                );
                parts.push(segment);
            }
        }
        path.query_pairs_mut().extend_pairs(query.iter().copied());
        let method = Method::from_bytes(method.as_bytes()).context("Invalid API method")?;
        let (status, bytes) = match self {
            Self::Local(router) => {
                let uri = format!(
                    "{}{}",
                    path.path(),
                    path.query()
                        .map(|query| format!("?{query}"))
                        .unwrap_or_default()
                );
                let body = body.map(|body| serde_json::to_vec(&body)).transpose()?;
                let mut request = Request::builder()
                    .method(method)
                    .uri(uri)
                    .header("content-type", "application/json");
                if body.is_none() {
                    request = request.header("content-length", "0");
                }
                let response = router
                    .clone()
                    .oneshot(request.body(Body::from(body.unwrap_or_default()))?)
                    .await?;
                let status = response.status().as_u16();
                let bytes = to_bytes(response.into_body(), 24 * 1024 * 1024)
                    .await
                    .context("Local response exceeds limit")?
                    .to_vec();
                (status, bytes)
            }
            Self::Remote {
                base,
                client,
                token,
            } => {
                let mut url = base.clone();
                {
                    let mut parts = url
                        .path_segments_mut()
                        .map_err(|_| anyhow::anyhow!("Invalid service URL"))?;
                    parts.pop_if_empty().push("api");
                    for segment in segments {
                        parts.push(segment);
                    }
                }
                url.query_pairs_mut().extend_pairs(query.iter().copied());
                let mut request = client.request(method, url);
                if let Some(token) = token {
                    request = request.bearer_auth(token);
                }
                if let Some(body) = body {
                    request = request.json(&body);
                }
                let mut response = request
                    .send()
                    .await
                    .map_err(|_| anyhow::anyhow!("Service request failed"))?;
                let status = response.status().as_u16();
                let mut bytes = vec![];
                while let Some(chunk) = response
                    .chunk()
                    .await
                    .map_err(|_| anyhow::anyhow!("Cannot read service response"))?
                {
                    if bytes.len() + chunk.len() > 24 * 1024 * 1024 {
                        bail!("Service response exceeds 24 MiB");
                    }
                    bytes.extend_from_slice(&chunk);
                }
                (status, bytes)
            }
        };
        if !(200..300).contains(&status) {
            bail!("API request failed with HTTP {status}");
        }
        serde_json::from_slice(&bytes).context("Service returned invalid JSON")
    }
}
