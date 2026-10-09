//! Generate trusted model fixtures through the same capped application worker.
use anyhow::Result;
use moleapi_generation::project::{ProjectInput, ProjectRuntime, project_catalog};
use serde_json::json;
use std::{collections::BTreeMap, path::PathBuf};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let output = PathBuf::from(args.next().expect("output directory"));
    let worker = PathBuf::from(args.next().expect("absolute application worker"));
    let runtime = ProjectRuntime::new(worker, args.next().map(PathBuf::from))?;
    let specification = json!({"openapi":"3.0.3","info":{"title":"Trusted models","version":"1"},"paths":{},"components":{"schemas":{"Pet":{"type":"object","required":["id","status"],"properties":{"id":{"type":"integer"},"status":{"type":"string","enum":["ready","done"]},"note":{"type":"string","nullable":true},"friend":{"$ref":"#/components/schemas/Pet"}}}}}});
    for target in project_catalog()?
        .into_iter()
        .filter(|target| target.kind == "model")
    {
        if !target.id.starts_with("model-") && !runtime.java_available() {
            continue;
        }
        let artifact = runtime
            .generate(
                ProjectInput {
                    specification: specification.clone(),
                    target: target.id.clone(),
                    options: BTreeMap::new(),
                    include_secrets: false,
                    templates: None,
                },
                CancellationToken::new(),
            )
            .await?;
        let directory = output.join(&target.id);
        std::fs::create_dir_all(&directory)?;
        for file in artifact.files {
            if file.encoding != "utf8" {
                anyhow::bail!("Unexpected binary model fixture");
            }
            let path = directory.join(file.path);
            std::fs::create_dir_all(path.parent().unwrap())?;
            std::fs::write(path, file.content)?;
        }
        println!("PASS {}", target.id);
    }
    Ok(())
}
