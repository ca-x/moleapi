use crate::args::{Cli, RunnerServiceCommand};
use anyhow::Result;
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
use anyhow::bail;

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub fn execute(_: &Cli, _: &RunnerServiceCommand) -> Result<u8> {
    bail!("Runner service installation is not implemented for this platform yet")
}

#[cfg(target_os = "windows")]
mod windows;
#[cfg(any(target_os = "windows", test))]
mod windows_xml;

#[cfg(target_os = "windows")]
pub fn execute(cli: &Cli, command: &RunnerServiceCommand) -> Result<u8> {
    windows::execute(cli, command)
}

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub fn execute(cli: &Cli, command: &RunnerServiceCommand) -> Result<u8> {
    macos::execute(cli, command)
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use crate::io;
    use anyhow::{Context, ensure};
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
    };

    const MARKER: &str = "# Managed by MoleAPI runner service v1";

    fn runner_id(id: &str) -> Result<String> {
        let parsed = uuid::Uuid::parse_str(id).context("Runner ID must be a canonical UUID")?;
        ensure!(
            parsed.hyphenated().to_string() == id,
            "Runner ID must be a canonical UUID"
        );
        Ok(id.to_owned())
    }
    fn unit_name(id: &str) -> String {
        format!("moleapi-runner-{id}.service")
    }
    fn unit_directory() -> Result<PathBuf> {
        let base = if let Some(path) = std::env::var_os("XDG_CONFIG_HOME") {
            PathBuf::from(path)
        } else {
            PathBuf::from(std::env::var_os("HOME").context("HOME is unavailable")?).join(".config")
        };
        ensure!(
            base.is_absolute(),
            "User configuration directory must be absolute"
        );
        Ok(base.join("systemd/user"))
    }
    fn private_file(path: &Path) -> Result<PathBuf> {
        let metadata = fs::symlink_metadata(path).context("Private runner file is unavailable")?;
        ensure!(
            metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
            "Runner file must be a regular file, not a symlink"
        );
        use std::os::unix::fs::PermissionsExt;
        ensure!(
            metadata.permissions().mode() & 0o077 == 0,
            "Runner file must deny group and other access"
        );
        path.canonicalize()
            .context("Cannot resolve private runner file")
    }
    fn argument(value: &str) -> Result<String> {
        ensure!(
            !value.is_empty() && !value.contains('$') && !value.chars().any(char::is_control),
            "Service argument contains unsupported characters"
        );
        Ok(format!(
            "\"{}\"",
            value
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('%', "%%")
        ))
    }
    fn source(cli: &Cli, variables: Option<&Path>) -> Result<(String, PathBuf, Option<PathBuf>)> {
        ensure!(
            cli.database.is_none() && cli.token_env.is_none(),
            "Service installation requires --server and a private --token-file"
        );
        let server = cli
            .server
            .as_deref()
            .context("Service installation requires --server")?;
        let url = reqwest::Url::parse(server).context("Invalid hosted service URL")?;
        let loopback = url.host_str().is_some_and(|host| {
            host == "localhost"
                || host
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
        });
        ensure!(
            matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none()
                && (url.scheme() == "https" || loopback),
            "Service URL requires HTTPS except loopback HTTP and no embedded credentials/query/fragment"
        );
        let token = private_file(
            cli.token_file
                .as_deref()
                .context("Service installation requires --token-file")?,
        )?;
        let raw = io::token(None, Some(&token))?.context("Agent credential file is empty")?;
        ensure!(
            raw.strip_prefix("moleapi_runner_")
                .is_some_and(|secret| secret.len() == 64
                    && secret
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))),
            "Token file must contain a workspace-scoped runner credential"
        );
        let variables = variables.map(private_file).transpose()?;
        Ok((server.to_owned(), token, variables))
    }
    fn render(
        id: &str,
        executable: &Path,
        server: &str,
        token: &Path,
        variables: Option<&Path>,
    ) -> Result<String> {
        let mut command = format!(
            "{} --server {} --token-file {} runner agent --id {}",
            argument(&executable.to_string_lossy())?,
            argument(server)?,
            argument(&token.to_string_lossy())?,
            id
        );
        if let Some(variables) = variables {
            command.push_str(&format!(
                " --variables-file {}",
                argument(&variables.to_string_lossy())?
            ));
        }
        let body = format!(
            "[Unit]\nDescription=MoleAPI runner {id}\nWants=network-online.target\nAfter=network-online.target\n\n[Service]\nType=simple\nExecStart={command}\nRestart=on-failure\nRestartSec=5s\nUMask=0077\nNoNewPrivileges=true\n\n[Install]\nWantedBy=default.target\n"
        );
        let digest = hex::encode(Sha256::digest(body.as_bytes()));
        Ok(format!(
            "{MARKER}\n# Runner-ID: {id}\n# SHA256: {digest}\n{body}"
        ))
    }
    fn owned(content: &str, id: &str) -> Result<()> {
        let prefix = format!("{MARKER}\n# Runner-ID: {id}\n# SHA256: ");
        let rest = content
            .strip_prefix(&prefix)
            .context("Existing unit is not managed by MoleAPI for this runner")?;
        let (digest, body) = rest
            .split_once('\n')
            .context("Existing unit has no checksum")?;
        ensure!(
            digest == hex::encode(Sha256::digest(body.as_bytes())),
            "Existing unit was modified; review it manually"
        );
        Ok(())
    }
    fn manager(arguments: &[&str]) -> Result<String> {
        let output = Command::new("systemctl")
            .arg("--user")
            .args(arguments)
            .output()
            .context("Cannot start systemctl user manager")?;
        ensure!(
            output.status.success(),
            "systemctl --user {} failed; inspect the user manager",
            arguments.join(" ")
        );
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }
    fn installed_path(id: &str) -> Result<PathBuf> {
        Ok(unit_directory()?.join(unit_name(id)))
    }

    pub(super) fn execute(cli: &Cli, command: &RunnerServiceCommand) -> Result<u8> {
        match command {
            RunnerServiceCommand::Print { id, variables_file } => {
                let id = runner_id(id)?;
                let (server, token, variables) = source(cli, variables_file.as_deref())?;
                let executable = std::env::current_exe()?.canonicalize()?;
                print!(
                    "{}",
                    render(&id, &executable, &server, &token, variables.as_deref())?
                );
            }
            RunnerServiceCommand::Install {
                id,
                variables_file,
                replace,
            } => {
                let id = runner_id(id)?;
                let (server, token, variables) = source(cli, variables_file.as_deref())?;
                let executable = std::env::current_exe()?.canonicalize()?;
                let content = render(&id, &executable, &server, &token, variables.as_deref())?;
                let path = installed_path(&id)?;
                let existing = fs::symlink_metadata(&path).ok();
                if let Some(metadata) = &existing {
                    ensure!(
                        metadata.file_type().is_file(),
                        "Runner unit must be a regular file, not a symlink"
                    );
                    ensure!(
                        *replace,
                        "Runner unit already exists; use --replace after review"
                    );
                    owned(
                        &fs::read_to_string(&path).context("Cannot read existing runner unit")?,
                        &id,
                    )?;
                }
                fs::create_dir_all(path.parent().unwrap())
                    .context("Cannot create user unit directory")?;
                io::save(&path, content.as_bytes(), *replace)?;
                manager(&["daemon-reload"])?;
                manager(&["enable", "--now", &unit_name(&id)])?;
                if existing.is_some() {
                    manager(&["restart", &unit_name(&id)])?;
                }
                println!("{}", json!({"runner_id":id,"installed":true,"unit":path}));
            }
            RunnerServiceCommand::Status { id } => {
                let id = runner_id(id)?;
                let path = installed_path(&id)?;
                let existing = fs::symlink_metadata(&path).ok();
                if let Some(metadata) = existing {
                    ensure!(
                        metadata.file_type().is_file(),
                        "Runner unit must be a regular file, not a symlink"
                    );
                    owned(
                        &fs::read_to_string(&path).context("Cannot read runner unit")?,
                        &id,
                    )?;
                    let output = Command::new("systemctl")
                        .arg("--user")
                        .args(["is-active", &unit_name(&id)])
                        .output()
                        .context("Cannot query systemctl user manager")?;
                    let state = String::from_utf8_lossy(&output.stdout).trim().to_owned();
                    ensure!(
                        matches!(
                            state.as_str(),
                            "active" | "inactive" | "failed" | "activating" | "deactivating"
                        ),
                        "Cannot determine runner service state"
                    );
                    println!(
                        "{}",
                        json!({"runner_id":id,"installed":true,"active":state=="active","state":state})
                    );
                } else {
                    println!(
                        "{}",
                        json!({"runner_id":id,"installed":false,"active":false})
                    );
                }
            }
            RunnerServiceCommand::Uninstall { id } => {
                let id = runner_id(id)?;
                let path = installed_path(&id)?;
                ensure!(
                    fs::symlink_metadata(&path)
                        .context("Runner unit is not installed")?
                        .file_type()
                        .is_file(),
                    "Runner unit must be a regular file, not a symlink"
                );
                let content = fs::read_to_string(&path).context("Runner unit is not installed")?;
                owned(&content, &id)?;
                manager(&["disable", "--now", &unit_name(&id)])?;
                fs::remove_file(&path).context("Cannot remove runner unit")?;
                manager(&["daemon-reload"])?;
                println!("{}", json!({"runner_id":id,"installed":false}));
            }
        }
        Ok(0)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn unit_escapes_paths_and_refuses_modified_content() {
            let id = "00000000-0000-4000-8000-000000000001";
            let unit = render(
                id,
                Path::new("/tmp/Mole API/%agent"),
                "https://example.com/",
                Path::new("/tmp/private token"),
                None,
            )
            .unwrap();
            assert!(unit.contains("ExecStart=\"/tmp/Mole API/%%agent\""));
            assert!(unit.contains("--token-file \"/tmp/private token\""));
            owned(&unit, id).unwrap();
            assert!(owned(&unit.replace("RestartSec=5s", "RestartSec=0s"), id).is_err());
            assert!(argument("unsafe$variable").is_err());
        }
    }
}

#[cfg(target_os = "linux")]
pub fn execute(cli: &Cli, command: &RunnerServiceCommand) -> Result<u8> {
    linux::execute(cli, command)
}
