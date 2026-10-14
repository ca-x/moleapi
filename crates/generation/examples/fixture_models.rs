//! Generate trusted model fixtures through the same capped application worker.
use anyhow::{Result, ensure};
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
    let version = args.next().unwrap_or_else(|| "3.0.3".into());
    let selected = args.next().unwrap_or_else(|| "*".into());
    let profile = args.next().unwrap_or_else(|| "core".into());
    ensure!(args.next().is_none(), "Unexpected fixture argument");
    ensure!(
        profile == "core"
            || profile == "date" && selected == "model-flow"
            || matches!(profile.as_str(), "flat" | "collision") && selected == "model-elm"
            || profile == "containers" && selected == "model-pike"
            || profile == "enum-tags" && selected == "model-crystal",
        "Unknown model fixture profile"
    );
    ensure!(
        matches!(version.as_str(), "3.0.3" | "3.1.0"),
        "Unknown fixture dialect"
    );
    let catalog = project_catalog()?;
    ensure!(
        selected == "*"
            || selected.split(',').all(|id| catalog
                .iter()
                .any(|target| target.kind == "model" && target.id == id)),
        "Unknown model fixture target"
    );
    let note = if version == "3.0.3" {
        json!({"type":"string","nullable":true})
    } else {
        json!({"type":["string","null"]})
    };
    let mut specification = json!({"openapi":version,"info":{"title":"Trusted models","version":"1"},"paths":{},"components":{"schemas":{"Pet":{"type":"object","required":["id","status"],"properties":{"id":{"type":"integer"},"status":{"type":"string","enum":["ready","done"]},"note":note,"friend":{"$ref":"#/components/schemas/Pet"}}}}}});
    if profile == "date" {
        specification["components"]["schemas"]["Pet"]["properties"]["observed"] =
            json!({"type":"string","format":"date-time"});
    }
    if profile == "flat" {
        specification["components"]["schemas"]["Pet"]["properties"]
            .as_object_mut()
            .unwrap()
            .remove("friend");
    }
    if profile == "collision" {
        specification["components"]["schemas"]["StatusValue"] =
            json!({"type":"object","required":["value"],"properties":{"value":{"type":"integer"}}});
    }
    if profile == "containers" {
        let pet = json!({"$ref":"#/components/schemas/Pet"});
        let status = json!({"type":"string","enum":["ready","done"]});
        specification["components"]["schemas"]["Envelope"] = json!({"type":"object","required":["primary","pets","lookup","shades"],"properties":{
            "primary":pet.clone(),"pets":{"type":"array","items":pet.clone()},"lookup":{"type":"object","additionalProperties":pet.clone()},
            "shades":{"type":"array","items":status},"counts":{"type":"object","additionalProperties":{"type":"integer"}}
        }});
        specification["components"]["schemas"]["PetArray"] =
            json!({"type":"array","items":pet.clone()});
        specification["components"]["schemas"]["PetMap"] =
            json!({"type":"object","additionalProperties":pet});
    }
    if profile == "enum-tags" {
        specification["components"]["schemas"]["Pet"]["properties"]["status"]["enum"] =
            json!(["ready-state", "done/state", "#{1 + 2}"]);
    }
    for target in catalog.into_iter().filter(|target| {
        target.kind == "model" && (selected == "*" || selected.split(',').any(|id| id == target.id))
    }) {
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
        std::fs::write(
            directory.join("fixture-source.json"),
            serde_json::to_vec(&specification)?,
        )?;
        println!("PASS {}", target.id);
    }
    Ok(())
}
