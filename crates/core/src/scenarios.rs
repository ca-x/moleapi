use crate::{Collection, RequestSpec, WorkspaceData};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notification_ids: Vec<String>,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub collection_id: String,
    pub steps: Vec<ScenarioStep>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parallel: Vec<ScenarioParallel>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioParallel {
    pub id: String,
    pub name: String,
    pub step_ids: Vec<String>,
    #[serde(default = "parallel_concurrency")]
    pub concurrency: usize,
}
fn parallel_concurrency() -> usize {
    4
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioStep {
    pub id: String,
    pub request_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub group: String,
    #[serde(default = "enabled")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    #[serde(default = "once")]
    pub repeat: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_true: Option<ScenarioTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_false: Option<ScenarioTarget>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScenarioTarget {
    Step { step_id: String },
    Stop,
}
fn once() -> usize {
    1
}
fn enabled() -> bool {
    true
}

pub fn validate_scenarios(data: &WorkspaceData) -> Result<()> {
    ensure!(
        data.scenarios.len() <= 100,
        "Workspace exceeds 100 scenarios"
    );
    let mut ids = BTreeSet::new();
    let mut count = 0usize;
    for scenario in &data.scenarios {
        let notifications = scenario.notification_ids.iter().collect::<BTreeSet<_>>();
        ensure!(
            scenario.notification_ids.len() <= 20
                && notifications.len() == scenario.notification_ids.len()
                && notifications
                    .iter()
                    .all(|id| !id.is_empty() && id.len() <= 128),
            "Scenario notification IDs must be unique, bounded and nonempty"
        );
        ensure!(
            !scenario.id.is_empty() && scenario.id.len() <= 128 && ids.insert(&scenario.id),
            "Scenario IDs must be unique and nonempty"
        );
        ensure!(
            !scenario.name.trim().is_empty()
                && scenario.name.len() <= 256
                && scenario.description.len() <= 4096,
            "Invalid scenario name or description"
        );
        count = count.saturating_add(scenario.steps.len());
        ensure!(count <= 1000, "Workspace exceeds 1000 scenario steps");
        let root = data
            .collections
            .iter()
            .find(|collection| collection.id == scenario.collection_id)
            .ok_or_else(|| anyhow::anyhow!("Scenario collection does not exist"))?;
        let subtree = crate::collection_subtree(data, root)?;
        let requests = subtree
            .iter()
            .flat_map(|collection| &collection.requests)
            .map(|request| &request.id)
            .collect::<BTreeSet<_>>();
        ensure!(
            scenario.parallel.len() <= 100,
            "Scenario exceeds 100 parallel blocks"
        );
        let active = scenario
            .steps
            .iter()
            .filter(|step| step.enabled)
            .collect::<Vec<_>>();
        let mut block_ids = BTreeSet::new();
        let mut members = BTreeSet::new();
        let mut interiors = BTreeSet::new();
        for block in &scenario.parallel {
            ensure!(
                !block.id.is_empty() && block.id.len() <= 128 && block_ids.insert(&block.id),
                "Parallel block IDs must be unique and nonempty"
            );
            ensure!(
                !block.name.trim().is_empty()
                    && block.name.len() <= 256
                    && (1..=4).contains(&block.concurrency),
                "Invalid parallel block name or concurrency"
            );
            ensure!(
                (2..=8).contains(&block.step_ids.len()),
                "Parallel block requires 2 to 8 steps"
            );
            let mut previous = None;
            for (offset, id) in block.step_ids.iter().enumerate() {
                ensure!(members.insert(id), "Parallel blocks cannot overlap");
                let index = active
                    .iter()
                    .position(|step| step.id == *id)
                    .ok_or_else(|| {
                        anyhow::anyhow!("Parallel member must be an enabled scenario step")
                    })?;
                ensure!(
                    previous.is_none_or(|previous| index == previous + 1),
                    "Parallel members must follow consecutive saved order"
                );
                previous = Some(index);
                ensure!(
                    active[index].on_true.is_none() && active[index].on_false.is_none(),
                    "Parallel members cannot redirect shared flow"
                );
                if offset > 0 {
                    interiors.insert(id);
                }
            }
        }
        let mut steps = BTreeSet::new();
        for step in &scenario.steps {
            ensure!(
                !step.id.is_empty() && step.id.len() <= 128 && steps.insert(&step.id),
                "Scenario step IDs must be unique and nonempty"
            );
            ensure!(
                step.name.len() <= 256 && step.group.len() <= 256,
                "Invalid scenario step label or group"
            );
            ensure!(
                (1..=1000).contains(&step.repeat),
                "Scenario repeat must be 1 to 1000"
            );
            if let Some(condition) = &step.condition {
                ensure!(
                    !condition.trim().is_empty() && condition.len() <= 4096,
                    "Invalid scenario condition"
                );
            }
            for target in [&step.on_true, &step.on_false].into_iter().flatten() {
                if let ScenarioTarget::Step { step_id } = target {
                    let selected = scenario
                        .steps
                        .iter()
                        .find(|candidate| candidate.id == *step_id)
                        .ok_or_else(|| anyhow::anyhow!("Scenario branch step does not exist"))?;
                    ensure!(
                        !step.enabled || selected.enabled,
                        "Enabled scenario branch targets a disabled step"
                    );
                    ensure!(
                        !interiors.contains(step_id),
                        "Scenario branch must enter the first parallel member"
                    );
                }
            }
            ensure!(
                requests.contains(&step.request_id),
                "Scenario request does not exist in selected subtree"
            );
        }
    }
    Ok(())
}

pub fn scenario_plan<'a>(
    data: &'a WorkspaceData,
    scenario: &'a Scenario,
) -> Result<Vec<(&'a Collection, &'a RequestSpec, &'a ScenarioStep)>> {
    let root = data
        .collections
        .iter()
        .find(|collection| collection.id == scenario.collection_id)
        .ok_or_else(|| anyhow::anyhow!("Scenario collection does not exist"))?;
    let subtree = crate::collection_subtree(data, root)?;
    let mut plan = Vec::new();
    for step in scenario.steps.iter().filter(|step| step.enabled) {
        let (collection, request) = subtree
            .iter()
            .find_map(|collection| {
                collection
                    .requests
                    .iter()
                    .find(|request| request.id == step.request_id)
                    .map(|request| (*collection, request))
            })
            .ok_or_else(|| {
                anyhow::anyhow!("Scenario request does not exist in selected subtree")
            })?;
        plan.push((collection, request, step));
    }
    ensure!(
        !plan.is_empty(),
        "Scenario requires an enabled request step"
    );
    Ok(plan)
}
