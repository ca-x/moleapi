//! Compare a checked ZooKeeper metadata read with Apache's official MetadataReport.
use anyhow::{Result, ensure};
use moleapi_core::{DubboConfig, DubboRegistryAuth, NetworkPolicy};
use moleapi_protocols::dubbo::preview_zookeeper_metadata;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let endpoint = args.next().expect("metadata center endpoint");
    let expected = args.next().expect("original Apache metadata document");
    let secure = match args.next() {
        None => false,
        Some(value) if value == "--auth" => true,
        Some(_) => anyhow::bail!("Unexpected metadata fixture argument"),
    };
    ensure!(
        args.next().is_none(),
        "Unexpected metadata fixture argument"
    );
    let config = DubboConfig {
        service: "fixture.RegistryService".into(),
        version: "1.0.0".into(),
        group: "qa".into(),
        ..Default::default()
    };
    let auth = secure.then(|| DubboRegistryAuth {
        username: "fixture-reader".into(),
        password: "fixture-password".into(),
    });
    if secure {
        ensure!(
            preview_zookeeper_metadata(
                &endpoint,
                &config,
                "moleapi-metadata-qa",
                None,
                NetworkPolicy {
                    allow_private_network: true
                },
                Duration::from_secs(5)
            )
            .await
            .is_err(),
            "Anonymous read accessed protected metadata"
        );
        let wrong = DubboRegistryAuth {
            username: "fixture-reader".into(),
            password: "incorrect-password".into(),
        };
        ensure!(
            preview_zookeeper_metadata(
                &endpoint,
                &config,
                "moleapi-metadata-qa",
                Some(&wrong),
                NetworkPolicy {
                    allow_private_network: true
                },
                Duration::from_secs(5)
            )
            .await
            .is_err(),
            "Incorrect digest password accessed protected metadata"
        );
    }
    let preview = preview_zookeeper_metadata(
        &endpoint,
        &config,
        "moleapi-metadata-qa",
        auth.as_ref(),
        NetworkPolicy {
            allow_private_network: true,
        },
        Duration::from_secs(5),
    )
    .await?;
    ensure!(
        preview.raw_document == expected,
        "Rust metadata differs from Apache MetadataReport"
    );
    ensure!(
        preview.methods.len() == 1
            && preview.methods[0].name == "echo"
            && preview.methods[0].parameter_types == ["java.lang.String"]
            && preview.methods[0].return_type == "java.lang.String",
        "Metadata method mapping differs from provider definition"
    );
    ensure!(
        preview_zookeeper_metadata(
            &endpoint,
            &config,
            "moleapi-metadata-qa",
            auth.as_ref(),
            NetworkPolicy {
                allow_private_network: false
            },
            Duration::from_secs(5)
        )
        .await
        .is_err(),
        "Hosted policy admitted private metadata center"
    );
    println!("PASS official Apache metadata document -> checked Rust preview");
    Ok(())
}
