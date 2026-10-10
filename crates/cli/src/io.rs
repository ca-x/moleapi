use anyhow::{Context, Result, bail};
use std::{
    io::{Read, Write},
    path::Path,
};
pub fn read(path: &Path, limit: usize) -> Result<String> {
    let file = std::fs::File::open(path).context("Cannot open input file")?;
    let mut bytes = vec![];
    file.take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .context("Cannot read input file")?;
    if bytes.len() > limit {
        bail!("Input file exceeds its size limit");
    }
    String::from_utf8(bytes).context("Input must be UTF-8")
}
pub struct OutputFile {
    file: tempfile::NamedTempFile,
    path: std::path::PathBuf,
    overwrite: bool,
}
impl OutputFile {
    pub fn prepare(path: &Path, overwrite: bool) -> Result<Self> {
        if !overwrite && std::fs::symlink_metadata(path).is_ok() {
            bail!("Output exists; use --overwrite explicitly");
        }
        let parent = path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let file = tempfile::NamedTempFile::new_in(parent).context("Cannot create output file")?;
        Ok(Self {
            file,
            path: path.to_owned(),
            overwrite,
        })
    }
    pub fn save(mut self, bytes: &[u8]) -> Result<()> {
        self.file
            .write_all(bytes)
            .context("Cannot write output file")?;
        self.file
            .as_file()
            .sync_all()
            .context("Cannot flush output file")?;
        if self.overwrite {
            self.file
                .persist(self.path)
                .map_err(|_| anyhow::anyhow!("Cannot replace output file"))?;
        } else {
            self.file.persist_noclobber(self.path).map_err(|_| {
                anyhow::anyhow!("Output exists or cannot be created; use --overwrite explicitly")
            })?;
        }
        Ok(())
    }
}
pub fn save(path: &Path, bytes: &[u8], overwrite: bool) -> Result<()> {
    OutputFile::prepare(path, overwrite)?.save(bytes)
}

pub fn token(environment: Option<&str>, file: Option<&Path>) -> Result<Option<String>> {
    let value = if let Some(environment) = environment {
        Some(std::env::var(environment).context("Token environment variable is unavailable")?)
    } else if let Some(file) = file {
        Some(read(file, 16384)?)
    } else {
        None
    };
    if let Some(value) = value {
        let value = value.trim().to_string();
        if value.is_empty() || value.len() > 16384 || value.chars().any(char::is_control) {
            bail!("Invalid token value");
        }
        Ok(Some(value))
    } else {
        Ok(None)
    }
}
