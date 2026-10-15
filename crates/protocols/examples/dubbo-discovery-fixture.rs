//! Consume one original official ZooKeeper lookup result through Rust policy admission.
use anyhow::{Result, ensure};
use moleapi_core::{DubboConfig, NetworkPolicy, RequestSpec};
use moleapi_protocols::dubbo::{DubboRuntime, lookup_zookeeper, select_registry_provider};
use serde_json::{Value, json};
use std::{path::Path, time::Duration};

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let source = args.next().expect("original provider URL");
    let registry = args.next().expect("ZooKeeper registry URL");
    let java = args.next().expect("Java executable");
    let jar = args.next().expect("pinned worker JAR");
    ensure!(
        args.next().is_none(),
        "Unexpected discovery fixture argument"
    );
    let config = DubboConfig {
        service: "fixture.RegistryService".into(),
        version: "1.0.0".into(),
        group: "qa".into(),
        method: "echo".into(),
        ..Default::default()
    };
    let snapshot = lookup_zookeeper(
        &registry,
        &config,
        NetworkPolicy {
            allow_private_network: true,
        },
        Duration::from_secs(5),
    )
    .await?;
    ensure!(
        snapshot.len() == 1,
        "ZooKeeper returned unexpected provider count"
    );
    // Apache lookup normalizes registry metadata (for example timestamp/check).
    let raw = moleapi_core::dubbo_provider_url(&snapshot[0], &config)?;
    let official = moleapi_core::dubbo_provider_url(&source, &config)?;
    ensure!(
        raw.host_str() == official.host_str() && raw.port() == official.port(),
        "ZooKeeper provider destination differs from official lookup"
    );
    let provider = select_registry_provider(
        &config,
        &snapshot,
        NetworkPolicy {
            allow_private_network: true,
        },
        Duration::from_secs(5),
    )
    .await?;
    ensure!(
        provider.original_url() == snapshot[0],
        "Registry source changed"
    );
    let request: RequestSpec = serde_json::from_value(json!({
      "id":"registry-qa","name":"Registry QA","method":"GET",
      "url":format!("dubbo://{}",provider.destination()),"description":"",
      "query":[],"headers":[],"body_kind":"none","body":"",
      "auth":{"kind":"none","token":"","username":"","password":""},
      "timeout_ms":10000,"follow_redirects":false,"verify_tls":true,"assertions":[],"examples":[],
      "protocol":{"kind":"dubbo","source_version":1,"transport":"classic","serialization":"hessian2",
        "service":config.service,"method":"echo","version":"1.0.0","group":"qa",
        "arguments":[{"id":"name","type_name":"java.lang.String","value_source":"\"from-rust\"","secret":false}],
        "attachments":[]}
    }))?;
    let runtime = DubboRuntime::new(Path::new(&jar))?;
    let answer = runtime
        .invoke(
            Path::new(&java),
            &request,
            NetworkPolicy {
                allow_private_network: true,
            },
        )
        .await?;
    ensure!(
        answer.status == 200 && serde_json::from_str::<Value>(&answer.body)? == "echo:from-rust",
        "Selected registered provider did not answer"
    );
    let mut registry_request = request.clone();
    registry_request.url = registry.clone();
    if let moleapi_core::Protocol::Dubbo { config } = &mut registry_request.protocol {
        config.source_version = 2;
        config.source = "zookeeper".into();
    }
    let registry_answer = runtime
        .invoke(
            Path::new(&java),
            &registry_request,
            NetworkPolicy {
                allow_private_network: true,
            },
        )
        .await?;
    ensure!(
        registry_answer.status == 200
            && serde_json::from_str::<Value>(&registry_answer.body)? == "echo:from-rust",
        "Product registry route did not answer"
    );
    ensure!(
        registry_answer
            .logs
            .iter()
            .any(|entry| entry.message == format!("ZooKeeper → {}", provider.destination())),
        "Selected provider was not shown in the response console"
    );
    ensure!(
        runtime
            .invoke(
                Path::new(&java),
                &registry_request,
                NetworkPolicy {
                    allow_private_network: false
                },
            )
            .await
            .is_err(),
        "Hosted policy admitted a private ZooKeeper registry"
    );
    ensure!(
        select_registry_provider(
            &config,
            &snapshot,
            NetworkPolicy {
                allow_private_network: false
            },
            Duration::from_secs(5)
        )
        .await
        .is_err(),
        "Hosted policy admitted a private registry result"
    );
    println!("PASS original ZooKeeper provider URL -> checked Rust address -> official direct RPC");
    Ok(())
}
