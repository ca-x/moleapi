//! Mature SDK/server emitters produce isolated, bounded artifacts.
mod artifact;
mod import;
pub use import::*;
mod models;
mod native;
pub use models::ModelOption;
mod protobuf;
mod protoc;
mod regeneration;
pub use regeneration::*;
mod reference_policy;
mod runtime;
mod source;
mod source_bundle;
use anyhow::{Result, ensure};
pub use artifact::*;
pub use native::dispatch_project_worker;
pub use runtime::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub use source::*;
pub use source_bundle::*;
use std::collections::BTreeMap;
pub const SPEC_LIMIT: usize = 1024 * 1024;
pub const FILE_LIMIT: usize = 4 * 1024 * 1024;
pub const PROJECT_LIMIT: usize = 8 * 1024 * 1024;
pub const IPC_LIMIT: usize = 20 * 1024 * 1024;
pub const JAR_SHA256: &str = "5ccb60c4678901715376c6114afba60c28d0844fe506bd1e02891cbc363ab9ff";
pub const JAR_VERSION: &str = "7.26.0";
const JAR: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/openapi-generator.jar"));
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectInput {
    pub specification: Value,
    pub target: String,
    #[serde(default)]
    pub options: BTreeMap<String, Value>,
    #[serde(default)]
    pub include_secrets: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectTarget {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub kind: String,
    pub upstream_stability: String,
    pub validation: String,
    #[serde(default)]
    pub options: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub model_options: Vec<ModelOption>,
}
pub fn project_catalog() -> Result<Vec<ProjectTarget>> {
    let manifest: Value = serde_json::from_str(include_str!(
        "../../../../vendor/openapi-generator/manifest.json"
    ))?;
    let mut targets: Vec<ProjectTarget> = serde_json::from_value(manifest["targets"].clone())?;
    for target in &mut targets {
        if matches!(target.id.as_str(), "mysql-schema" | "postgresql-schema") {
            target.kind = "model".into();
            target.title = Some(
                if target.id == "mysql-schema" {
                    "SQL (MySQL)"
                } else {
                    "SQL (PostgreSQL)"
                }
                .into(),
            );
        }
        if target.options.get("npmName") == Some(&Value::Null) {
            target.options.insert(
                "npmName".into(),
                Value::String("moleapi-generated-project".into()),
            );
        }
    }
    targets.insert(
        0,
        ProjectTarget {
            id: "rust-progenitor".into(),
            title: None,
            model_options: Vec::new(),
            kind: "client".into(),
            upstream_stability: "stable".into(),
            validation: "representative-compile-call-fixture".into(),
            options: BTreeMap::from([
                ("packageName".into(), Value::String("moleapi_sdk".into())),
                ("packageVersion".into(), Value::String("0.1.0".into())),
                ("interface".into(), Value::String("positional".into())),
            ]),
        },
    );
    targets.insert(
        1,
        ProjectTarget {
            id: "rust-progenitor-cli".into(),
            title: None,
            model_options: Vec::new(),
            kind: "cli".into(),
            upstream_stability: "stable".into(),
            validation: "implementation-awaiting-cli-fixtures".into(),
            options: BTreeMap::from([
                ("packageName".into(), Value::String("moleapi_cli".into())),
                ("packageVersion".into(), Value::String("0.1.0".into())),
            ]),
        },
    );
    targets.insert(
        2,
        ProjectTarget {
            id: "rust-tonic".into(),
            title: None,
            model_options: Vec::new(),
            kind: "protobuf".into(),
            upstream_stability: "stable".into(),
            validation: "awaiting-protobuf-fixtures".into(),
            options: BTreeMap::from([
                ("packageName".into(), Value::String("moleapi_grpc".into())),
                ("packageVersion".into(), Value::String("0.1.0".into())),
            ]),
        },
    );
    for language in [
        "cpp", "csharp", "java", "kotlin", "objc", "php", "python", "ruby",
    ] {
        targets.push(ProjectTarget {
            id: format!("protobuf-{language}"),
            title: None,
            model_options: Vec::new(),
            kind: "protobuf".into(),
            upstream_stability: "stable".into(),
            validation: "compiler-integration-awaiting-fixtures".into(),
            options: BTreeMap::new(),
        });
    }
    targets.splice(3..3, models::catalog()?);
    Ok(targets)
}
pub(crate) fn validate_options(input: &ProjectInput) -> Result<()> {
    if input.target.starts_with("model-") {
        return models::validate(input);
    }
    ensure!(
        project_catalog()?.iter().any(|t| t.id == input.target),
        "Unknown project generator"
    );
    ensure!(input.options.len() <= 16, "Generator option limit exceeded");
    let target = project_catalog()?
        .into_iter()
        .find(|t| t.id == input.target)
        .unwrap();
    for (key, value) in &input.options {
        ensure!(
            target.options.contains_key(key),
            "This generator does not expose the selected option"
        );
        ensure!(
            [
                "packageName",
                "packageVersion",
                "apiPackage",
                "modelPackage",
                "invokerPackage",
                "artifactId",
                "artifactVersion",
                "library",
                "namespace",
                "interface",
                "npmName",
                "npmVersion",
                "groupId",
            ]
            .contains(&key.as_str()),
            "Unsupported or unsafe generator option"
        );
        if let Some(text) = value.as_str() {
            ensure!(
                !text.is_empty() && text.len() <= 128 && !text.contains(".."),
                "Invalid generator naming/library option"
            );
            let qualified = matches!(
                key.as_str(),
                "packageName"
                    | "apiPackage"
                    | "modelPackage"
                    | "invokerPackage"
                    | "namespace"
                    | "groupId"
            );
            if qualified {
                let normalized = if key == "namespace" {
                    text.replace("::", ".")
                } else {
                    text.into()
                };
                ensure!(
                    normalized.split(['.', '\\']).all(|part| part
                        .starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
                        && part
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')),
                    "Invalid package/namespace segments"
                );
            } else {
                ensure!(
                    text.chars()
                        .all(|c| c.is_ascii_alphanumeric() || "_.-+".contains(c)),
                    "Invalid version/library/interface option"
                );
            }
        } else {
            ensure!(
                value.is_boolean(),
                "Generator options require bounded strings or booleans"
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;

pub fn is_protobuf_target(target: &str) -> bool {
    target == "rust-tonic" || target.starts_with("protobuf-")
}
