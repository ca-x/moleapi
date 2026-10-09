//! Portable request generation. Scalar/Postman own emitters; QuickJS has no host APIs.
pub mod project;
mod request;
use anyhow::{Result, ensure};
pub use request::*;
use rquickjs::{Context, Function, Runtime};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

pub const ENGINE: &str = "@scalar/snippetz@0.10.5 + postman-code-generators@2.1.1";
pub const INPUT_LIMIT: usize = 512 * 1024;
pub const OUTPUT_LIMIT: usize = 2 * 1024 * 1024;
const BUNDLE: &str = include_str!("../../../vendor/snippet-engine/engine.js");
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Client {
    pub client: String,
    pub title: String,
    #[serde(default)]
    pub binary_file: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Target {
    pub target: String,
    pub title: String,
    pub clients: Vec<Client>,
}
fn invoke(function: &str, input: Option<&str>) -> Result<String> {
    ensure!(
        input.is_none_or(|s| s.len() <= INPUT_LIMIT),
        "Snippet input exceeds 512 KiB"
    );
    let runtime = Runtime::new()?;
    runtime.set_memory_limit(64 * 1024 * 1024);
    runtime.set_max_stack_size(512 * 1024);
    // Loading the fixed, integrity-checked emitters is separate from processing
    // untrusted requests; concurrent hosts must not spend the request budget here.
    let deadline = Instant::now() + Duration::from_secs(2);
    runtime.set_interrupt_handler(Some(Box::new(move || Instant::now() >= deadline)));
    let context = Context::full(&runtime)?;
    context.with(|ctx| -> Result<()> {
        ctx.eval::<(), _>(BUNDLE)
            .map_err(|_| anyhow::anyhow!("Cannot initialize embedded snippet engine"))?;
        Ok(())
    })?;
    let deadline = Instant::now() + Duration::from_millis(250);
    runtime.set_interrupt_handler(Some(Box::new(move || Instant::now() >= deadline)));
    context.with(|ctx| -> Result<String> {
        let call: Function = ctx.globals().get(function)?;
        let code: String = if let Some(input) = input {
            call.call((input,))
        } else {
            call.call(())
        }
        .map_err(|_| {
            anyhow::anyhow!("Snippet generation failed or exceeded its execution budget")
        })?;
        ensure!(code.len() <= OUTPUT_LIMIT, "Generated code exceeds 2 MiB");
        Ok(code)
    })
}
pub fn catalog() -> Result<Vec<Target>> {
    Ok(serde_json::from_str(&invoke(
        "moleapiSnippetCatalog",
        None,
    )?)?)
}
pub fn generate_har(target: &str, client: &str, request: &serde_json::Value) -> Result<String> {
    ensure!(
        target.len() <= 32 && client.len() <= 32,
        "Invalid snippet target/library"
    );
    let input = serde_json::to_string(
        &serde_json::json!({"target":target,"client":client,"request":request}),
    )?;
    invoke("moleapiSnippetGenerate", Some(&input))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn embedded_engine_lists_and_generates_every_upstream_plugin() {
        let targets = catalog().unwrap();
        assert_eq!(targets.len(), 23);
        for (target, clients) in [
            ("dart", vec!["dio"]),
            ("node", vec!["native", "request", "unirest"]),
            ("php", vec!["httprequest2", "pecl_http"]),
            ("r", vec!["httr", "rcurl"]),
            ("shell", vec!["curl_windows"]),
        ] {
            let target = targets.iter().find(|entry| entry.target == target).unwrap();
            for client in clients {
                assert!(target.clients.iter().any(|entry| entry.client == client));
            }
        }
        let request = serde_json::json!({"method":"POST","url":"http://127.0.0.1:18921/echo?a=1","httpVersion":"HTTP/1.1","headers":[{"name":"X-Test","value":"fixture"}],"queryString":[],"cookies":[],"postData":{"mimeType":"application/json","text":"{\"hello\":\"world\"}"}});
        for target in targets {
            for client in target.clients {
                let result = generate_har(&target.target, &client.client, &request);
                assert!(
                    result.is_ok(),
                    "{}/{}: {result:?}",
                    target.target,
                    client.client
                );
                assert!(!result.unwrap().is_empty());
            }
        }
    }
    #[test]
    fn invalid_target_and_limits_never_echo_request() {
        let request =
            serde_json::json!({"url":"http://secret.invalid","postData":{"text":"private"}});
        let error = generate_har("missing", "missing", &request)
            .unwrap_err()
            .to_string();
        assert!(!error.contains("private") && !error.contains("secret.invalid"));
        assert!(invoke("moleapiSnippetGenerate", Some(&"x".repeat(INPUT_LIMIT + 1))).is_err());
    }
}
