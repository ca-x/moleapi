#[allow(dead_code)]
mod common;
use common::*;
use serde_json::{Value, json};
#[tokio::test]
async fn cli_mints_private_token_once_and_uses_it_until_revoked() {
    let temporary = tempfile::tempdir().unwrap();
    let router = moleapi_server::hosted_with_worker(
        moleapi_server::Config {
            database_url: format!(
                "sqlite://{}?mode=rwc",
                temporary.path().join("tokens.db").display()
            ),
            setup_token: "fixture-setup".into(),
            allow_registration: true,
            allow_private_network: true,
        },
        std::path::Path::new(env!("CARGO_BIN_EXE_moleapi-cli")),
    )
    .await
    .unwrap();
    let (server, handle) = serve(router).await;
    let http = reqwest::Client::new();
    let session:Value=http.post(format!("{server}/api/auth/register")).json(&json!({"username":"tokenowner","password":"fixture-password-123","setup_token":"fixture-setup"})).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
    let session_file = temporary.path().join("session.txt");
    std::fs::write(&session_file, session["token"].as_str().unwrap()).unwrap();
    let prefix = [
        "--server",
        server.as_str(),
        "--token-file",
        session_file.to_str().unwrap(),
    ];
    let output = temporary.path().join("ci-token.txt");
    let mut args = prefix.to_vec();
    args.extend([
        "tokens",
        "create",
        "--name",
        "CI",
        "--days",
        "7",
        "--output",
        output.to_str().unwrap(),
    ]);
    let metadata = result(cli(&args).await, 0);
    assert!(metadata.get("token").is_none());
    let secret = std::fs::read_to_string(&output).unwrap();
    assert!(secret.starts_with("moleapi_pat_"));
    assert!(!metadata.to_string().contains(&secret));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&output).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    let duplicate = cli(&args).await;
    assert_eq!(duplicate.status.code(), Some(2));
    assert_eq!(std::fs::read_to_string(&output).unwrap(), secret);
    let mut list = prefix.to_vec();
    list.extend(["tokens", "list"]);
    let list = result(cli(&list).await, 0);
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert!(!list.to_string().contains(&secret));
    let result_value = result(
        cli(&[
            "--server",
            &server,
            "--token-file",
            output.to_str().unwrap(),
            "list",
            "workspaces",
        ])
        .await,
        0,
    );
    assert!(result_value.is_array());
    assert_eq!(
        cli(&[
            "--server",
            &server,
            "--token-file",
            output.to_str().unwrap(),
            "tokens",
            "list"
        ])
        .await
        .status
        .code(),
        Some(2)
    );
    let mut revoke = prefix.to_vec();
    revoke.extend(["tokens", "revoke", "--id", metadata["id"].as_str().unwrap()]);
    assert_eq!(result(cli(&revoke).await, 0)["revoked"], true);
    assert_eq!(
        cli(&[
            "--server",
            &server,
            "--token-file",
            output.to_str().unwrap(),
            "list",
            "workspaces"
        ])
        .await
        .status
        .code(),
        Some(2)
    );
    handle.abort();
}
