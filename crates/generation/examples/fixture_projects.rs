//! Trusted SDK/server validation inputs; generated programs run only in explicit QA tools.
use anyhow::{Context, Result, ensure};
use base64::Engine;
use moleapi_generation::project::{ProjectInput, ProjectRuntime};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf};
use tokio_util::sync::CancellationToken;

fn specification(version: &str, profile: &str, discriminator_property: &str) -> Value {
    let mut note = json!({"type":"string","nullable":true});
    if version == "3.1.0" {
        note = json!({"type":["string","null"]});
    }
    let mut specification = json!({"openapi":version,"info":{"title":"Runtime fixture","version":"1"},
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
    }}});
    if matches!(profile, "binary" | "multipart") {
        specification["paths"]["/files"] = json!({
            "get":{"operationId":"downloadFile","responses":{"200":{"description":"Binary download","content":{"application/octet-stream":{"schema":{"type":"string","format":"binary"}}}}}},
            "post":{"operationId":"uploadFile","requestBody":{"required":true,"content":{"application/octet-stream":{"schema":{"type":"string","format":"binary"}}}},"responses":{"200":{"description":"Upload receipt","content":{"application/json":{"schema":{"$ref":"#/components/schemas/UploadReceipt"}}}}}}
        });
        specification["components"]["schemas"]["UploadReceipt"] = json!({"type":"object","required":["size"],"properties":{"size":{"type":"integer","format":"int32"}}});
    }
    if profile == "multipart" {
        specification["paths"]
            .as_object_mut()
            .unwrap()
            .remove("/files");
        specification["paths"]["/multipart"] = json!({"post":{
            "operationId":"uploadMultipart",
            "requestBody":{"required":true,"content":{"multipart/form-data":{
                "schema":{"type":"object","required":["file","note","tags"],"properties":{
                    "file":{"type":"string","format":"binary"},"note":{"type":"string"},"tags":{"type":"array","items":{"type":"string"}},"optionalTags":{"type":"array","items":{"type":"string"}},"packedTags":{"type":"array","items":{"type":"string"}}
                }},"encoding":{"file":{"contentType":"application/octet-stream"},"tags":{"style":"form","explode":true},"optionalTags":{"style":"form","explode":true},"packedTags":{"style":"form","explode":false}}
            }}},"responses":{"200":{"description":"Multipart receipt","content":{"application/json":{"schema":{"$ref":"#/components/schemas/UploadReceipt"}}}}}
        }});
    }
    if profile == "union" {
        specification["components"]["schemas"]["Cat"] = json!({"type":"object","required":["kind","name"],"properties":{"kind":{"type":"string","enum":["cat"]},"name":{"type":"string"}}});
        specification["components"]["schemas"]["Dog"] = json!({"type":"object","required":["kind","barks"],"properties":{"kind":{"type":"string","enum":["dog"]},"barks":{"type":"boolean"}}});
        specification["components"]["schemas"]["Animal"] = json!({"oneOf":[{"$ref":"#/components/schemas/Cat"},{"$ref":"#/components/schemas/Dog"}],"discriminator":{"propertyName":"kind","mapping":{"cat":"#/components/schemas/Cat","dog":"#/components/schemas/Dog"}}});
        specification["paths"]["/animals/{kind}"] = json!({"get":{"operationId":"getAnimal","parameters":[{"name":"kind","in":"path","required":true,"schema":{"type":"string"}}],"responses":{"200":{"description":"Tagged animal","content":{"application/json":{"schema":{"$ref":"#/components/schemas/Animal"}}}}}}});
        specification["paths"]["/animals"] = json!({"post":{"operationId":"echoAnimal","requestBody":{"required":true,"content":{"application/json":{"schema":{"$ref":"#/components/schemas/Animal"}}}},"responses":{"200":{"description":"Tagged echo","content":{"application/json":{"schema":{"$ref":"#/components/schemas/Animal"}}}}}}});
        specification["paths"]["/animal-list"] = json!({
            "get":{"operationId":"listAnimals","responses":{"200":{"description":"Animals","content":{"application/json":{"schema":{"type":"array","items":{"$ref":"#/components/schemas/Animal"}}}}}}},
            "post":{"operationId":"echoAnimalArray","requestBody":{"required":true,"content":{"application/json":{"schema":{"type":"array","items":{"$ref":"#/components/schemas/Animal"}}}}},"responses":{"200":{"description":"Animals","content":{"application/json":{"schema":{"type":"array","items":{"$ref":"#/components/schemas/Animal"}}}}}}}
        });
        specification["paths"]["/mixed"] = json!({"post":{"operationId":"mixedModel","requestBody":{"required":true,"content":{"application/json":{"schema":{"$ref":"#/components/schemas/Pet"}}}},"responses":{"200":{"description":"Mapped response","content":{"application/json":{"schema":{"$ref":"#/components/schemas/Animal"}}}}}}});
    }
    if profile == "union" && discriminator_property != "kind" {
        for name in ["Cat", "Dog"] {
            let schema = &mut specification["components"]["schemas"][name];
            let property = schema["properties"]
                .as_object_mut()
                .unwrap()
                .remove("kind")
                .unwrap();
            schema["properties"][discriminator_property] = property;
            schema["required"][0] = discriminator_property.into();
        }
        specification["components"]["schemas"]["Animal"]["discriminator"]["propertyName"] =
            discriminator_property.into();
    }
    specification
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
    let profile = args.next().unwrap_or_else(|| "core".into());
    ensure!(
        matches!(profile.as_str(), "core" | "binary" | "multipart" | "union"),
        "Unknown fixture profile"
    );
    let options: BTreeMap<String, Value> =
        serde_json::from_str(&args.next().unwrap_or_else(|| "{}".into()))?;
    let discriminator_property = args.next().unwrap_or_else(|| "kind".into());
    ensure!(
        !discriminator_property.is_empty() && discriminator_property.len() <= 128,
        "Invalid fixture discriminator property"
    );
    ensure!(args.next().is_none(), "Unexpected fixture argument");
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
                    "python-fastapi",
                    "csharp",
                    "aspnetcore",
                    "java",
                    "spring",
                    "kotlin",
                    "php",
                    "dart"
                ]
                .contains(&target),
                "Unknown fixture target"
            );
            let input = ProjectInput {
                target: target.into(),
                specification: specification(version, &profile, &discriminator_property),
                options: options.clone(),
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
