#![cfg(target_os = "linux")]
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Command, Output},
};

const ID: &str = "00000000-0000-4000-8000-000000000001";
fn invoke(root: &Path, args: &[&str]) -> Output {
    let bin = root.join("bin");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
    Command::new(env!("CARGO_BIN_EXE_moleapi-cli"))
        .args(args)
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("MOLEAPI_TEST_SYSTEMCTL_LOG", root.join("systemctl.log"))
        .env("PATH", path)
        .output()
        .unwrap()
}
fn output(result: &Output, code: i32) -> serde_json::Value {
    assert_eq!(
        result.status.code(),
        Some(code),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    serde_json::from_slice(&result.stdout).unwrap()
}

#[test]
fn installs_and_removes_only_owned_private_systemd_units_without_printing_a_bearer() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    fs::create_dir_all(root.join("bin")).unwrap();
    let manager = root.join("bin/systemctl");
    fs::write(&manager, "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$MOLEAPI_TEST_SYSTEMCTL_LOG\"\nif [ \"$2\" = is-active ]; then printf 'active\\n'; fi\n").unwrap();
    fs::set_permissions(&manager, fs::Permissions::from_mode(0o700)).unwrap();
    let token_path = root.join("private token.txt");
    let token = format!("moleapi_runner_{}", "a".repeat(64));
    fs::write(&token_path, &token).unwrap();
    fs::set_permissions(&token_path, fs::Permissions::from_mode(0o600)).unwrap();
    let private = token_path.to_str().unwrap();
    let args = [
        "--server",
        "https://api.example.com",
        "--token-file",
        private,
        "runner",
        "service",
    ];
    let preview = invoke(root, &[args.as_slice(), &["print", "--id", ID]].concat());
    assert_eq!(preview.status.code(), Some(0));
    let preview = String::from_utf8(preview.stdout).unwrap();
    assert!(preview.contains("Restart=on-failure"));
    assert!(preview.contains("--token-file"));
    assert!(!preview.contains(&token));
    let installed = output(
        &invoke(root, &[args.as_slice(), &["install", "--id", ID]].concat()),
        0,
    );
    assert_eq!(installed["installed"], true);
    let unit_path = root
        .join("config/systemd/user")
        .join(format!("moleapi-runner-{ID}.service"));
    let unit = fs::read_to_string(&unit_path).unwrap();
    assert!(unit.starts_with("# Managed by MoleAPI runner service v1\n"));
    assert!(!unit.contains(&token));
    assert_eq!(
        fs::metadata(&unit_path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        output(
            &invoke(root, &["runner", "service", "status", "--id", ID]),
            0
        )["active"],
        true
    );
    assert_eq!(
        invoke(root, &[args.as_slice(), &["install", "--id", ID]].concat())
            .status
            .code(),
        Some(2)
    );
    fs::write(&unit_path, format!("{unit}# local edit\n")).unwrap();
    assert_eq!(
        invoke(root, &["runner", "service", "uninstall", "--id", ID])
            .status
            .code(),
        Some(2)
    );
    fs::write(&unit_path, &unit).unwrap();
    output(
        &invoke(
            root,
            &[args.as_slice(), &["install", "--id", ID, "--replace"]].concat(),
        ),
        0,
    );
    output(
        &invoke(root, &["runner", "service", "uninstall", "--id", ID]),
        0,
    );
    assert!(!unit_path.exists());
    let log = fs::read_to_string(root.join("systemctl.log")).unwrap();
    assert!(log.contains("--user daemon-reload"));
    assert!(log.contains(&format!("--user enable --now moleapi-runner-{ID}.service")));
    assert!(log.contains(&format!("--user restart moleapi-runner-{ID}.service")));
    assert!(log.contains(&format!("--user disable --now moleapi-runner-{ID}.service")));
    fs::set_permissions(&token_path, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        invoke(root, &[args.as_slice(), &["install", "--id", ID]].concat())
            .status
            .code(),
        Some(2)
    );
    assert!(!unit_path.exists());
}
