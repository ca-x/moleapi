mod args;
mod backend;
mod commands;
mod io;
mod newman;
use args::{Cli, Command};
use clap::{CommandFactory, Parser};
fn schema(command: &clap::Command) -> serde_json::Value {
    serde_json::json!({
        "name": command.get_name(),
        "about": command.get_about().map(ToString::to_string),
        "arguments": command.get_arguments().map(|arg| serde_json::json!({
            "id": arg.get_id().as_str(), "long": arg.get_long(), "short": arg.get_short(),
            "required": arg.is_required_set(), "global": arg.is_global_set(),
            "action": format!("{:?}", arg.get_action()),
            "help": arg.get_help().map(ToString::to_string),
            "defaults": arg.get_default_values().iter().map(|value| value.to_string_lossy()).collect::<Vec<_>>(),
            "values": arg.get_value_parser().possible_values().map(|values| values.filter(|value| !value.is_hide_set()).map(|value|value.get_name().to_owned()).collect::<Vec<_>>())
        })).collect::<Vec<_>>(),
        "subcommands": command.get_subcommands().map(schema).collect::<Vec<_>>()
    })
}
fn main() {
    match moleapi_server::dispatch_script_worker() {
        Ok(true) => return,
        Ok(false) => {}
        Err(_) => {
            eprintln!("Worker dispatch failed");
            std::process::exit(2);
        }
    }
    let cli = Cli::parse();
    if matches!(cli.command, Command::Schema) {
        let mut command = Cli::command();
        command.build();
        println!(
            "{}",
            serde_json::to_string_pretty(&schema(&command)).unwrap()
        );
        return;
    }
    if let Command::Newman(options) = &cli.command {
        let result = if cli.server.is_some()
            || cli.database.is_some()
            || cli.token_env.is_some()
            || cli.token_file.is_some()
        {
            Err(anyhow::anyhow!(
                "Newman uses its own arguments; native server/database/token options cannot be combined"
            ))
        } else {
            newman::execute(options)
        };
        match result {
            Ok(code) => std::process::exit(i32::from(code)),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(2);
            }
        }
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Create CLI runtime");
    let result = runtime.block_on(async {
        let temporary = if cli.server.is_none() && cli.database.is_none() {
            Some(tempfile::tempdir()?)
        } else {
            None
        };
        let backend = if let Some(server) = &cli.server {
            backend::Backend::remote(
                server,
                io::token(cli.token_env.as_deref(), cli.token_file.as_deref())?,
                cli.timeout,
            )?
        } else {
            if cli.token_env.is_some() || cli.token_file.is_some() {
                anyhow::bail!("Authentication options require --server");
            }
            let default_path = temporary
                .as_ref()
                .map(|directory| directory.path().join("moleapi.db"));
            let path = cli
                .database
                .as_deref()
                .or(default_path.as_deref())
                .ok_or_else(|| anyhow::anyhow!("Local database path is unavailable"))?;
            backend::Backend::local(path).await?
        };
        commands::execute(&cli, &backend).await
    });
    match result {
        Ok(code) => std::process::exit(i32::from(code)),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    }
}
