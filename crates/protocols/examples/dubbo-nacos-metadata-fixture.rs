//! Compare a checked Nacos ConfigService read with Apache's official metadata report.
use anyhow::{Result, ensure};
use moleapi_core::{DubboConfig, DubboRegistryAuth, NetworkPolicy};
use moleapi_protocols::dubbo::preview_nacos_metadata;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let endpoint = args.next().expect("Nacos metadata center endpoint");
    let expected = args.next().expect("original Apache metadata document");
    ensure!(
        args.next().is_none(),
        "Unexpected metadata fixture argument"
    );
    let config = DubboConfig {
        service: "fixture.MetadataService".into(),
        version: "1.0.0".into(),
        group: "qa".into(),
        ..Default::default()
    };
    let preview = preview_nacos_metadata(
        &endpoint,
        &config,
        "moleapi-nacos-metadata-qa",
        NetworkPolicy {
            allow_private_network: true,
        },
        Duration::from_secs(5),
    )
    .await?;
    ensure!(
        preview.raw_document == expected,
        "Nacos metadata differs from Apache report"
    );
    ensure!(
        preview.metadata_path
            == "fixture.MetadataService:1.0.0:qa:provider:moleapi-nacos-metadata-qa",
        "Nacos metadata dataId mismatch"
    );
    ensure!(
        preview.methods.len() == 1
            && preview.methods[0].name == "exchange"
            && preview.methods[0].parameter_types == ["fixture.MetadataProfile"]
            && preview.methods[0].return_type == "fixture.MetadataProfile",
        "Nacos metadata method mapping mismatch"
    );
    let profile = preview
        .types
        .iter()
        .find(|row| row.type_name == "fixture.MetadataProfile");
    ensure!(
        profile.is_some_and(|row| row
            .properties
            .iter()
            .any(|field| field.name == "name" && field.type_name == "java.lang.String")),
        "Nacos metadata DTO mapping mismatch"
    );
    let mut wrong_group = config.clone();
    wrong_group.group = "other".into();
    ensure!(
        preview_nacos_metadata(
            &endpoint,
            &wrong_group,
            "moleapi-nacos-metadata-qa",
            NetworkPolicy {
                allow_private_network: true,
            },
            Duration::from_secs(5),
        )
        .await
        .is_err(),
        "Nacos metadata lookup ignored the service group"
    );
    ensure!(
        preview_nacos_metadata(
            &endpoint,
            &config,
            "moleapi-nacos-metadata-qa",
            NetworkPolicy {
                allow_private_network: false
            },
            Duration::from_secs(5),
        )
        .await
        .is_err(),
        "Hosted policy admitted a private metadata center"
    );
    let mut unsupported = config.clone();
    unsupported.metadata_auth = Some(DubboRegistryAuth {
        username: "reader".into(),
        password: "secret".into(),
    });
    ensure!(
        preview_nacos_metadata(
            &endpoint,
            &unsupported,
            "moleapi-nacos-metadata-qa",
            NetworkPolicy {
                allow_private_network: true,
            },
            Duration::from_secs(5),
        )
        .await
        .is_err(),
        "Unsupported Nacos metadata credentials were accepted"
    );
    println!("PASS official Nacos metadata -> checked Rust preview");
    Ok(())
}
