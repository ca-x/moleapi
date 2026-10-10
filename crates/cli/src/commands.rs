use crate::{
    args::{Cli, Command, Resource, Run},
    backend::Backend,
    io,
};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{path::Path, time::Duration};
fn id(items: &[Value], name: &str) -> Result<String> {
    if let Some(item) = items.iter().find(|item| item["id"] == name) {
        return Ok(item["id"]
            .as_str()
            .context("Resource ID is invalid")?
            .into());
    }
    let matches = items
        .iter()
        .filter(|item| item["name"] == name)
        .collect::<Vec<_>>();
    ensure!(
        matches.len() == 1,
        "Resource name was not found or is ambiguous; use its ID"
    );
    Ok(matches[0]["id"]
        .as_str()
        .context("Resource ID is invalid")?
        .into())
}
async fn workspace(backend: &Backend, name: &str) -> Result<moleapi_core::Workspace> {
    let items = backend.request("GET", &["workspaces"], &[], None).await?;
    let id = id(items.as_array().context("Invalid workspace list")?, name)?;
    Ok(serde_json::from_value(
        backend
            .request("GET", &["workspaces", &id], &[], None)
            .await?,
    )?)
}
fn resource<T: serde::Serialize>(values: &[T], name: &str) -> Result<String> {
    let values = values
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<Vec<_>, _>>()?;
    id(&values, name)
}
async fn import(
    backend: &Backend,
    path: &Path,
    format: &str,
    name: Option<&str>,
) -> Result<moleapi_core::Workspace> {
    let text = io::read(path, 20 * 1024 * 1024)?;
    let parsed = moleapi_formats::import(format, &text).context("Import failed")?;
    let result = backend
        .request(
            "POST",
            &["workspaces"],
            &[],
            Some(json!({"name":name.unwrap_or(&parsed.name),"data":parsed.data})),
        )
        .await?;
    Ok(serde_json::from_value(result)?)
}
pub(crate) fn output(value: &Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
async fn report_file(
    backend: &Backend,
    workspace: &str,
    id: &str,
    format: &str,
    language: &str,
    path: Option<&Path>,
    overwrite: bool,
) -> Result<()> {
    let file = backend
        .request(
            "GET",
            &["workspaces", workspace, "reports", id, "export"],
            &[("format", format), ("language", language)],
            None,
        )
        .await?;
    let content = file["content"]
        .as_str()
        .context("Report export is invalid")?;
    if let Some(path) = path {
        io::save(path, content.as_bytes(), overwrite)?;
    } else {
        println!("{content}");
    }
    Ok(())
}
pub async fn execute(cli: &Cli, backend: &Backend) -> Result<u8> {
    match &cli.command {
        Command::Tokens { command } => {
            ensure!(
                cli.server.is_some(),
                "Access tokens require --server and a login session"
            );
            return crate::tokens::execute(backend, command).await;
        }
        Command::Schema | Command::Newman(_) | Command::Skill { .. } => unreachable!(),
        Command::Login {
            username,
            password_env,
            output,
            overwrite,
        } => {
            ensure!(cli.server.is_some(), "Login requires --server");
            let password = std::env::var(password_env)
                .context("Password environment variable is unavailable")?;
            let value = backend
                .request(
                    "POST",
                    &["auth", "login"],
                    &[],
                    Some(json!({"username":username,"password":password})),
                )
                .await?;
            let token = value["token"]
                .as_str()
                .context("Login response lacks a token")?;
            io::save(output, token.as_bytes(), *overwrite)?;
            output_message("Login token saved privately")?;
        }
        Command::List {
            kind,
            workspace: name,
        } => {
            if matches!(kind, Resource::Workspaces) {
                let values = backend.request("GET", &["workspaces"], &[], None).await?;
                let items=values.as_array().context("Invalid workspace list")?.iter().map(|value|json!({"id":value["id"],"name":value["name"],"revision":value["revision"]})).collect::<Vec<_>>();
                output(&json!(items))?;
            } else {
                let name = name
                    .as_ref()
                    .context("This resource requires --workspace")?;
                let data = workspace(backend, name).await?.data;
                let items=match kind{Resource::Requests=>data.collections.iter().flat_map(|collection|collection.requests.iter().map(move |request|json!({"id":request.id,"name":request.name,"method":request.method,"collection_id":collection.id,"collection_name":collection.name}))).collect::<Vec<_>>(),Resource::Collections=>data.collections.iter().map(|value|json!({"id":value.id,"name":value.name,"requests":value.requests.len()})).collect::<Vec<_>>(),Resource::Environments=>data.environments.iter().map(|value|json!({"id":value.id,"name":value.name,"variables":value.variables.len()})).collect(),Resource::Scenarios=>data.scenarios.iter().map(|value|json!({"id":value.id,"name":value.name,"collection_id":value.collection_id,"steps":value.steps.len()})).collect(),Resource::Datasets=>data.datasets.iter().map(|value|json!({"id":value.id,"name":value.name,"has_source":value.source.is_some()})).collect(),Resource::Workspaces=>unreachable!()};
                output(&json!(items))?;
            }
        }
        Command::Import {
            input,
            format,
            name,
        } => {
            let workspace = import(backend, input, format, name.as_deref()).await?;
            output(
                &json!({"id":workspace.id,"name":workspace.name,"revision":workspace.revision}),
            )?;
        }
        Command::Export {
            workspace: name,
            format,
            output: path,
            overwrite,
            include_secrets,
        } => {
            let workspace = workspace(backend, name).await?;
            let file = backend
                .request(
                    "POST",
                    &["workspaces", &workspace.id, "export"],
                    &[],
                    Some(json!({"format":format,"include_secrets":include_secrets})),
                )
                .await?;
            io::save(
                path,
                file["content"]
                    .as_str()
                    .context("Workspace export is invalid")?
                    .as_bytes(),
                *overwrite,
            )?;
        }
        Command::Run(options) => return run(cli, backend, options).await,
        Command::Report {
            workspace: name,
            id,
            format,
            language,
            output: path,
            overwrite,
        } => {
            let workspace = workspace(backend, name).await?;
            report_file(
                backend,
                &workspace.id,
                id,
                format,
                language,
                path.as_deref(),
                *overwrite,
            )
            .await?;
        }
        Command::Snippet {
            workspace: name,
            request,
            target,
            client,
            output: path,
            overwrite,
            include_secrets,
        } => {
            if name.is_none() && request.is_none() {
                output(
                    &backend
                        .request("GET", &["generation", "snippets", "catalog"], &[], None)
                        .await?,
                )?;
            } else {
                let workspace = workspace(
                    backend,
                    name.as_deref()
                        .context("Snippet generation requires --workspace")?,
                )
                .await?;
                let requests = workspace
                    .data
                    .collections
                    .iter()
                    .flat_map(|collection| &collection.requests)
                    .map(serde_json::to_value)
                    .collect::<Result<Vec<_>, _>>()?;
                let request = id(
                    &requests,
                    request
                        .as_deref()
                        .context("Snippet generation requires --request")?,
                )?;
                let snippet=backend.request("POST",&["generation","snippets"],&[],Some(json!({"workspace_id":workspace.id,"request_id":request,"target":target,"client":client,"include_secrets":include_secrets}))).await?;
                let code = snippet["code"]
                    .as_str()
                    .context("Snippet output is invalid")?;
                if let Some(path) = path {
                    io::save(path, code.as_bytes(), *overwrite)?;
                } else {
                    println!("{code}");
                }
                if let Some(warnings) = snippet["warnings"].as_array() {
                    for warning in warnings.iter().filter_map(Value::as_str) {
                        eprintln!("Warning: {warning}");
                    }
                }
            }
        }
    }
    Ok(0)
}
fn output_message(message: &str) -> Result<()> {
    output(&json!({"message":message}))
}
async fn run(_cli: &Cli, backend: &Backend, options: &Run) -> Result<u8> {
    let mut imported = None;
    let workspace = if let Some(input) = &options.input {
        let value = import(backend, input, &options.input_format, None).await?;
        imported = Some((value.id.clone(), value.revision));
        value
    } else {
        workspace(
            backend,
            options
                .workspace
                .as_deref()
                .context("Run requires --workspace or --input")?,
        )
        .await?
    };
    let result = run_workspace(backend, options, &workspace).await;
    if let Some((id, revision)) = imported {
        let cleanup = backend
            .request(
                "DELETE",
                &["workspaces", &id],
                &[],
                Some(json!({"expected_revision":revision})),
            )
            .await;
        if cleanup.is_err() {
            eprintln!("Temporary workspace cleanup failed; remove workspace {id} explicitly");
            if result.is_ok() {
                anyhow::bail!("Temporary workspace could not be removed");
            }
        }
    }
    result
}
async fn run_workspace(
    backend: &Backend,
    options: &Run,
    workspace: &moleapi_core::Workspace,
) -> Result<u8> {
    let scenario = options
        .scenario
        .as_deref()
        .map(|name| resource(&workspace.data.scenarios, name))
        .transpose()?;
    let collection = if let Some(name) = &options.collection {
        resource(&workspace.data.collections, name)?
    } else if let Some(scenario) = &scenario {
        workspace
            .data
            .scenarios
            .iter()
            .find(|value| value.id == *scenario)
            .context("Scenario disappeared")?
            .collection_id
            .clone()
    } else {
        ensure!(
            workspace.data.collections.len() == 1,
            "Select --collection explicitly when there is not exactly one collection"
        );
        workspace.data.collections[0].id.clone()
    };
    let request_ids = if options.requests.is_empty() {
        None
    } else {
        let selected = workspace
            .data
            .collections
            .iter()
            .find(|value| value.id == collection)
            .context("Collection disappeared")?;
        let subtree = moleapi_core::collection_subtree(&workspace.data, selected)?;
        let requests = subtree
            .iter()
            .flat_map(|value| &value.requests)
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()?;
        Some(
            options
                .requests
                .iter()
                .map(|name| id(&requests, name))
                .collect::<Result<Vec<_>>>()?,
        )
    };
    let environment = options
        .environment
        .as_deref()
        .map(|name| resource(&workspace.data.environments, name))
        .transpose()?;
    let dataset_id = options
        .dataset
        .as_deref()
        .map(|name| resource(&workspace.data.datasets, name))
        .transpose()?;
    let dataset = options
        .data_file
        .as_ref()
        .map(|path| {
            io::read(path, 1024 * 1024)
                .map(|source| json!({"format":options.data_format,"source":source}))
        })
        .transpose()?;
    let ticket = uuid::Uuid::new_v4().to_string();
    let mut body = json!({"collection_id":collection,"scenario_id":scenario,"request_ids":request_ids,"environment_id":environment,"dataset_id":dataset_id,"dataset":dataset,"iterations":options.iterations,"job_id":ticket,"run_origin":if options.ci{"ci"}else{"interactive"}});
    if options.no_notifications {
        body["notification_ids"] = json!([]);
    } else if !options.notify.is_empty() {
        body["notification_ids"] = json!(options.notify);
    }
    let run_path = ["workspaces", workspace.id.as_str(), "run"];
    let future = backend.request("POST", &run_path, &[], Some(body));
    tokio::pin!(future);
    let mut interrupted = false;
    let result = tokio::select! {
        value = &mut future => value,
        signal = tokio::signal::ctrl_c() => {
            signal.context("Cannot listen for cancellation")?;
            interrupted = true;
            let cancel_path = ["workspaces", workspace.id.as_str(), "run", "cancel"];
            let cancel = backend.request("POST", &cancel_path, &[], Some(json!({"job_id":ticket})));
            // Keep polling execution while cancellation waits for the database.
            let (result, _) = tokio::time::timeout(Duration::from_secs(15), async {
                tokio::join!(&mut future, cancel)
            }).await.context("Run cancellation follow-up exceeded 15 seconds")?;
            result
        }
    }?;
    if let Some(path) = &options.report {
        let id = result["report_id"]
            .as_str()
            .context("The run report was not saved")?;
        report_file(
            backend,
            &workspace.id,
            id,
            &options.reporter,
            &options.language,
            Some(path),
            options.overwrite,
        )
        .await?;
    }
    output(
        &json!({"passed":result["passed"],"failed":result["failed"],"skipped":result["skipped"],"elapsed_ms":result["elapsed_ms"],"report_id":if options.input.is_some(){Value::Null}else{result["report_id"].clone()},"temporary_workspace":options.input.is_some(),"stopped_reason":result["stopped_reason"],"executed_steps":result["executed_steps"],"report_save_error":result.get("report_save_error").map(|_|"Report could not be saved")}),
    )?;
    if interrupted {
        return Ok(130);
    }
    if result["failed"].as_u64().unwrap_or(0) > 0
        || result["stopped_reason"].is_string()
        || result["cancelled"] == true
        || result["executed_steps"].as_u64().unwrap_or(0) == 0
    {
        return Ok(1);
    }
    Ok(0)
}
