use crate::{
    args::{RunnerCommand, RunnerCredentialCommand},
    backend::Backend,
    commands::output,
    io, variables,
};
use anyhow::{Context, Result, ensure};
use moleapi_core::{HistoryEntry, RunnerClaim};
use serde_json::{Value, json};
use std::time::Duration;
async fn control(backend: &Backend, path: &[&str], body: Option<Value>) -> Result<Value> {
    tokio::time::timeout(
        Duration::from_secs(10),
        backend.request("POST", path, &[], body),
    )
    .await
    .context("Runner control request timed out")?
}
pub async fn execute(backend: &Backend, command: &RunnerCommand) -> Result<u8> {
    ensure!(
        matches!(backend, Backend::Remote { .. }),
        "Runner commands require a hosted service"
    );
    let value = match command {
        RunnerCommand::Register { name, labels } => {
            backend
                .request(
                    "POST",
                    &["runners"],
                    &[],
                    Some(json!({"name":name,"labels":labels})),
                )
                .await?
        }
        RunnerCommand::List => backend.request("GET", &["runners"], &[], None).await?,
        RunnerCommand::Credentials { command } => return credentials(backend, command).await,
        RunnerCommand::Set {
            id,
            name,
            enabled,
            revision,
            labels,
            clear_labels,
        } => {
            let mut body = json!({"name":name,"enabled":enabled,"expected_revision":revision});
            if *clear_labels {
                body["labels"] = json!([]);
            } else if !labels.is_empty() {
                body["labels"] = json!(labels);
            }
            backend
                .request("PATCH", &["runners", id], &[], Some(body))
                .await?
        }
        RunnerCommand::Queue { workspace, config } => {
            let body: Value = serde_json::from_str(&io::read(config, 1024 * 1024)?)
                .context("Invalid runner task configuration")?;
            backend
                .request(
                    "POST",
                    &["workspaces", workspace, "runner-tasks"],
                    &[],
                    Some(body),
                )
                .await?
        }
        RunnerCommand::Tasks { workspace } => {
            backend
                .request("GET", &["workspaces", workspace, "runner-tasks"], &[], None)
                .await?
        }
        RunnerCommand::Cancel { workspace, id } => {
            backend
                .request(
                    "POST",
                    &["workspaces", workspace, "runner-tasks", id, "cancel"],
                    &[],
                    None,
                )
                .await?
        }
        RunnerCommand::Agent {
            id,
            once,
            variables_file,
            variables_env,
        } => {
            let overrides = variables::read(variables_file.as_deref(), variables_env.as_deref())?;
            return agent(backend, id, *once, &overrides).await;
        }
    };
    output(&value)?;
    Ok(0)
}

