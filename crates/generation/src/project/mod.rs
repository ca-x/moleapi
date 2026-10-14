//! Mature SDK/server emitters produce isolated, bounded artifacts.
mod artifact;
mod discriminator;
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
mod schema_resources;
mod source;
mod source_bundle;
mod templates;
use anyhow::{Result, ensure};
pub use artifact::*;
pub use native::dispatch_project_worker;
pub use runtime::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub use source::*;
pub use source_bundle::*;
use std::collections::BTreeMap;
pub use templates::{
    TemplateBundle, TemplateFile, TemplateKind, TemplateOutput, supports_templates,
};
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub templates: Option<TemplateBundle>,
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
        // Representative runtime evidence is narrower than full target/dialect parity.
        match target.id.as_str() {
            "javascript" | "python" | "go" | "typescript-fetch" | "csharp" | "java" | "kotlin"
            | "php" | "dart" | "swift6" => {
                target.validation = "representative-3.0-3.1-compile-call-fixture".into();
            }
            "python-fastapi" | "aspnetcore" | "spring" => {
                target.validation = "representative-3.0-3.1-skeleton-start-fixture".into();
            }
            _ => {}
        }
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
        if target.id == "kotlin" {
            target
                .options
                .insert("serializationLibrary".into(), Value::String("moshi".into()));
            target
                .options
                .insert("generateOneOfAnyOfWrappers".into(), Value::Bool(false));
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
            validation: "representative-3.0-cli-compile-call-fixture".into(),
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
            validation: "representative-tonic-four-mode-source-descriptor-fixture".into(),
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
            validation: match language {
                "python" => "representative-python-four-mode-configured-plugin-fixture",
                "java" => "representative-java-four-mode-configured-plugin-fixture",
                "csharp" => "representative-csharp-four-mode-configured-plugin-fixture",
                "kotlin" => "representative-kotlin-java-four-mode-configured-plugins-fixture",
                "ruby" => "representative-ruby-four-mode-configured-plugin-fixture",
                "cpp" => "representative-cpp-four-mode-configured-plugin-fixture",
                "php" => "representative-php-four-mode-client-configured-plugin-fixture",
                _ => "representative-protoc-message-source-descriptor-fixture",
            }
            .into(),
            options: BTreeMap::new(),
        });
    }
    targets.splice(3..3, models::catalog()?);
    Ok(targets)
}
pub(crate) fn validate_options(input: &ProjectInput) -> Result<()> {
    templates::validate(input)?;
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
                "serializationLibrary",
                "generateOneOfAnyOfWrappers",
            ]
            .contains(&key.as_str()),
            "Unsupported or unsafe generator option"
        );
        if key == "generateOneOfAnyOfWrappers" {
            ensure!(
                value.is_boolean(),
                "Union wrapper option requires a boolean"
            );
        } else if key == "serializationLibrary" {
            ensure!(
                matches!(
                    value.as_str(),
                    Some("moshi" | "gson" | "jackson" | "kotlinx_serialization")
                ),
                "Unsupported Kotlin serialization library"
            );
        }
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
    if input.target == "kotlin" {
        let wrappers = input
            .options
            .get("generateOneOfAnyOfWrappers")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let library = input
            .options
            .get("library")
            .and_then(Value::as_str)
            .unwrap_or("jvm-okhttp4");
        let serializer = input
            .options
            .get("serializationLibrary")
            .and_then(Value::as_str)
            .unwrap_or("moshi");
        if wrappers {
            ensure!(
                library == "jvm-retrofit2"
                    && matches!(serializer, "gson" | "kotlinx_serialization"),
                "Kotlin union wrappers require jvm-retrofit2 with gson or kotlinx_serialization"
            );
        }
        let nodes = schema_resources::openapi_schema_nodes(&input.specification);
        fn has_union(value: &Value, nodes: &std::collections::HashSet<usize>) -> bool {
            if nodes.contains(&(std::ptr::from_ref(value) as usize))
                && ["oneOf", "anyOf"].iter().any(|key| {
                    value
                        .get(key)
                        .and_then(Value::as_array)
                        .is_some_and(|items| !items.is_empty())
                })
            {
                return true;
            }
            match value {
                Value::Object(object) => object.values().any(|child| has_union(child, nodes)),
                Value::Array(array) => array.iter().any(|child| has_union(child, nodes)),
                _ => false,
            }
        }
        ensure!(
            !has_union(&input.specification, &nodes) || wrappers,
            "Kotlin oneOf/anyOf schemas require explicit union wrappers and a supported library/serializer combination"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests;

pub fn is_protobuf_target(target: &str) -> bool {
    target == "rust-tonic" || target.starts_with("protobuf-")
}
