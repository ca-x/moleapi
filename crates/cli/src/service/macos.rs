use super::{Cli, Result, RunnerServiceCommand};
use crate::io;
use anyhow::{Context, ensure};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
};

const MARKER: &str = "<!-- Managed by MoleAPI runner service v1 -->";

fn runner_id(id: &str) -> Result<String> {
    let parsed = uuid::Uuid::parse_str(id).context("Runner ID must be a canonical UUID")?;
    ensure!(
        parsed.hyphenated().to_string() == id,
        "Runner ID must be a canonical UUID"
    );
    Ok(id.to_owned())
}
fn label(id: &str) -> String {
    format!("com.moleapi.runner.{id}")
}
fn path(id: &str) -> Result<PathBuf> {
    let directory = PathBuf::from(std::env::var_os("HOME").context("HOME is unavailable")?);
    ensure!(directory.is_absolute(), "HOME must be absolute");
    Ok(directory
        .join("Library/LaunchAgents")
        .join(format!("{}.plist", label(id))))
}
fn private_file(path: &Path) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path).context("Private runner file is unavailable")?;
    ensure!(
        metadata.file_type().is_file(),
        "Runner file must be a regular file, not a symlink"
    );
    ensure!(
        metadata.permissions().mode() & 0o077 == 0,
        "Runner file must deny group and other access"
    );
    path.canonicalize()
        .context("Cannot resolve private runner file")
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
fn xml(value: &str) -> Result<String> {
    ensure!(
        !value.chars().any(char::is_control),
        "Service argument contains control characters"
    );
    Ok(value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;"))
}
fn render(
    id: &str,
    executable: &Path,
    server: &str,
    token: &Path,
    variables: Option<&Path>,
) -> Result<String> {
    let mut arguments = vec![
        executable.to_string_lossy().into_owned(),
        "--server".into(),
        server.into(),
        "--token-file".into(),
        token.to_string_lossy().into_owned(),
        "runner".into(),
        "agent".into(),
        "--id".into(),
        id.into(),
    ];
    if let Some(variables) = variables {
        arguments.extend([
            "--variables-file".into(),
            variables.to_string_lossy().into_owned(),
        ]);
    }
    let arguments = arguments
        .iter()
        .map(|argument| xml(argument).map(|value| format!("    <string>{value}</string>\n")))
        .collect::<Result<String>>()?;
    let body = format!(
        "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n  <key>Label</key><string>{}</string>\n  <key>ProgramArguments</key>\n  <array>\n{arguments}  </array>\n  <key>RunAtLoad</key><true/>\n  <key>KeepAlive</key><true/>\n  <key>ThrottleInterval</key><integer>5</integer>\n</dict>\n</plist>\n",
        label(id)
    );
    let digest = hex::encode(Sha256::digest(body.as_bytes()));
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{MARKER}\n<!-- Runner-ID: {id} -->\n<!-- SHA256: {digest} -->\n{body}"
    ))
}
fn owned(content: &str, id: &str) -> Result<()> {
    let prefix = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{MARKER}\n<!-- Runner-ID: {id} -->\n<!-- SHA256: "
    );
    let rest = content
        .strip_prefix(&prefix)
        .context("Existing LaunchAgent is not managed by MoleAPI for this runner")?;
    let (digest, body) = rest
        .split_once(" -->\n")
        .context("Existing LaunchAgent has no checksum")?;
    ensure!(
        digest == hex::encode(Sha256::digest(body.as_bytes())),
        "Existing LaunchAgent was modified; review it manually"
    );
    Ok(())
}
fn domain() -> String {
    format!("gui/{}", unsafe { libc::geteuid() })
}
fn target(id: &str) -> String {
    format!("{}/{}", domain(), label(id))
}
fn launchctl(arguments: &[&str]) -> Result<std::process::Output> {
    Command::new("launchctl")
        .args(arguments)
        .output()
        .context("Cannot start launchctl")
}
fn loaded(id: &str) -> Result<bool> {
    Ok(launchctl(&["print", &target(id)])?.status.success())
}
fn require_launchctl(arguments: &[&str]) -> Result<()> {
    ensure!(
        launchctl(arguments)?.status.success(),
        "launchctl {} failed; inspect the user agent",
        arguments.join(" ")
    );
    Ok(())
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
            let file = path(&id)?;
            let existing = fs::symlink_metadata(&file).ok();
            if let Some(metadata) = existing {
                ensure!(
                    metadata.file_type().is_file(),
                    "Runner LaunchAgent must be a regular file, not a symlink"
                );
                ensure!(
                    *replace,
                    "Runner LaunchAgent already exists; use --replace after review"
                );
                owned(
                    &fs::read_to_string(&file).context("Cannot read existing LaunchAgent")?,
                    &id,
                )?;
                if loaded(&id)? {
                    require_launchctl(&["bootout", &target(&id)])?;
                }
            }
            fs::create_dir_all(file.parent().unwrap())
                .context("Cannot create user LaunchAgents directory")?;
            io::save(&file, content.as_bytes(), *replace)?;
            require_launchctl(&[
                "bootstrap",
                &domain(),
                file.to_str().context("LaunchAgent path is not UTF-8")?,
            ])?;
            println!("{}", json!({"runner_id":id,"installed":true,"agent":file}));
        }
        RunnerServiceCommand::Status { id } => {
            let id = runner_id(id)?;
            let file = path(&id)?;
            if !file.exists() {
                println!(
                    "{}",
                    json!({"runner_id":id,"installed":false,"loaded":false})
                );
            } else {
                ensure!(
                    fs::symlink_metadata(&file)?.file_type().is_file(),
                    "Runner LaunchAgent must be a regular file, not a symlink"
                );
                owned(&fs::read_to_string(&file)?, &id)?;
                println!(
                    "{}",
                    json!({"runner_id":id,"installed":true,"loaded":loaded(&id)?})
                );
            }
        }
        RunnerServiceCommand::Uninstall { id } => {
            let id = runner_id(id)?;
            let file = path(&id)?;
            ensure!(
                fs::symlink_metadata(&file)
                    .context("Runner LaunchAgent is not installed")?
                    .file_type()
                    .is_file(),
                "Runner LaunchAgent must be a regular file, not a symlink"
            );
            owned(&fs::read_to_string(&file)?, &id)?;
            if loaded(&id)? {
                require_launchctl(&["bootout", &target(&id)])?;
            }
            fs::remove_file(&file).context("Cannot remove runner LaunchAgent")?;
            println!("{}", json!({"runner_id":id,"installed":false}));
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plist_escapes_arguments_and_detects_local_edits() {
        let id = "00000000-0000-4000-8000-000000000001";
        let plist = render(
            id,
            Path::new("/tmp/Mole & API"),
            "https://example.com/a?x=1&y=2",
            Path::new("/tmp/private token"),
            None,
        )
        .unwrap();
        assert!(plist.contains("Mole &amp; API"));
        assert!(plist.contains("x=1&amp;y=2"));
        owned(&plist, id).unwrap();
        assert!(
            owned(
                &plist.replace(
                    "<key>KeepAlive</key><true/>",
                    "<key>KeepAlive</key><false/>"
                ),
                id
            )
            .is_err()
        );
    }
}
