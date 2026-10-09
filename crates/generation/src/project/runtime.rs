use super::*;
use anyhow::{Context, bail, ensure};
use std::{path::PathBuf, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};
use tokio_util::sync::CancellationToken;
#[derive(Clone)]
pub struct ProjectRuntime {
    pub worker: PathBuf,
    pub java: Option<PathBuf>,
    pub protoc: Option<PathBuf>,
    pub grpc_plugins: std::collections::BTreeMap<String, PathBuf>,
}
impl ProjectRuntime {
    pub fn new(worker: PathBuf, java: Option<PathBuf>) -> Result<Self> {
        ensure!(
            worker.is_absolute() && worker.is_file(),
            "Project worker must be an explicit application executable"
        );
        if let Some(path) = &java {
            ensure!(
                path.is_absolute() && path.is_file(),
                "Java executable must be explicitly configured as an absolute file path"
            );
        }
        Ok(Self {
            worker,
            java,
            protoc: None,
            grpc_plugins: std::collections::BTreeMap::new(),
        })
    }
    pub fn java_available(&self) -> bool {
        self.java.is_some() && !JAR.is_empty()
    }
    pub async fn generate(
        &self,
        input: ProjectInput,
        cancel: CancellationToken,
    ) -> Result<ProjectArtifact> {
        if !is_protobuf_target(&input.target) {
            validate_project_specification(&input.specification)?;
        }
        validate_options(&input)?;
        if input.target.starts_with("model-")
            || matches!(
                input.target.as_str(),
                "rust-progenitor" | "rust-progenitor-cli" | "rust-tonic"
            )
        {
            return self.native(input, cancel).await;
        }
        if input.target.starts_with("protobuf-") {
            return self.protoc_generate(input, cancel).await;
        }
        self.jvm(input, cancel).await
    }
    async fn native(
        &self,
        input: ProjectInput,
        cancel: CancellationToken,
    ) -> Result<ProjectArtifact> {
        let bytes = serde_json::to_vec(&input)?;
        ensure!(
            bytes.len() <= SPEC_LIMIT + 65536,
            "Native generation input limit exceeded"
        );
        let mut child = Command::new(&self.worker)
            .arg(super::native::WORKER_ARG)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .context("Cannot start native project worker")?;
        let run = async {
            let mut stdin = child.stdin.take().context("Generation stdin missing")?;
            stdin.write_all(&bytes).await?;
            stdin.shutdown().await?;
            drop(stdin);
            let mut output = Vec::new();
            child
                .stdout
                .take()
                .context("Generation stdout missing")?
                .take((IPC_LIMIT + 1) as u64)
                .read_to_end(&mut output)
                .await?;
            ensure!(
                output.len() <= IPC_LIMIT,
                "Generation reply exceeded output limit"
            );
            ensure!(
                child.wait().await?.success(),
                "Native generation worker failed or exceeded heap limits"
            );
            let value: Value = serde_json::from_slice(&output)
                .map_err(|_| anyhow::anyhow!("Invalid native generation worker reply"))?;
            ensure!(
                value.get("error").is_none(),
                "Native code generation failed; check specification and target options"
            );
            serde_json::from_value(value["artifact"].clone()).map_err(Into::into)
        };
        let result = tokio::select! {biased;_=cancel.cancelled()=>Err(anyhow::anyhow!("Project generation cancelled")),result=tokio::time::timeout(Duration::from_secs(30),run)=>result.unwrap_or_else(|_|Err(anyhow::anyhow!("Project generation timed out")))};
        if result.is_err() {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
        result
    }
    async fn jvm(
        &self,
        mut input: ProjectInput,
        cancel: CancellationToken,
    ) -> Result<ProjectArtifact> {
        let java = self
            .java
            .as_ref()
            .context("Configure an explicit Java17+ executable for this generator")?;
        ensure!(
            !JAR.is_empty(),
            "OpenAPI Generator engine is not bundled; build with the pinned engine asset"
        );
        let target = project_catalog()?
            .into_iter()
            .find(|t| t.id == input.target)
            .unwrap();
        if target.options.contains_key("npmName") && !input.options.contains_key("npmName") {
            input.options.insert(
                "npmName".into(),
                Value::String("moleapi-generated-project".into()),
            );
        }
        let directory = tempfile::tempdir()?;
        let jar = directory.path().join("engine.jar");
        let spec = directory.path().join("openapi.json");
        let config = directory.path().join("config.json");
        let output = directory.path().join("output");
        let template_directory = directory.path().join("templates");
        if let Some(templates) = &input.templates {
            templates.write(&template_directory)?;
        }
        std::fs::write(&jar, JAR)?;
        std::fs::write(&spec, serde_json::to_vec(&input.specification)?)?;
        ensure!(
            !input.options.contains_key("interface"),
            "Interface option belongs to the native Rust generator"
        );
        let mut properties = input.options.clone();
        properties.insert("hideGenerationTimestamp".into(), Value::Bool(true));
        let mut configuration = serde_json::json!({"additionalProperties":properties});
        if let Some(templates) = &input.templates
            && !templates.outputs.is_empty()
        {
            configuration["files"] = serde_json::to_value(&templates.outputs)?;
        }
        std::fs::write(&config, serde_json::to_vec(&configuration)?)?;
        let mut command = Command::new(java);
        command
            .env_clear()
            .arg("-Xmx512m")
            .arg("-Xss1m")
            .arg("-Djava.awt.headless=true")
            .arg("-Djava.net.useSystemProxies=false")
            .arg("-Duser.name=moleapi-codegen")
            .arg(format!("-Duser.home={}", directory.path().display()))
            .arg("-jar")
            .arg(&jar)
            .arg("generate")
            .arg("-g")
            .arg(&input.target)
            .arg("-i")
            .arg(&spec)
            .arg("-o")
            .arg(&output)
            .arg("-c")
            .arg(&config)
            .arg("--global-property")
            .arg("debugOpenAPI=false,debugModels=false,debugOperations=false")
            .current_dir(directory.path())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        if input.templates.is_some() {
            command.arg("--template-dir").arg(&template_directory);
        }
        #[cfg(windows)]
        if let Some(root) = std::env::var_os("SYSTEMROOT") {
            command.env("SYSTEMROOT", root);
        }
        command
            .env("TMPDIR", directory.path())
            .env("TMP", directory.path())
            .env("TEMP", directory.path());
        let mut child = command
            .spawn()
            .context("Cannot start configured Java generator")?;
        let deadline = tokio::time::sleep(Duration::from_secs(60));
        tokio::pin!(deadline);
        let mut interval = tokio::time::interval(Duration::from_millis(200));
        let status = loop {
            tokio::select! {biased;_=cancel.cancelled()=>break None,_=&mut deadline=>break None,
                _=interval.tick()=>{if check_output_budget(&output).is_err(){let _=child.kill().await;let _=child.wait().await;bail!("Generated project exceeded filesystem budget");}},
                status=child.wait()=>break Some(status?)
            }
        };
        match status {
            Some(status) if status.success() => {}
            Some(_) => bail!("OpenAPI Generator failed; check specification and selected options"),
            None => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                bail!("Project generation cancelled or timed out");
            }
        }
        ensure!(!cancel.is_cancelled(), "Project generation cancelled");
        artifact(
            &input,
            &format!("openapi-generator@{JAR_VERSION}"),
            collect(&output)?,
        )
    }
}

