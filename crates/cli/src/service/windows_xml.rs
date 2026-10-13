use anyhow::{Result, ensure};
use sha2::{Digest, Sha256};

const MARKER: &str = "Managed by MoleAPI runner service v1";
pub(super) fn task_name(id: &str) -> String {
    format!("\\MoleAPI\\Runner-{id}")
}
fn xml(value: &str) -> Result<String> {
    ensure!(
        !value.chars().any(char::is_control),
        "Task argument contains control characters"
    );
    Ok(value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;"))
}
fn quote(value: &str) -> Result<String> {
    ensure!(
        !value.chars().any(char::is_control),
        "Task argument contains control characters"
    );
    let mut output = String::from("\"");
    let mut slashes = 0;
    for character in value.chars() {
        match character {
            '\\' => slashes += 1,
            '"' => {
                output.push_str(&"\\".repeat(slashes * 2 + 1));
                output.push('"');
                slashes = 0;
            }
            _ => {
                output.push_str(&"\\".repeat(slashes));
                output.push(character);
                slashes = 0;
            }
        }
    }
    output.push_str(&"\\".repeat(slashes * 2));
    output.push('"');
    Ok(output)
}
pub(super) fn render(
    id: &str,
    executable: &str,
    server: &str,
    token: &str,
    variables: Option<&str>,
    sid: &str,
) -> Result<String> {
    ensure!(
        sid.starts_with("S-1-")
            && sid
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b'S' || byte == b'-'),
        "Current Windows user SID is invalid"
    );
    let mut args = vec![
        "--server",
        server,
        "--token-file",
        token,
        "runner",
        "agent",
        "--id",
        id,
    ];
    if let Some(variables) = variables {
        args.extend(["--variables-file", variables]);
    }
    let arguments = args
        .into_iter()
        .map(quote)
        .collect::<Result<Vec<_>>>()?
        .join(" ");
    let body = format!(
        "<Task version=\"1.4\" xmlns=\"http://schemas.microsoft.com/windows/2004/02/mit/task\">\n  <RegistrationInfo><Description>{MARKER}; {id}</Description></RegistrationInfo>\n  <Triggers><LogonTrigger><Enabled>true</Enabled></LogonTrigger></Triggers>\n  <Principals><Principal id=\"Author\"><UserId>{sid}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals>\n  <Settings><MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy><DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries><StopIfGoingOnBatteries>false</StopIfGoingOnBatteries><AllowStartOnDemand>true</AllowStartOnDemand><StartWhenAvailable>true</StartWhenAvailable><RunOnlyIfNetworkAvailable>true</RunOnlyIfNetworkAvailable><ExecutionTimeLimit>PT0S</ExecutionTimeLimit><RestartOnFailure><Interval>PT1M</Interval><Count>999</Count></RestartOnFailure></Settings>\n  <Actions Context=\"Author\"><Exec><Command>{}</Command><Arguments>{}</Arguments></Exec></Actions>\n</Task>\n",
        xml(executable)?,
        xml(&arguments)?
    );
    let digest = hex::encode(Sha256::digest(body.as_bytes()));
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- {MARKER} -->\n<!-- Runner-ID: {id} -->\n<!-- SHA256: {digest} -->\n{body}"
    ))
}
pub(super) fn owned(content: &str, id: &str) -> Result<()> {
    let prefix = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- {MARKER} -->\n<!-- Runner-ID: {id} -->\n<!-- SHA256: "
    );
    let rest = content.strip_prefix(&prefix).ok_or_else(|| {
        anyhow::anyhow!("Existing task manifest is not managed by MoleAPI for this runner")
    })?;
    let (digest, body) = rest
        .split_once(" -->\n")
        .ok_or_else(|| anyhow::anyhow!("Existing task manifest has no checksum"))?;
    ensure!(
        digest == hex::encode(Sha256::digest(body.as_bytes())),
        "Existing task manifest was modified; review it manually"
    );
    Ok(())
}
pub(super) fn marker(id: &str) -> String {
    format!("{MARKER}; {id}")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn task_xml_quotes_arguments_and_rejects_modified_manifest() {
        let id = "00000000-0000-4000-8000-000000000001";
        let xml = render(
            id,
            r"C:\Program Files\Mole & API\moleapi-cli.exe",
            "https://example.com/",
            r"C:\Users\Me\private token.txt",
            None,
            "S-1-5-21-42",
        )
        .unwrap();
        assert!(xml.contains("Mole &amp; API"));
        assert!(xml.contains("private token.txt"));
        assert!(xml.contains("<RestartOnFailure>"));
        assert!(xml.contains(&marker(id)));
        assert!(task_name(id).ends_with(&format!("Runner-{id}")));
        assert!(!xml.contains("moleapi_runner_"));
        owned(&xml, id).unwrap();
        assert!(owned(&xml.replace("<Count>999</Count>", "<Count>1</Count>"), id).is_err());
        assert_eq!(quote("a\\\"b").unwrap(), "\"a\\\\\\\"b\"");
    }
}
