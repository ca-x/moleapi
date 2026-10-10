use crate::io;
use anyhow::{Context, Result};
use std::path::Path;
pub fn execute(config: &Path, output: Option<&Path>, overwrite: bool) -> Result<()> {
    let source = io::read(config, 512 * 1024)?;
    let config: moleapi_formats::ci::Config =
        serde_json::from_str(&source).context("Cannot parse CI configuration")?;
    let preset = moleapi_formats::ci::generate(&config).context("Cannot generate CI preset")?;
    if let Some(path) = output {
        io::save(path, preset.content.as_bytes(), overwrite)?;
    } else {
        println!("{}", serde_json::to_string_pretty(&preset)?);
    }
    Ok(())
}