impl ProjectRuntime {
    /// Diff computation is isolated from HTTP/native hosts just like native generation.
    pub async fn regenerate(
        &self,
        input: RegenerationInput,
        cancel: CancellationToken,
    ) -> Result<RegenerationResult> {
        self.file_worker(input, cancel).await
    }
    /// Archive parsing shares regeneration's bounded and cancellable process.
    pub async fn import(
        &self,
        input: ProjectImportInput,
        cancel: CancellationToken,
    ) -> Result<ImportedProject> {
        self.file_worker(input, cancel).await
    }
    pub async fn bundle_openapi(
        &self,
        input: OpenapiSourceBundle,
        cancel: CancellationToken,
    ) -> Result<BundledOpenapi> {
        self.file_worker(input, cancel).await
    }
    async fn file_worker<I: Serialize, O: serde::de::DeserializeOwned>(
        &self,
        input: I,
        cancel: CancellationToken,
    ) -> Result<O> {
        let bytes = serde_json::to_vec(&input)?;
        ensure!(
            bytes.len() <= IPC_LIMIT,
            "Regeneration worker input exceeds limit"
        );
        let mut child = Command::new(&self.worker)
            .arg(DIFF_WORKER_ARG)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()?;
        let run = async {
            let mut stdin = child.stdin.take().context("Regeneration stdin missing")?;
            stdin.write_all(&bytes).await?;
            stdin.shutdown().await?;
            drop(stdin);
            let mut output = Vec::new();
            child
                .stdout
                .take()
                .context("Regeneration stdout missing")?
                .take((IPC_LIMIT + 1) as u64)
                .read_to_end(&mut output)
                .await?;
            ensure!(
                output.len() <= IPC_LIMIT,
                "Regeneration output exceeds limit"
            );
            ensure!(
                child.wait().await?.success(),
                "Regeneration worker failed or exceeded limits"
            );
            let value: Value = serde_json::from_slice(&output)?;
            ensure!(
                value.get("error").is_none(),
                "Project import/regeneration failed; check paths, file hashes and limits"
            );
            serde_json::from_value(value["result"].clone()).map_err(Into::into)
        };
        let result = tokio::select! {biased;_=cancel.cancelled()=>Err(anyhow::anyhow!("Regeneration cancelled")),value=tokio::time::timeout(Duration::from_secs(20),run)=>value.unwrap_or_else(|_|Err(anyhow::anyhow!("Regeneration timed out")))};
        if result.is_err() {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
        result
    }
}

impl ProjectRuntime {
    pub fn with_protoc(
        mut self,
        path: Option<PathBuf>,
        plugins: std::collections::BTreeMap<String, PathBuf>,
    ) -> Result<Self> {
        if let Some(path) = &path {
            ensure!(
                path.is_absolute() && path.is_file(),
                "Protoc must be an explicitly configured absolute executable"
            );
        }
        for (language, path) in &plugins {
            ensure!(
                [
                    "cpp", "csharp", "java", "kotlin", "objc", "php", "python", "ruby"
                ]
                .contains(&language.as_str())
                    && path.is_absolute()
                    && path.is_file(),
                "gRPC plugins require a known language and explicit executable path"
            );
        }
        self.protoc = path;
        self.grpc_plugins = plugins;
        Ok(self)
    }
    pub fn from_environment(worker: PathBuf) -> Result<Self> {
        let java = std::env::var_os("MOLEAPI_CODEGEN_JAVA").map(PathBuf::from);
        let protoc = std::env::var_os("MOLEAPI_CODEGEN_PROTOC").map(PathBuf::from);
        let plugins = [
            "cpp", "csharp", "java", "kotlin", "objc", "php", "python", "ruby",
        ]
        .into_iter()
        .filter_map(|language| {
            std::env::var_os(format!(
                "MOLEAPI_CODEGEN_GRPC_{}",
                language.to_ascii_uppercase()
            ))
            .map(|path| (language.into(), PathBuf::from(path)))
        })
        .collect();
        Self::new(worker, java)?.with_protoc(protoc, plugins)
    }
}