async fn credentials(backend: &Backend, command: &RunnerCredentialCommand) -> Result<u8> {
    match command {
        RunnerCredentialCommand::List { runner } => output(
            &backend
                .request("GET", &["runners", runner, "credentials"], &[], None)
                .await?,
        )?,
        RunnerCredentialCommand::Create {
            runner,
            workspace,
            name,
            days,
            output: path,
            overwrite,
        } => {
            let file = io::OutputFile::prepare(path, *overwrite)?;
            let mut value = backend
                .request(
                    "POST",
                    &["runners", runner, "credentials"],
                    &[],
                    Some(json!({"workspace_id":workspace,"name":name,"expires_in_days":days})),
                )
                .await?;
            let secret = value
                .as_object_mut()
                .context("Invalid credential response")?
                .remove("token")
                .context("Credential response missing its secret")?;
            let secret = secret.as_str().context("Invalid credential secret")?;
            if let Err(error) = file.save(secret.as_bytes()) {
                if let Some(id) = value["id"].as_str() {
                    if backend
                        .request("DELETE", &["runners", runner, "credentials", id], &[], None)
                        .await
                        .is_err()
                    {
                        eprintln!(
                            "Output failed and credential cleanup failed; revoke credential {id} explicitly"
                        );
                    }
                }
                return Err(error);
            }
            output(&value)?;
        }
        RunnerCredentialCommand::Revoke { runner, id } => output(
            &backend
                .request("DELETE", &["runners", runner, "credentials", id], &[], None)
                .await?,
        )?,
    }
    Ok(0)
}
async fn agent(
    remote: &Backend,
    runner: &str,
    once: bool,
    overrides: &variables::Overrides,
) -> Result<u8> {
    let claim_path = ["runners", runner, "claim"];
    loop {
        let claimed = tokio::select! {
            result = control(remote, &claim_path, None) => result?,
            signal = tokio::signal::ctrl_c() => { signal.context("Cannot listen for cancellation")?; return Ok(130); }
        };
        if claimed.is_null() {
            if once {
                output(&json!({"runner_id":runner,"claimed":false}))?;
                return Ok(0);
            }
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(2)) => {},
                signal = tokio::signal::ctrl_c() => { signal.context("Cannot listen for cancellation")?; return Ok(130); }
            }
            continue;
        }
        let claim: RunnerClaim =
            serde_json::from_value(claimed).context("Invalid runner claim response")?;
        ensure!(
            claim.task.runner_id == runner && claim.workspace.id == claim.task.workspace_id,
            "Runner claim source mismatch"
        );
        let (result, interrupted) = task(remote, runner, &claim, overrides).await;
        let body = match &result {
            Ok(report) if !interrupted => json!({"lease_token":claim.lease_token,"report":report}),
            _ => {
                json!({"lease_token":claim.lease_token,"status":if interrupted{"cancelled"}else{"failed"}})
            }
        };
        let path = ["runners", runner, "tasks", &claim.task.id, "complete"];
        let mut completed = None;
        for attempt in 0..3 {
            match control(remote, &path, Some(body.clone())).await {
                Ok(summary) => {
                    completed = Some(summary);
                    break;
                }
                Err(_) if attempt < 2 => tokio::time::sleep(Duration::from_secs(2)).await,
                Err(_) => {}
            }
        }
        output(
            &json!({"runner_id":runner,"task_id":claim.task.id,"status":completed.as_ref().map(|s|s["status"].clone()),"report_id":completed.as_ref().map(|s|s["report_id"].clone()),"completion_acknowledged":completed.is_some(),"response_history_omitted":result.as_ref().ok().and_then(|r|r["history_upload_omitted"].as_u64()).unwrap_or(0)}),
        )?;
        if interrupted {
            return Ok(130);
        }
        if completed.is_none() {
            anyhow::bail!(
                "Runner completion was not acknowledged; inspect the task before retrying"
            );
        }
        if once {
            return Ok(match result {
                Ok(report)
                    if report["failed"].as_u64().unwrap_or(0) == 0
                        && report["executed_steps"].as_u64().unwrap_or(0) > 0
                        && report["cancelled"] != true =>
                {
                    0
                }
                _ => 1,
            });
        }
    }
}
async fn task(
    remote: &Backend,
    runner: &str,
    claim: &RunnerClaim,
    overrides: &variables::Overrides,
) -> (Result<Value>, bool) {
    let prepared = async {
        let temporary = tempfile::tempdir().context("Cannot prepare isolated runner storage")?;
        let local = Backend::local(&temporary.path().join("task.db")).await?;
        let source = &claim.workspace;
        local
            .request(
                "POST",
                &["workspaces"],
                &[],
                Some(json!({"id":source.id,"name":source.name,"data":source.data})),
            )
            .await?;
        Ok::<_, anyhow::Error>((temporary, local))
    }
    .await;
    let (_temporary, local) = match prepared {
        Ok(value) => value,
        Err(error) => return (Err(error), false),
    };
    let selection = &claim.task.selection;
    if !selection.environment_id.is_some()
        && claim.workspace.data.active_environment_id.is_none()
        && overrides.locals.iter().any(|v| v.scope == "environment")
    {
        return (
            Err(anyhow::anyhow!(
                "Environment overrides require a selected or active profile"
            )),
            false,
        );
    }
    let ticket = uuid::Uuid::new_v4().to_string();
    let body = json!({"collection_id":selection.collection_id,"scenario_id":selection.scenario_id,"environment_id":selection.environment_id,"dataset_id":selection.dataset_id,"iterations":selection.iterations,"request_ids":selection.request_ids,"notification_ids":[],"run_origin":"ci","job_id":ticket,"variables":overrides.temporary,"locals":overrides.locals});
    let path = ["workspaces", claim.workspace.id.as_str(), "run"];
    let run = local.request("POST", &path, &[], Some(body));
    tokio::pin!(run);
    let mut heartbeat = tokio::time::interval(Duration::from_secs(20));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    heartbeat.tick().await;
    let interrupted = loop {
        tokio::select! {
            result = &mut run => return (match result { Ok(report) => attach_response_histories(&local, &claim.workspace.id, report).await, Err(error) => Err(error) },false),
            signal = tokio::signal::ctrl_c() => {
                if let Err(error) = signal { return (Err(error.into()),false); }
                break true;
            }
            _ = heartbeat.tick() => {
                let lease = json!({"lease_token":claim.lease_token});
                if control(remote, &["runners",runner,"tasks",&claim.task.id,"heartbeat"], Some(lease)).await.is_err() { break false; }
            }
        }
    };
    let cancel_path = ["workspaces", claim.workspace.id.as_str(), "run", "cancel"];
    let cancellation = local.request("POST", &cancel_path, &[], Some(json!({"job_id":ticket})));
    let cancelled = tokio::time::timeout(Duration::from_secs(15), async {
        tokio::join!(&mut run, cancellation)
    })
    .await;
    let result = match cancelled {
        Ok((_, _)) => Err(anyhow::anyhow!(
            "Runner execution cancelled after interrupt or lost lease"
        )),
        Err(_) => Err(anyhow::anyhow!("Runner cancellation timed out")),
    };
    (result, interrupted)
}

