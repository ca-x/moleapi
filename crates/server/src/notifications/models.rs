use crate::ApiError;
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Settings {
    pub name: String,
    pub kind: String,
    pub enabled: bool,
    pub statuses: Vec<String>,
    #[serde(default)]
    pub changes_only: bool,
    #[serde(default = "english")]
    pub language: String,
}
fn english() -> String {
    "en".into()
}
impl Settings {
    pub fn validate(&self) -> Result<(), ApiError> {
        if self.name.trim().is_empty()
            || self.name.len() > 256
            || !matches!(
                self.kind.as_str(),
                "webhook"
                    | "slack"
                    | "teams"
                    | "wecom"
                    | "dingtalk"
                    | "feishu"
                    | "jenkins"
                    | "pagerduty"
                    | "email"
            )
            || !matches!(self.language.as_str(), "en" | "zh-CN")
            || self.statuses.is_empty()
            || self.statuses.len() > 4
            || self.statuses.iter().any(|status| {
                !matches!(
                    status.as_str(),
                    "passed" | "failed" | "cancelled" | "interrupted"
                )
            })
        {
            return Err(ApiError::bad(
                "Invalid notification name, kind, status filter or language",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Smtp {
    pub host: String,
    pub port: u16,
    pub tls: String,
    pub from: String,
    pub to: Vec<String>,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Credentials {
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub signing_secret: String,
    #[serde(default)]
    pub routing_key: String,
    #[serde(default)]
    pub smtp: Option<Smtp>,
}
impl Credentials {
    pub fn validate(&self, kind: &str) -> Result<(), ApiError> {
        if self.endpoint.len() > 8192
            || self.signing_secret.len() > 4096
            || self.routing_key.len() > 4096
        {
            return Err(ApiError::bad("Notification credentials exceed limits"));
        }
        if kind == "email" {
            let smtp = self
                .smtp
                .as_ref()
                .ok_or_else(|| ApiError::bad("SMTP settings are required"))?;
            let mut url = url::Url::parse("https://smtp.invalid").unwrap();
            url.set_host(Some(&smtp.host))
                .map_err(|_| ApiError::bad("Invalid SMTP host"))?;
            if smtp.host.is_empty()
                || smtp.host.len() > 253
                || smtp.port == 0
                || !matches!(smtp.tls.as_str(), "starttls" | "tls" | "none")
                || smtp.to.is_empty()
                || smtp.to.len() > 20
                || smtp.from.len() > 1024
                || smtp.to.iter().any(|value| value.len() > 1024)
                || (smtp.username.is_empty() && !smtp.password.is_empty())
                || smtp.username.len() > 1024
                || smtp.password.len() > 4096
            {
                return Err(ApiError::bad("Invalid SMTP settings"));
            }
            smtp.from
                .parse::<lettre::message::Mailbox>()
                .map_err(|_| ApiError::bad("Invalid SMTP sender"))?;
            for recipient in &smtp.to {
                recipient
                    .parse::<lettre::message::Mailbox>()
                    .map_err(|_| ApiError::bad("Invalid SMTP recipient"))?;
            }
        } else {
            moleapi_core::valid_url(&self.endpoint)
                .map_err(|_| ApiError::bad("Invalid notification URL"))?;
            if kind == "pagerduty" && self.routing_key.is_empty() {
                return Err(ApiError::bad("PagerDuty requires a routing key"));
            }
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Target {
    pub id: String,
    pub workspace_id: String,
    pub revision: i64,
    pub settings: Settings,
}
#[derive(Serialize)]
pub(crate) struct Metadata {
    pub id: String,
    pub revision: i64,
    pub settings: Settings,
    pub origin: Option<String>,
    pub has_endpoint: bool,
    pub has_signing_secret: bool,
    pub has_routing_key: bool,
    pub smtp: Option<SmtpMetadata>,
}
#[derive(Serialize)]
pub(crate) struct SmtpMetadata {
    pub host: String,
    pub port: u16,
    pub tls: String,
    pub from: String,
    pub to: Vec<String>,
    pub has_username: bool,
    pub has_password: bool,
}
impl Target {
    pub fn metadata(&self, credentials: &Credentials) -> Metadata {
        Metadata {
            id: self.id.clone(),
            revision: self.revision,
            settings: self.settings.clone(),
            origin: url::Url::parse(&credentials.endpoint)
                .ok()
                .map(|url| url.origin().ascii_serialization()),
            has_endpoint: !credentials.endpoint.is_empty(),
            has_signing_secret: !credentials.signing_secret.is_empty(),
            has_routing_key: !credentials.routing_key.is_empty(),
            smtp: credentials.smtp.as_ref().map(|smtp| SmtpMetadata {
                host: smtp.host.clone(),
                port: smtp.port,
                tls: smtp.tls.clone(),
                from: smtp.from.clone(),
                to: smtp.to.clone(),
                has_username: !smtp.username.is_empty(),
                has_password: !smtp.password.is_empty(),
            }),
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Event {
    pub id: String,
    pub schedule_id: String,
    pub job_id: String,
    pub status: String,
    pub previous_status: Option<String>,
    pub finished_at: String,
    pub report_id: Option<String>,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub workspace_name: String,
    pub schedule_name: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Delivery {
    pub id: String,
    pub target_id: String,
    pub target_revision: i64,
    pub event: Event,
    pub status: String,
    pub attempts: usize,
    pub next_attempt_at: String,
    pub lease_until: Option<String>,
    pub lease_id: Option<String>,
    pub last_http_status: Option<u16>,
    pub last_error: Option<String>,
    pub delivered_at: Option<String>,
}
impl Delivery {
    pub fn wake_at(&self) -> i64 {
        let time = if self.status == "sending" {
            self.lease_until.as_ref()
        } else if self.status == "pending" {
            Some(&self.next_attempt_at)
        } else {
            None
        };
        time.and_then(|time| chrono::DateTime::parse_from_rfc3339(time).ok())
            .map(|time| time.timestamp_millis())
            .unwrap_or(i64::MAX)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CredentialsPatch {
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub signing_secret: Option<String>,
    #[serde(default)]
    pub routing_key: Option<String>,
    #[serde(default)]
    pub smtp: Option<SmtpPatch>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SmtpPatch {
    pub host: String,
    pub port: u16,
    pub tls: String,
    pub from: String,
    pub to: Vec<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
}
impl CredentialsPatch {
    pub fn apply(self, mut previous: Credentials) -> Credentials {
        if let Some(value) = self.endpoint {
            previous.endpoint = value;
        }
        if let Some(value) = self.signing_secret {
            previous.signing_secret = value;
        }
        if let Some(value) = self.routing_key {
            previous.routing_key = value;
        }
        if let Some(value) = self.smtp {
            let old = previous.smtp.take();
            previous.smtp = Some(Smtp {
                host: value.host,
                port: value.port,
                tls: value.tls,
                from: value.from,
                to: value.to,
                username: value.username.unwrap_or_else(|| {
                    old.as_ref()
                        .map(|smtp| smtp.username.clone())
                        .unwrap_or_default()
                }),
                password: value.password.unwrap_or_else(|| {
                    old.as_ref()
                        .map(|smtp| smtp.password.clone())
                        .unwrap_or_default()
                }),
            });
        }
        previous
    }
}
