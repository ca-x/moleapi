//! Embedded agent instructions are available without initializing storage or credentials.
use crate::{args::SkillFormat, io};
use anyhow::Result;
use std::path::Path;
const SKILL: &str = include_str!("../../../tools/skills/moleapi/SKILL.md");
const INTERFACE: &str = include_str!("../../../tools/skills/moleapi/agents/openai.yaml");
pub fn execute(format: &SkillFormat, output: Option<&Path>, overwrite: bool) -> Result<()> {
    let content = match format {
        SkillFormat::Markdown => SKILL.to_owned(),
        SkillFormat::Json => serde_json::to_string_pretty(
            &serde_json::json!({"name":"moleapi","files":[{"path":"SKILL.md","content":SKILL},{"path":"agents/openai.yaml","content":INTERFACE}]}),
        )?,
    };
    if let Some(path) = output {
        io::save(path, content.as_bytes(), overwrite)?;
    } else {
        println!("{content}");
    }
    Ok(())
}