async fn attach_response_histories(
    local: &Backend,
    workspace: &str,
    mut report: Value,
) -> Result<Value> {
    const BUDGET: usize = 8 * 1024 * 1024;
    let references: Vec<(usize, String)> = report["results"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(position, item)| {
            item["history_id"]
                .as_str()
                .map(|id| (position, id.to_owned()))
        })
        .collect();
    let Some(report_id) = report["report_id"].as_str().map(str::to_owned) else {
        report["history_upload_omitted"] = references.len().into();
        return Ok(report);
    };
    let mut uploads = Vec::new();
    let mut used = 2usize;
    let mut omitted = 0usize;
    for (position, id) in references {
        let position_text = position.to_string();
        let entry = local
            .request(
                "GET",
                &[
                    "workspaces",
                    workspace,
                    "reports",
                    &report_id,
                    "steps",
                    &position_text,
                    "response",
                ],
                &[],
                None,
            )
            .await
            .ok()
            .and_then(|value| serde_json::from_value::<HistoryEntry>(value).ok())
            .filter(|entry| entry.id == id && entry.workspace_id == workspace);
        if let Some(entry) = entry {
            let upload = json!({"position":position,"entry":entry});
            let bytes = serde_json::to_vec(&upload)
                .context("Cannot encode redacted response history")?
                .len()
                + usize::from(!uploads.is_empty());
            if used.saturating_add(bytes) <= BUDGET {
                used += bytes;
                uploads.push(upload);
                continue;
            }
        }
        omitted += 1;
    }
    report["history_uploads"] = json!(uploads);
    report["history_upload_omitted"] = omitted.into();
    Ok(report)
}

#[cfg(test)]
mod response_history_tests {
    use super::*;
    use axum::{Json, Router, routing::get};

    #[tokio::test]
    async fn oversized_local_response_is_omitted_without_leaking_its_body() {
        let body = "x".repeat(8 * 1024 * 1024);
        let size = body.len();
        let entry = json!({"id":"local","workspace_id":"w","request_id":"r","request_name":"Request","method":"GET","url":"https://example.com/","status":200,"elapsed_ms":1,"size_bytes":size,"created_at":"2026-10-13T00:00:00Z","response":{"status":200,"status_text":"OK","headers":[],"body":body,"elapsed_ms":1,"size_bytes":size,"truncated":false,"url":"https://example.com/","tests":[]}});
        let router = Router::new().route(
            "/api/workspaces/w/reports/report/steps/0/response",
            get(move || {
                let entry = entry.clone();
                async move { Json(entry) }
            }),
        );
        let report =
            json!({"report_id":"report","results":[{"request_id":"r","history_id":"local"}]});
        let result = attach_response_histories(&Backend::Local(router), "w", report)
            .await
            .unwrap();
        assert_eq!(result["history_uploads"], json!([]));
        assert_eq!(result["history_upload_omitted"], 1);
        assert!(!result.to_string().contains(&"x".repeat(1024)));
    }
}
