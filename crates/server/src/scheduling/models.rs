use crate::ApiError;
use chrono::{DateTime, Utc};
use croner::{
    Cron,
    parser::{CronParser, Seconds, Year},
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub name: String,
    pub cron: String,
    pub timezone: String,
    pub enabled: bool,
    pub collection_id: String,
    #[serde(default)]
    pub scenario_id: Option<String>,
    #[serde(default)]
    pub environment_id: Option<String>,
    #[serde(default)]
    pub dataset_id: Option<String>,
    #[serde(default)]
    pub iterations: Option<usize>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Job {
    #[serde(default)]
    pub cancel_requested: bool,
    pub id: String,
    pub started_at: String,
    pub lease_until: String,
    pub config_revision: i64,
    pub manual: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Queued {
    pub id: String,
    pub requested_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Occurrence {
    pub id: String,
    pub schedule_id: String,
    pub config_revision: i64,
    pub manual: bool,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub report_id: Option<String>,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Schedule {
    pub id: String,
    pub workspace_id: String,
    pub revision: i64,
    pub definition: Definition,
    pub next_run_at: Option<String>,
    pub queued: Option<Queued>,
    pub running: Option<Job>,
    pub last_run: Option<Occurrence>,
    pub created_at: String,
    pub updated_at: String,
}
impl Definition {
    pub fn parsed(&self) -> Result<(Cron, chrono_tz::Tz), ApiError> {
        if self.name.trim().is_empty()
            || self.name.len() > 256
            || self.cron.is_empty()
            || self.cron.len() > 256
            || self.timezone.len() > 128
            || self
                .iterations
                .is_some_and(|value| !(1..=100).contains(&value))
        {
            return Err(ApiError::bad(
                "Invalid schedule name, expression, timezone or iterations",
            ));
        }
        let cron = CronParser::builder()
            .seconds(Seconds::Optional)
            .year(Year::Disallowed)
            .build()
            .parse(&self.cron)
            .map_err(|_| ApiError::bad("Invalid cron expression"))?;
        let timezone = self
            .timezone
            .parse()
            .map_err(|_| ApiError::bad("Invalid IANA timezone"))?;
        Ok((cron, timezone))
    }
    pub fn next(&self, after: DateTime<Utc>) -> Result<DateTime<Utc>, ApiError> {
        let (cron, timezone) = self.parsed()?;
        cron.find_next_occurrence(&after.with_timezone(&timezone), false)
            .map(|value| value.with_timezone(&Utc))
            .map_err(|_| ApiError::bad("Cron has no next occurrence"))
    }
    pub fn validate_refs(&self, workspace: &moleapi_core::Workspace) -> Result<(), ApiError> {
        let collection = workspace
            .data
            .collections
            .iter()
            .find(|collection| collection.id == self.collection_id)
            .ok_or_else(|| ApiError::bad("Scheduled collection does not exist"))?;
        if let Some(id) = &self.scenario_id
            && !workspace
                .data
                .scenarios
                .iter()
                .any(|scenario| scenario.id == *id && scenario.collection_id == collection.id)
        {
            return Err(ApiError::bad(
                "Scheduled scenario does not belong to the collection",
            ));
        }
        if self.environment_id.as_ref().is_some_and(|id| {
            !workspace
                .data
                .environments
                .iter()
                .any(|environment| environment.id == *id)
        }) {
            return Err(ApiError::bad("Scheduled environment does not exist"));
        }
        if let Some(id) = &self.dataset_id {
            let source = workspace
                .data
                .datasets
                .iter()
                .find(|dataset| dataset.id == *id)
                .and_then(|dataset| dataset.source.as_ref())
                .ok_or_else(|| ApiError::bad("Scheduled dataset source does not exist"))?;
            let data = source
                .parse()
                .map_err(|_| ApiError::bad("Scheduled dataset is invalid"))?;
            if self.iterations.is_some_and(|count| count > data.rows.len()) {
                return Err(ApiError::bad("Scheduled iterations exceed dataset rows"));
            }
        }
        Ok(())
    }
}
fn timestamp(value: &str) -> i64 {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.timestamp_millis())
        .unwrap_or(i64::MAX)
}
impl Schedule {
    pub fn wake_at(&self) -> i64 {
        if let Some(job) = &self.running {
            return timestamp(&job.lease_until);
        }
        let queued = self
            .queued
            .as_ref()
            .map(|job| timestamp(&job.requested_at))
            .unwrap_or(i64::MAX);
        queued.min(if self.definition.enabled {
            self.next_run_at
                .as_deref()
                .map(timestamp)
                .unwrap_or(i64::MAX)
        } else {
            i64::MAX
        })
    }
}
pub(super) fn ledger_id(schedule: &str, job: &str) -> String {
    format!("sched-run-{schedule}-{job}")
}
#[cfg(test)]
mod tests {
    use super::*;
    fn definition(cron: &str, timezone: &str) -> Definition {
        Definition {
            name: "Time".into(),
            cron: cron.into(),
            timezone: timezone.into(),
            enabled: true,
            collection_id: "c".into(),
            scenario_id: None,
            environment_id: None,
            dataset_id: None,
            iterations: None,
        }
    }
    #[test]
    fn mature_cron_parser_supports_optional_seconds_timezone_and_dst() {
        let now = DateTime::parse_from_rfc3339("2026-03-08T06:59:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let next = definition("0 2 * * *", "America/New_York")
            .next(now)
            .unwrap();
        // Croner fixed-time jobs roll forward to the first real instant after a DST gap.
        assert_eq!(next.to_rfc3339(), "2026-03-08T07:00:00+00:00");
        let next = definition("*/5 * * * * *", "UTC").next(now).unwrap();
        assert_eq!(next.to_rfc3339(), "2026-03-08T06:59:05+00:00");
        assert!(definition("bad", "UTC").next(now).is_err());
        assert!(
            definition("0 9 * * *", "Unknown/Timezone")
                .next(now)
                .is_err()
        );
    }
}
