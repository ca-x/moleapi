//! Trusted imported protobuf QA artifacts; only explicit tools execute code generation.
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use moleapi_generation::project::{ProjectInput, ProjectRuntime};
use std::{collections::BTreeMap, path::PathBuf};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let output = PathBuf::from(args.next().context("Absolute output root required")?);
    let worker = PathBuf::from(args.next().context("Absolute worker required")?);
    let selector = args
        .next()
        .context("Explicit protoc or rust-tonic selector required")?;
    let (runtime, target) = if selector == "rust-tonic" {
        (ProjectRuntime::new(worker, None)?, "rust-tonic".to_owned())
    } else {
        let compiler = PathBuf::from(selector);
        let plugin = PathBuf::from(args.next().context("Absolute gRPC plugin required")?);
        let language = args.next().unwrap_or_else(|| "python".into());
        ensure!(
            matches!(language.as_str(), "python" | "java"),
            "Unsupported QA consumer language"
        );
        let runtime = ProjectRuntime::new(worker, None)?
            .with_protoc(Some(compiler), BTreeMap::from([(language.clone(), plugin)]))?;
        (runtime, format!("protobuf-{language}"))
    };
    ensure!(args.next().is_none(), "Unexpected argument");
    ensure!(
        output.is_absolute() && !output.exists(),
        "Use a fresh absolute QA output root"
    );
    let mut specification = serde_json::from_str(include_str!(
        "../../../tools/projects/fixtures/protobuf-imports.json"
    ))?;
    for representation in ["source", "descriptor"] {
        let artifact = runtime
            .generate(
                ProjectInput {
                    target: target.clone(),
                    specification,
                    options: BTreeMap::new(),
                    include_secrets: false,
                    templates: None,
                },
                CancellationToken::new(),
            )
            .await?;
        let root = output.join(representation);
        for file in &artifact.files {
            let bytes = if file.encoding == "base64" {
                STANDARD.decode(&file.content)?
            } else {
                file.content.as_bytes().to_vec()
            };
            let path = root.join(&file.path);
            std::fs::create_dir_all(path.parent().context("Artifact parent missing")?)?;
            std::fs::write(path, bytes)?;
        }
        let descriptor = artifact
            .files
            .iter()
            .find(|file| file.path == "descriptor-set.bin")
            .context("Descriptor artifact missing")?;
        ensure!(
            descriptor.encoding == "base64",
            "Descriptor must preserve bytes"
        );
        specification =
            serde_json::json!({"kind":"descriptor","descriptor_set_base64":descriptor.content});
        println!("GENERATED {target} {representation}");
    }
    Ok(())
}
