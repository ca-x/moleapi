//! Portable CI presets. Mature serializers/quoters own syntax and literal handling.
use anyhow::{Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::json;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Github,
    Gitlab,
    Jenkins,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Source {
    Remote {
        server: String,
        workspace: String,
    },
    File {
        path: String,
        #[serde(default = "native_format")]
        format: String,
    },
}
fn native_format() -> String {
    "moleapi".into()
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportFormat {
    #[default]
    Junit,
    Json,
    Html,
    Csv,
}
impl ReportFormat {
    pub fn name(&self) -> &str {
        match self {
            Self::Junit => "junit",
            Self::Json => "json",
            Self::Html => "html",
            Self::Csv => "csv",
        }
    }
    pub fn path(&self) -> String {
        format!(
            ".moleapi-ci/report.{}",
            match self {
                Self::Junit => "xml",
                _ => self.name(),
            }
        )
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum Notifications {
    #[default]
    Silent,
    Defaults,
    Selected {
        ids: Vec<String>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub provider: Provider,
    pub source: Source,
    pub collection: Option<String>,
    pub scenario: Option<String>,
    pub environment: Option<String>,
    pub dataset: Option<String>,
    pub data_file: Option<String>,
    #[serde(default = "csv")]
    pub data_format: String,
    pub iterations: Option<u16>,
    #[serde(default)]
    pub requests: Vec<String>,
    #[serde(default)]
    pub notifications: Notifications,
    #[serde(default)]
    pub reporter: ReportFormat,
    #[serde(default = "english")]
    pub language: String,
    #[serde(default = "secret")]
    pub secret_name: String,
    pub variables_secret: Option<String>,
    #[serde(default)]
    pub branches: Vec<String>,
}
fn csv() -> String {
    "csv".into()
}
fn english() -> String {
    "en".into()
}
fn secret() -> String {
    "MOLEAPI_TOKEN".into()
}
fn literal(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 1024
            && !value.chars().any(char::is_control)
            && !value.contains("${{"),
        "CI values must be nonempty bounded literals without controls or workflow expressions"
    );
    Ok(())
}
fn secret_reference(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            && !value.starts_with(|ch: char| ch.is_ascii_digit()),
        "Secret reference must be an ASCII identifier"
    );
    Ok(())
}
impl Config {
    pub fn arguments(&self) -> Result<Vec<String>> {
        ensure!(
            self.collection.is_some() ^ self.scenario.is_some(),
            "Select exactly one collection or scenario"
        );
        ensure!(
            self.dataset.is_none() || self.data_file.is_none(),
            "Select a saved dataset or data file, not both"
        );
        ensure!(
            self.scenario.is_none() || self.requests.is_empty(),
            "Scenario steps cannot be combined with request filters"
        );
        ensure!(
            self.iterations.is_none_or(|n| (1..=100).contains(&n)),
            "Iterations must be 1 to 100"
        );
        ensure!(
            self.requests.len() <= 1000
                && self.branches.len() <= 32
                && self
                    .requests
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    == self.requests.len(),
            "Too many CI selections"
        );
        ensure!(
            matches!(self.language.as_str(), "en" | "zh-CN")
                && matches!(self.data_format.as_str(), "csv" | "json"),
            "Unsupported report language or data format"
        );
        if let Some(reference) = &self.variables_secret {
            secret_reference(reference)?;
            if matches!(self.source, Source::Remote { .. }) {
                ensure!(
                    reference != &self.secret_name
                        && reference != "MOLEAPI_TOKEN"
                        && self.secret_name != "MOLEAPI_RUN_VARIABLES",
                    "Authentication and run-variable secret references must remain separate"
                );
            }
        }
        let mut args = vec!["moleapi-cli".into()];
        match &self.source {
            Source::Remote { server, workspace } => {
                literal(server)?;
                literal(workspace)?;
                let url = url::Url::parse(server)?;
                ensure!(
                    matches!(url.scheme(), "http" | "https")
                        && url.host_str().is_some()
                        && url.username().is_empty()
                        && url.password().is_none()
                        && url.query().is_none()
                        && url.fragment().is_none(),
                    "CI service URL must be HTTP(S) without URL credentials/query/fragment"
                );
                secret_reference(&self.secret_name)?;
                args.extend([
                    format!("--server={server}"),
                    "--token-env=MOLEAPI_TOKEN".into(),
                    "run".into(),
                    format!("--workspace={workspace}"),
                ]);
            }
            Source::File { path, format } => {
                ensure!(
                    matches!(self.notifications, Notifications::Silent),
                    "Notifications require a saved remote workspace"
                );
                literal(path)?;
                ensure!(
                    matches!(
                        format.as_str(),
                        "moleapi" | "postman" | "openapi" | "har" | "curl"
                    ),
                    "Unsupported collection format"
                );
                args.extend([
                    "run".into(),
                    format!("--input={path}"),
                    format!("--input-format={format}"),
                ]);
            }
        }
        for (flag, value) in [
            ("--collection", &self.collection),
            ("--scenario", &self.scenario),
            ("--environment", &self.environment),
            ("--dataset", &self.dataset),
            ("--data-file", &self.data_file),
        ] {
            if let Some(value) = value {
                literal(value)?;
                args.push(format!("{flag}={value}"));
            }
        }
        if self.data_file.is_some() {
            args.push(format!("--data-format={}", self.data_format));
        }
        if let Some(n) = self.iterations {
            args.push(format!("--iterations={n}"));
        }
        for value in &self.requests {
            literal(value)?;
            args.push(format!("--request={value}"));
        }
        match &self.notifications {
            Notifications::Silent => args.push("--no-notifications".into()),
            Notifications::Defaults => {}
            Notifications::Selected { ids } => {
                ensure!(
                    !ids.is_empty()
                        && ids.len() <= 20
                        && ids.iter().collect::<std::collections::BTreeSet<_>>().len() == ids.len(),
                    "Select 1 to 20 unique notification IDs"
                );
                for id in ids {
                    literal(id)?;
                    args.push(format!("--notify={id}"));
                }
            }
        }
        for branch in &self.branches {
            literal(branch)?;
        }
        if self.variables_secret.is_some() {
            args.push("--variables-env=MOLEAPI_RUN_VARIABLES".into());
        }
        args.extend([
            "--ci".into(),
            format!("--report={}", self.reporter.path()),
            format!("--reporter={}", self.reporter.name()),
            format!("--language={}", self.language),
        ]);
        Ok(args)
    }
}
#[derive(Serialize)]
pub struct Preset {
    pub filename: String,
    pub mime: String,
    pub content: String,
    pub command: Vec<String>,
    pub prerequisites: Vec<String>,
}
pub fn generate(config: &Config) -> Result<Preset> {
    let args = config.arguments()?;
    let command = shlex::try_join(args.iter().map(String::as_str))?;
    let report = config.reporter.path();
    let script = format!(
        "set -eu\nmkdir -p .moleapi-ci\nrm -f -- {}\n{command}\n",
        shlex::try_quote(&report)?
    );
    let remote = matches!(config.source, Source::Remote { .. });
    let mut bindings = vec![];
    if remote {
        bindings.push(("MOLEAPI_TOKEN", config.secret_name.as_str()));
    }
    if let Some(reference) = &config.variables_secret {
        bindings.push(("MOLEAPI_RUN_VARIABLES", reference.as_str()));
    }
    let (filename, mime, content) = match config.provider {
        Provider::Github => {
            let mut trigger = json!({"push":{},"pull_request":{},"workflow_dispatch":{}});
            if !config.branches.is_empty() {
                // GitHub branch filters use glob syntax; these selectors are exact names.
                let branches = config
                    .branches
                    .iter()
                    .map(|branch| {
                        let escaped = regex::escape(branch);
                        if escaped.starts_with('!') {
                            format!("\\{escaped}")
                        } else {
                            escaped
                        }
                    })
                    .collect::<Vec<_>>();
                trigger["push"]["branches"] = json!(branches);
                trigger["pull_request"]["branches"] = json!(branches);
            }
            let mut run =
                json!({"name":"Run API tests","id":"api-tests","shell":"bash","run":script});
            if !bindings.is_empty() {
                let values = bindings
                    .iter()
                    .map(|(variable, reference)| {
                        (
                            (*variable).into(),
                            json!(format!("${{{{ secrets.{reference} }}}}")),
                        )
                    })
                    .collect::<serde_json::Map<String, serde_json::Value>>();
                run["env"] = serde_json::Value::Object(values);
            }
            let mut job = json!({"runs-on":["self-hosted","linux"],"steps":[{"uses":"actions/checkout@v4"},run,{"name":"Archive API report","if":"always()","uses":"actions/upload-artifact@v4","with":{"name":"moleapi-report","path":report,"if-no-files-found":"ignore"}}]});
            if !bindings.is_empty() {
                job["if"] = json!(
                    "github.event_name != 'pull_request' || github.event.pull_request.head.repo.full_name == github.repository"
                );
            }
            (
                "moleapi-tests.yml",
                "application/yaml",
                serde_yaml_ng::to_string(
                    &json!({"name":"MoleAPI tests","on":trigger,"permissions":{"contents":"read"},"jobs":{"api-tests":job}}),
                )?,
            )
        }
        Provider::Gitlab => {
            let branch = if config.branches.is_empty() {
                String::new()
            } else {
                let alternatives = config
                    .branches
                    .iter()
                    .map(|branch| regex::escape(branch).replace('/', "\\/"))
                    .collect::<Vec<_>>()
                    .join("|");
                format!(
                    " && ($CI_COMMIT_BRANCH =~ /^({alternatives})$/ || $CI_MERGE_REQUEST_TARGET_BRANCH_NAME =~ /^({alternatives})$/)"
                )
            };
            let mut artifact = json!({"when":"always","paths":[report]});
            if matches!(config.reporter, ReportFormat::Junit) {
                artifact["reports"] = json!({"junit":report});
            }
            let mut job = json!({"stage":"test","script":[script],"rules":[{"if":format!("($CI_PIPELINE_SOURCE == \"push\" || $CI_PIPELINE_SOURCE == \"merge_request_event\" || $CI_PIPELINE_SOURCE == \"web\"){branch}")}],"artifacts":artifact});
            let aliases = bindings
                .iter()
                .filter(|(variable, reference)| variable != reference)
                .map(|(variable, reference)| ((*variable).into(), json!(format!("${reference}"))))
                .collect::<serde_json::Map<String, serde_json::Value>>();
            if !aliases.is_empty() {
                job["variables"] = serde_json::Value::Object(aliases);
            }
            (
                "moleapi.gitlab-ci.yml",
                "application/yaml",
                serde_yaml_ng::to_string(&json!({"stages":["test"],"moleapi-tests":job}))?,
            )
        }
        Provider::Jenkins => {
            let encoded = STANDARD.encode(script.as_bytes());
            let step = format!("sh(script: new String('{encoded}'.decodeBase64(), 'UTF-8'))");
            let run = if !bindings.is_empty() {
                let credentials = bindings
                    .iter()
                    .map(|(variable, reference)| {
                        format!("string(credentialsId: '{reference}', variable: '{variable}')")
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("withCredentials([{credentials}]) {{\n            {step}\n          }}")
            } else {
                step
            };
            let filter = if config.branches.is_empty() {
                String::new()
            } else {
                let branches = STANDARD.encode(serde_json::to_vec(&config.branches)?);
                format!(
                    "\n      when {{ expression {{ def selected = new groovy.json.JsonSlurper().parseText(new String('{branches}'.decodeBase64(), 'UTF-8')); selected.contains(env.CHANGE_TARGET ?: env.BRANCH_NAME) }} }}"
                )
            };
            let publish = if matches!(config.reporter, ReportFormat::Junit) {
                format!("\n      junit testResults: '{report}', allowEmptyResults: true")
            } else {
                String::new()
            };
            (
                "Jenkinsfile.moleapi",
                "text/plain",
                format!(
                    "pipeline {{\n  agent any\n  triggers {{ pollSCM('H/5 * * * *') }}\n  stages {{\n    stage('MoleAPI tests') {{{filter}\n      steps {{\n        script {{\n          {run}\n        }}\n      }}\n      post {{\n        always {{\n          archiveArtifacts artifacts: '{report}', allowEmptyArchive: true{publish}\n        }}\n      }}\n    }}\n  }}\n}}\n"
                ),
            )
        }
    };
    let mut prerequisites = vec![
        "Use a POSIX runner with moleapi-cli installed and check out the repository before execution.".into(),
    ];
    if !bindings.is_empty() {
        prerequisites.push("Configure the named CI secret/string credentials with the hosted token and/or private run-variable JSON; generation never includes their values.".into());
    }
    if matches!(config.provider, Provider::Jenkins) {
        prerequisites.push("Use Pipeline from SCM for commit polling; branch selection requires a multibranch job. Jenkins needs Credentials Binding and the selected artifact/JUnit publishers.".into());
    }
    Ok(Preset {
        filename: filename.into(),
        mime: mime.into(),
        content,
        command: args,
        prerequisites,
    })
}
