//! Trusted SDK/server validation inputs; generated programs run only in explicit QA tools.
use anyhow::{Context, Result, ensure};
use base64::Engine;
use moleapi_generation::project::{ProjectInput, ProjectRuntime};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf};
use tokio_util::sync::CancellationToken;

fn specification(version: &str) -> Value {
    let mut note = json!({"type":"string","nullable":true});
    if version == "3.1.0" {
        note = json!({"type":["string","null"]});
    }
    json!({"openapi":version,"info":{"title":"Runtime fixture","version":"1"},
    "security":[{"bearerAuth":[]}],
    "paths":{
        "/pets/{id}":{
            "parameters":[{"name":"id","in":"path","required":true,"schema":{"type":"integer","format":"int32"}}],
            "get":{"operationId":"getPet","parameters":[
                {"name":"q","in":"query","required":true,"schema":{"type":"string"}},
                {"name":"X-Test","in":"header","required":true,"schema":{"type":"string"}}
            ],"responses":{"200":{"description":"Pet","content":{"application/json":{"schema":{"$ref":"#/components/schemas/Pet"}}}}}},
            "post":{"operationId":"replacePet","requestBody":{"required":true,"content":{"application/json":{"schema":{"$ref":"#/components/schemas/Pet"}}}},
                "responses":{"200":{"description":"Pet","content":{"application/json":{"schema":{"$ref":"#/components/schemas/Pet"}}}}}}
        },
        "/fail":{"get":{"operationId":"failGet","responses":{"400":{"description":"Expected failure","content":{"application/json":{"schema":{"$ref":"#/components/schemas/Failure"}}}}}}}
    },
    "components":{"securitySchemes":{"bearerAuth":{"type":"http","scheme":"bearer"}},"schemas":{
        "Pet":{"type":"object","required":["id","status"],"properties":{"id":{"type":"integer","format":"int32"},"status":{"type":"string","enum":["ready","done"]},"note":note}},
        "Failure":{"type":"object","required":["message"],"properties":{"message":{"type":"string"}}}
    }}})
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let output = PathBuf::from(args.next().context("Output directory required")?);
    let worker = PathBuf::from(
        args.next()
            .context("Absolute application worker required")?,
    );
    let java = PathBuf::from(args.next().context("Absolute Java executable required")?);
    let targets = args.next().context("Comma-separated targets required")?;
    let versions = args.next().unwrap_or_else(|| "3.0.3,3.1.0".into());
    let runtime = ProjectRuntime::new(worker, Some(java))?;
    for version in versions.split(',') {
        ensure!(
            matches!(version, "3.0.3" | "3.1.0"),
            "Unknown fixture dialect"
        );
        for target in targets.split(',') {
            ensure!(
                [
                    "javascript",
                    "python",
                    "go",
                    "typescript-fetch",
                    "python-fastapi"
                ]
                .contains(&target),
                "Unknown fixture target"
            );
            let input = ProjectInput {
                target: target.into(),
                specification: specification(version),
                options: BTreeMap::new(),
                include_secrets: false,
                templates: None,
            };
            let artifact = runtime
                .generate(input.clone(), CancellationToken::new())
                .await
                .with_context(|| format!("{target}/{version}"))?;
            let directory = output.join(version).join(target);
            ensure!(!directory.exists(), "Fixture output already exists");
            std::fs::create_dir_all(&directory)?;
            for file in artifact.files {
                let bytes = if file.encoding == "base64" {
                    base64::engine::general_purpose::STANDARD.decode(&file.content)?
                } else {
                    file.content.into_bytes()
                };
                let path = directory.join(&file.path);
                std::fs::create_dir_all(path.parent().unwrap())?;
                std::fs::write(path, bytes)?;
            }
            std::fs::write(
                directory.join("fixture-source.json"),
                serde_json::to_vec_pretty(&input.specification)?,
            )?;
            println!("GENERATED {target}/{version}");
        }
    }
    Ok(())
}
