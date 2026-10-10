//! Explicit optional upstream runtime. Native commands never require Node/Newman.
use crate::args::Newman;
use anyhow::{Context, Result, ensure};
use std::process::Command;
pub fn execute(options: &Newman) -> Result<u8> {
    ensure!(
        options.node.is_absolute() && options.node.is_file(),
        "Newman requires an explicit absolute Node executable path"
    );
    ensure!(
        options.entrypoint.is_absolute() && options.entrypoint.is_file(),
        "Newman requires an explicit absolute installed CLI entrypoint path"
    );
    let mut command = Command::new(&options.node);
    command.arg(&options.entrypoint).args(&options.arguments);
    // Unix replacement retains terminal signals, upstream reporters and exact exit status.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        Err(command.exec()).context("Cannot launch the configured Newman runtime")
    }
    #[cfg(not(unix))]
    {
        let status = command
            .status()
            .context("Cannot launch the configured Newman runtime")?;
        Ok(status
            .code()
            .and_then(|code| u8::try_from(code).ok())
            .unwrap_or(2))
    }
}
