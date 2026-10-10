use crate::{args::TokenCommand, backend::Backend, commands::output, io};
use anyhow::{Context, Result};
use serde_json::json;
pub async fn execute(backend: &Backend, command: &TokenCommand) -> Result<u8> {
    match command {
        TokenCommand::List => output(
            &backend
                .request("GET", &["auth", "tokens"], &[], None)
                .await?,
        )?,
        TokenCommand::Create {
            name,
            days,
            output: path,
            overwrite,
        } => {
            // Prepare the private output before minting a credential.
            let file = io::OutputFile::prepare(path, *overwrite)?;
            let mut value = backend
                .request(
                    "POST",
                    &["auth", "tokens"],
                    &[],
                    Some(json!({"name":name,"expires_in_days":days})),
                )
                .await?;
            let token = value
                .as_object_mut()
                .context("Invalid token response")?
                .remove("token")
                .context("Token response missing its secret")?;
            let secret = token.as_str().context("Invalid token secret")?;
            if let Err(error) = file.save(secret.as_bytes()) {
                if let Some(id) = value["id"].as_str() {
                    if backend
                        .request("DELETE", &["auth", "tokens", id], &[], None)
                        .await
                        .is_err()
                    {
                        eprintln!(
                            "Output failed and token cleanup failed; revoke token {id} explicitly"
                        );
                    }
                }
                return Err(error);
            }
            output(&value)?;
        }
        TokenCommand::Revoke { id } => output(
            &backend
                .request("DELETE", &["auth", "tokens", id], &[], None)
                .await?,
        )?,
    }
    Ok(0)
}
