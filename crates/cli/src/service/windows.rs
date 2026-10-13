use super::{Cli, Result, RunnerServiceCommand, windows_xml};
use crate::io;
use anyhow::{Context, ensure};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn runner_id(id: &str) -> Result<String> {
    let parsed = uuid::Uuid::parse_str(id).context("Runner ID must be a canonical UUID")?;
    ensure!(
        parsed.hyphenated().to_string() == id,
        "Runner ID must be a canonical UUID"
    );
    Ok(id.to_owned())
}
fn local_data() -> Result<PathBuf> {
    let path =
        PathBuf::from(std::env::var_os("LOCALAPPDATA").context("LOCALAPPDATA is unavailable")?);
    ensure!(path.is_absolute(), "LOCALAPPDATA must be absolute");
    path.canonicalize().context("Cannot resolve LOCALAPPDATA")
}
fn manifest(id: &str) -> Result<PathBuf> {
    Ok(local_data()?
        .join("MoleAPI/runner-services")
        .join(format!("{id}.xml")))
}
fn private_file(path: &Path, root: &Path) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path).context("Private runner file is unavailable")?;
    ensure!(
        metadata.file_type().is_file(),
        "Runner file must be a regular file, not a symlink"
    );
    let path = path
        .canonicalize()
        .context("Cannot resolve private runner file")?;
    ensure!(
        path.strip_prefix(root).is_ok(),
        "Windows runner files must be inside the current user's LOCALAPPDATA tree"
    );
    Ok(path)
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
    let root = local_data()?;
    let token = private_file(
        cli.token_file
            .as_deref()
            .context("Service installation requires --token-file")?,
        &root,
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
    let variables = variables
        .map(|path| private_file(path, &root))
        .transpose()?;
    Ok((server.to_owned(), token, variables))
}
fn sid() -> Result<String> {
    let result = Command::new("whoami.exe")
        .args(["/user", "/fo", "csv", "/nh"])
        .output()
        .context("Cannot identify the current Windows user")?;
    ensure!(
        result.status.success(),
        "Cannot identify the current Windows user"
    );
    let text = String::from_utf8_lossy(&result.stdout);
    let start = text
        .find("S-1-")
        .context("Current Windows user SID is unavailable")?;
    let value: String = text[start..]
        .chars()
        .take_while(|c| c.is_ascii_digit() || matches!(c, 'S' | '-'))
        .collect();
    ensure!(
        value.len() <= 128 && value.starts_with("S-1-"),
        "Current Windows user SID is invalid"
    );
    Ok(value)
}
fn schtasks(args: &[&str]) -> Result<Output> {
    Command::new("schtasks.exe")
        .args(args)
        .output()
        .context("Cannot start Windows Task Scheduler")
}
fn require_task(args: &[&str]) -> Result<()> {
    ensure!(
        schtasks(args)?.status.success(),
        "Task Scheduler {} failed; inspect the current user's tasks",
        args.join(" ")
    );
    Ok(())
}
fn query(name: &str) -> Result<Option<String>> {
    let result = schtasks(&["/Query", "/TN", name, "/XML"])?;
    if !result.status.success() {
        return Ok(None);
    }
    let bytes = &result.stdout;
    let text = if bytes.len() % 2 == 0
        && bytes
            .iter()
            .skip(1)
            .step_by(2)
            .filter(|&&byte| byte == 0)
            .count()
            > bytes.len() / 4
    {
        String::from_utf16_lossy(
            &bytes
                .chunks_exact(2)
                .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                .collect::<Vec<_>>(),
        )
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    };
    Ok(Some(text))
}
fn verified_task(name: &str, id: &str) -> Result<bool> {
    let Some(xml) = query(name)? else {
        return Ok(false);
    };
    ensure!(
        xml.contains(&windows_xml::marker(id)),
        "Registered task is not the MoleAPI runner for this ID"
    );
    Ok(true)
}
fn existing_file(path: &Path, id: &str) -> Result<bool> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    ensure!(
        metadata.file_type().is_file(),
        "Runner task manifest must be a regular file, not a symlink"
    );
    windows_xml::owned(
        &fs::read_to_string(path).context("Cannot read runner task manifest")?,
        id,
    )?;
    Ok(true)
}
pub(super) fn execute(cli: &Cli, command: &RunnerServiceCommand) -> Result<u8> {
    match command {
        RunnerServiceCommand::Print { id, variables_file } => {
            let id = runner_id(id)?;
            let (server, token, variables) = source(cli, variables_file.as_deref())?;
            let executable = std::env::current_exe()?.canonicalize()?;
            print!(
                "{}",
                windows_xml::render(
                    &id,
                    executable
                        .to_str()
                        .context("Executable path is not Unicode")?,
                    &server,
                    token.to_str().context("Token path is not Unicode")?,
                    variables
                        .as_deref()
                        .map(|value| value.to_str().context("Variables path is not Unicode"))
                        .transpose()?,
                    &sid()?
                )?
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
            let xml = windows_xml::render(
                &id,
                executable
                    .to_str()
                    .context("Executable path is not Unicode")?,
                &server,
                token.to_str().context("Token path is not Unicode")?,
                variables
                    .as_deref()
                    .map(|value| value.to_str().context("Variables path is not Unicode"))
                    .transpose()?,
                &sid()?,
            )?;
            let file = manifest(&id)?;
            let name = windows_xml::task_name(&id);
            let existing = existing_file(&file, &id)?;
            let registered = verified_task(&name, &id)?;
            ensure!(
                !registered || existing,
                "A foreign task already uses this runner name"
            );
            ensure!(
                !existing || *replace,
                "Runner task already exists; use --replace after review"
            );
            fs::create_dir_all(file.parent().unwrap())
                .context("Cannot create runner task directory")?;
            io::save(&file, xml.as_bytes(), *replace)?;
            let path = file.to_str().context("Task manifest path is not Unicode")?;
            if *replace && registered {
                let _ = schtasks(&["/End", "/TN", &name]);
                require_task(&["/Create", "/TN", &name, "/XML", path, "/F"])?;
            } else {
                require_task(&["/Create", "/TN", &name, "/XML", path])?;
            }
            require_task(&["/Run", "/TN", &name])?;
            println!("{}", json!({"runner_id":id,"installed":true,"task":name}));
        }
        RunnerServiceCommand::Status { id } => {
            let id = runner_id(id)?;
            let file = manifest(&id)?;
            let existing = existing_file(&file, &id)?;
            let registered = if existing {
                verified_task(&windows_xml::task_name(&id), &id)?
            } else {
                false
            };
            println!(
                "{}",
                json!({"runner_id":id,"installed":existing,"registered":registered})
            );
        }
        RunnerServiceCommand::Uninstall { id } => {
            let id = runner_id(id)?;
            let file = manifest(&id)?;
            ensure!(existing_file(&file, &id)?, "Runner task is not installed");
            let name = windows_xml::task_name(&id);
            if verified_task(&name, &id)? {
                let _ = schtasks(&["/End", "/TN", &name]);
                require_task(&["/Delete", "/TN", &name, "/F"])?;
            }
            fs::remove_file(&file).context("Cannot remove runner task manifest")?;
            println!("{}", json!({"runner_id":id,"installed":false}));
        }
    }
    Ok(0)
}
