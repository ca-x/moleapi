use crate::{args::RunnerCommand, backend::Backend, commands::output, io, variables};
use anyhow::{Context, Result, ensure};
use moleapi_core::RunnerClaim;
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
        RunnerCommand::Register { name } => {
            backend
                .request("POST", &["runners"], &[], Some(json!({"name":name})))
                .await?
        }
        RunnerCommand::List => backend.request("GET", &["runners"], &[], None).await?,
        RunnerCommand::Set {
            id,
            name,
            enabled,
            revision,
        } => {
            backend
                .request(
                    "PATCH",
                    &["runners", id],
                    &[],
                    Some(json!({"name":name,"enabled":enabled,"expected_revision":revision})),
                )
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
            &json!({"runner_id":runner,"task_id":claim.task.id,"status":completed.as_ref().map(|s|s["status"].clone()),"report_id":completed.as_ref().map(|s|s["report_id"].clone()),"completion_acknowledged":completed.is_some()}),
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
            result = &mut run => return (result,false),
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
