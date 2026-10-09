//! Explicit template overrides; upstream Mustache owns parsing and rendering.
use super::*;
use std::{collections::BTreeSet, path::Path};
pub const TEMPLATE_LIMIT: usize = 512 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateFile {
    pub path: String,
    pub content: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateBundle {
    pub format: String,
    pub files: Vec<TemplateFile>,
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
            ensure!(
                file.path.ends_with(".mustache"),
                "Template files must use .mustache"
            );
            ensure!(
                file.content.len() <= 64 * 1024 && !file.content.contains('\0'),
                "Template file limit exceeded"
            );
            ensure!(paths.insert(file.path.as_str()), "Duplicate template path");
            let partials = mustache::template_partials(&file.content)
                .map_err(|_| anyhow::anyhow!("Invalid Mustache template"))?;
            for partial in partials {
                template_path(&partial)?;
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
        Ok(())
    }
    pub(super) fn write(&self, root: &Path) -> Result<()> {
        self.validate()?;
        std::fs::create_dir(root)?;
        for file in &self.files {
            let path = root.join(&file.path);
            std::fs::create_dir_all(path.parent().unwrap())?;
            std::fs::write(path, file.content.as_bytes())?;
        }
        Ok(())
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
    fn bundle(source: &str) -> TemplateBundle {
        TemplateBundle {
            format: "moleapi-codegen-templates-v1".into(),
            files: vec![TemplateFile {
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
