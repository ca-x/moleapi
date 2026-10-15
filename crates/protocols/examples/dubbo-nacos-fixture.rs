//! Compare checked Nacos HTTP discovery with Apache Dubbo's official registry lookup.
use anyhow::{Result, ensure};
use moleapi_core::{DubboConfig, NetworkPolicy, RequestSpec};
use moleapi_protocols::dubbo::{DubboRuntime, lookup_nacos, select_registry_provider};
use serde_json::{Value, json};
use std::{path::Path, time::Duration};

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let official = args.next().expect("official Apache Nacos provider URL");
    let registry = args.next().expect("Nacos endpoint");
    let java = args.next().expect("Java executable");
    let jar = args.next().expect("pinned worker JAR");
    let namespace = args.next().unwrap_or_default();
    ensure!(
        matches!(namespace.as_str(), "" | "moleapi-qa"),
        "Unexpected fixture namespace"
    );
    ensure!(args.next().is_none(), "Unexpected Nacos fixture argument");
    let config = DubboConfig {
        source_version: 2,
        source: "nacos".into(),
        registry_namespace: namespace.clone(),
        service: "fixture.RegistryService".into(),
        version: "1.0.0".into(),
        group: "qa".into(),
        method: "echo".into(),
        ..Default::default()
    };
    let policy = NetworkPolicy {
        allow_private_network: true,
    };
    if !namespace.is_empty() {
        let mut public = config.clone();
        public.registry_namespace.clear();
        ensure!(
            lookup_nacos(&registry, &public, policy, Duration::from_secs(5))
                .await?
                .is_empty(),
            "Namespaced provider leaked into the public Nacos namespace"
        );
    } else {
        let mut custom = config.clone();
        custom.registry_namespace = "moleapi-qa".into();
        ensure!(
            lookup_nacos(&registry, &custom, policy, Duration::from_secs(5))
                .await?
                .is_empty(),
            "Public provider leaked into the custom Nacos namespace"
        );
    }
    let sources = lookup_nacos(&registry, &config, policy, Duration::from_secs(8)).await?;
    ensure!(
        sources == [official],
        "Nacos instance-to-provider mapping differs from Apache lookup: {sources:?}"
    );
    let provider =
        select_registry_provider(&config, &sources, policy, Duration::from_secs(5)).await?;
    let request: RequestSpec = serde_json::from_value(json!({
      "id":"nacos-qa","name":"Nacos QA","method":"GET","url":registry,"description":"",
      "query":[],"headers":[],"body_kind":"none","body":"",
      "auth":{"kind":"none","token":"","username":"","password":""},
      "timeout_ms":10000,"follow_redirects":false,"verify_tls":true,"assertions":[],"examples":[],
      "protocol":{"kind":"dubbo","source_version":2,"source":"nacos","transport":"classic","serialization":"hessian2",
        "service":config.service,"method":"echo","version":"1.0.0","group":"qa",
        "registry_namespace":namespace,
        "arguments":[{"id":"name","type_name":"java.lang.String","value_source":"\"from-rust\"","secret":false}],
        "attachments":[]}
    }))?;
    let runtime = DubboRuntime::new(Path::new(&jar))?;
    let answer = runtime.invoke(Path::new(&java), &request, policy).await?;
    ensure!(
        answer.status == 200 && serde_json::from_str::<Value>(&answer.body)? == "nacos:from-rust",
        "Nacos product route did not answer"
    );
    ensure!(
        answer
            .logs
            .iter()
            .any(|log| log.message == format!("Nacos → {}", provider.destination())),
        "Nacos provider address missing from response console"
    );
    let hosted = NetworkPolicy {
        allow_private_network: false,
    };
    ensure!(
        lookup_nacos(&request.url, &config, hosted, Duration::from_secs(5))
            .await
            .is_err(),
        "Hosted policy admitted private Nacos registry"
    );
    ensure!(
        runtime
            .invoke(Path::new(&java), &request, hosted)
            .await
            .is_err(),
        "Hosted policy admitted private Nacos registry route"
    );
    println!("PASS official Nacos provider URL -> checked HTTP snapshot -> product Dubbo RPC");
    Ok(())
}
