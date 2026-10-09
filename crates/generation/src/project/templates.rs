//! Explicit template overrides; upstream Mustache owns parsing and rendering.
use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use std::{collections::BTreeSet, path::Path};
pub const TEMPLATE_LIMIT: usize = 512 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateFile {
    pub path: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
}
impl TemplateFile {
    fn bytes(&self) -> Result<Vec<u8>> {
        let bytes = match self.encoding.as_deref().unwrap_or("utf8") {
            "utf8" => {
                ensure!(
                    self.content.len() <= 64 * 1024,
                    "Template file limit exceeded"
                );
                self.content.as_bytes().to_vec()
            }
            "base64" => {
                ensure!(self.content.len() <= 87384, "Template file limit exceeded");
                STANDARD.decode(&self.content)?
            }
            _ => anyhow::bail!("Unknown template file encoding"),
        };
        ensure!(bytes.len() <= 64 * 1024, "Template file limit exceeded");
        Ok(bytes)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateBundle {
    pub format: String,
    pub files: Vec<TemplateFile>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub outputs: BTreeMap<String, TemplateOutput>,
}
#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum TemplateKind {
    #[serde(rename = "API")]
    Api,
    #[serde(rename = "APIDocs")]
    ApiDocs,
    #[serde(rename = "APITests")]
    ApiTests,
    Model,
    ModelDocs,
    ModelTests,
    #[default]
    SupportingFiles,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateOutput {
    #[serde(default, rename = "templateType")]
    pub template_type: TemplateKind,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub folder: String,
    #[serde(
        default,
        rename = "destinationFilename",
        skip_serializing_if = "Option::is_none"
    )]
    pub destination_filename: Option<String>,
}
impl TemplateBundle {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.format == "moleapi-codegen-templates-v1",
            "Unknown template bundle format"
        );
        ensure!(
            !self.files.is_empty() && self.files.len() <= 128,
            "Template file count limit exceeded"
        );
        ensure!(
            serde_json::to_vec(self)?.len() <= TEMPLATE_LIMIT,
            "Template bundle exceeds 512 KiB"
        );
        let mut paths = BTreeSet::new();
        for file in &self.files {
            template_path(&file.path)?;
            let _ = file.bytes()?;
            ensure!(paths.insert(file.path.as_str()), "Duplicate template path");
            if file.path.ends_with(".mustache") {
                ensure!(
                    file.encoding.as_deref().unwrap_or("utf8") == "utf8"
                        && !file.content.contains('\0'),
                    "Mustache templates require UTF-8 text"
                );
                let partials = mustache::template_partials(&file.content)
                    .map_err(|_| anyhow::anyhow!("Invalid Mustache template"))?;
                for partial in partials {
                    template_path(&partial)?;
                }
            } else {
                ensure!(self.outputs.get(&file.path).is_some_and(|output| output.template_type == TemplateKind::SupportingFiles), "Static assets require an explicit SupportingFiles mapping");
            }
        }
        for path in &paths {
            for (index, _) in path.match_indices('/') {
                ensure!(
                    !paths.contains(&path[..index]),
                    "Template file/directory collision"
                );
            }
        }
        ensure!(
            self.outputs.len() <= 128,
            "Template output count limit exceeded"
        );
        let mut destinations = BTreeSet::new();
        for (source, output) in &self.outputs {
            ensure!(
                paths.contains(source.as_str()),
                "Output mapping source must be explicitly supplied"
            );
            if !output.folder.is_empty() {
                template_path(&output.folder)?;
                ensure!(
                    output.template_type == TemplateKind::SupportingFiles,
                    "Only supporting files expose output folders"
                );
            }
            let filename = output
                .destination_filename
                .as_deref()
                .unwrap_or(source.as_str());
            template_path(filename)?;
            ensure!(
                output.destination_filename.is_none() || !filename.contains('/'),
                "Output filename must be a single segment"
            );
            ensure!(
                !(source.ends_with(".mustache") && output.destination_filename.is_none()),
                "Rendered output mappings require a destination filename or suffix"
            );
            let path = if output.folder.is_empty() {
                filename.to_string()
            } else {
                format!("{}/{filename}", output.folder)
            };
            ensure!(
                ![
                    "moleapi-generation.json",
                    "moleapi-templates.json",
                    "moleapi-regeneration.json"
                ]
                .iter()
                .any(|reserved| path == *reserved || path.starts_with(&format!("{reserved}/"))),
                "Output mapping cannot replace application manifests"
            );
            ensure!(
                destinations.insert((output.template_type.clone(), path)),
                "Duplicate template output mapping"
            );
        }
        for (kind, path) in &destinations {
            if *kind == TemplateKind::SupportingFiles {
                for (index, _) in path.match_indices('/') {
                    ensure!(
                        !destinations
                            .contains(&(TemplateKind::SupportingFiles, path[..index].to_string())),
                        "Supporting output file/directory collision"
                    );
                }
            }
        }
        Ok(())
    }
    pub(super) fn write(&self, root: &Path) -> Result<()> {
        self.validate()?;
        std::fs::create_dir(root)?;
        for file in &self.files {
            let path = root.join(&file.path);
            std::fs::create_dir_all(path.parent().unwrap())?;
            std::fs::write(path, file.bytes()?)?;
        }
        Ok(())
    }
    /// Privacy screening uses decoded copies, while the canonical bundle stays unchanged.
    pub fn privacy_value(&self) -> Result<Value> {
        let mut value = serde_json::to_value(self)?;
        for (file, node) in self
            .files
            .iter()
            .zip(value["files"].as_array_mut().unwrap())
        {
            if file.encoding.as_deref() == Some("base64") {
                node["encoding"] = "utf8".into();
                node["content"] = String::from_utf8_lossy(&file.bytes()?).to_string().into();
            }
        }
        Ok(value)
    }
}
fn template_path(path: &str) -> Result<()> {
    artifact::safe_path(path)?;
    ensure!(
        path.len() <= 256
            && path
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || "_./-".contains(ch)),
        "Unsafe template or partial path"
    );
    Ok(())
}
pub fn supports_templates(target: &str) -> bool {
    !target.starts_with("model-")
        && !is_protobuf_target(target)
        && !matches!(target, "rust-progenitor" | "rust-progenitor-cli")
}
pub(super) fn validate(input: &ProjectInput) -> Result<()> {
    if let Some(templates) = &input.templates {
        ensure!(
            supports_templates(&input.target),
            "This native generator does not expose Mustache templates; select an OpenAPI Generator target"
        );
        templates.validate()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_mappings_require_supplied_sources_and_confined_distinct_paths() {
        let mut input = bundle("{{appName}}");
        input.outputs.insert(
            "model.mustache".into(),
            TemplateOutput {
                template_type: TemplateKind::SupportingFiles,
                folder: "extras".into(),
                destination_filename: Some("result.md".into()),
            },
        );
        assert!(input.validate().is_ok());
        for folder in ["../outside", "/outside", "moleapi-generation.json"] {
            input.outputs.get_mut("model.mustache").unwrap().folder = folder.into();
            assert!(input.validate().is_err(), "{folder}");
        }
        input
            .outputs
            .get_mut("model.mustache")
            .unwrap()
            .folder
            .clear();
        for filename in ["../outside", "nested/file.md", "moleapi-templates.json"] {
            input
                .outputs
                .get_mut("model.mustache")
                .unwrap()
                .destination_filename = Some(filename.into());
            assert!(input.validate().is_err(), "{filename}");
        }
        input.outputs.clear();
        input
            .outputs
            .insert("missing.mustache".into(), TemplateOutput::default());
        assert!(input.validate().is_err());
        let raw = TemplateBundle {
            format: "moleapi-codegen-templates-v1".into(),
            files: vec![TemplateFile {
                encoding: None,
                path: "AUTHORS.md".into(),
                content: "{{ literal static content }}".into(),
            }],
            outputs: BTreeMap::from([("AUTHORS.md".into(), TemplateOutput::default())]),
        };
        assert!(raw.validate().is_ok());
    }
    fn bundle(source: &str) -> TemplateBundle {
        TemplateBundle {
            format: "moleapi-codegen-templates-v1".into(),
            outputs: BTreeMap::new(),
            files: vec![TemplateFile {
                encoding: None,
                path: "model.mustache".into(),
                content: source.into(),
            }],
        }
    }
    #[test]
    fn mature_parser_checks_partials_in_sections_and_changed_delimiters_without_loading_files() {
        assert!(
            bundle("{{#models}}{{>common/header}}{{/models}}")
                .validate()
                .is_ok()
        );
        for source in [
            "{{>../../etc/passwd}}",
            "{{#models}}{{>/etc/passwd}}{{/models}}",
            "{{=<% %>=}}<%>../private%>",
            "{{#unclosed}}",
            "{{>https://private.invalid/a}}",
        ] {
            assert!(bundle(source).validate().is_err(), "{source}");
        }
        let mut input = bundle("{{classname}}");
        input.files.push(input.files[0].clone());
        assert!(input.validate().is_err());
        input.files.truncate(1);
        input.files[0].path = "../model.mustache".into();
        assert!(input.validate().is_err());
        assert!(
            bundle(&("{{#a}}".repeat(65) + &"{{/a}}".repeat(65)))
                .validate()
                .is_err()
        );
    }
}
