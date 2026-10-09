use crate::{Collection, RequestSpec, WorkspaceData};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub collection_id: String,
    pub steps: Vec<ScenarioStep>,
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
