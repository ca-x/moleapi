//! Official protoc and explicitly selected gRPC plugins own non-Rust emitters.
use super::*;
use anyhow::{Context, bail, ensure};
use prost::Message;
use std::{process::Stdio, time::Duration};
use tokio::{io::AsyncReadExt, process::Command};
use tokio_util::sync::CancellationToken;
impl ProjectRuntime {
    pub(crate) async fn protoc_generate(
        &self,
        input: ProjectInput,
        cancel: CancellationToken,
    ) -> Result<ProjectArtifact> {
        let compiler = self
            .protoc
            .as_ref()
            .context("Configure an explicit protoc executable for this target")?;
        let language = input
            .target
            .strip_prefix("protobuf-")
            .context("Invalid protobuf target")?;
        ensure!(
            [
                "cpp", "csharp", "java", "kotlin", "objc", "php", "python", "ruby"
            ]
            .contains(&language),
            "Unsupported protoc language"
        );
        ensure!(
            serde_json::to_vec(&input.specification)?.len() <= SPEC_LIMIT,
            "Protobuf generation source limit exceeded"
        );
        let specification = moleapi_core::Specification {
            id: "generation".into(),
            name: "Source".into(),
            kind: "protobuf".into(),
            dialect: "proto3".into(),
            source: serde_json::to_string(&input.specification)?,
        };
        let pool = moleapi_core::protobuf_pool(&specification)?;
        let bytes = pool.encode_to_vec();
        let descriptor = prost_types::FileDescriptorSet::decode(bytes.as_slice())?;
        let source: moleapi_core::ProtobufSource =
            serde_json::from_value(input.specification.clone())?;
        let entries = match source {
            moleapi_core::ProtobufSource::Proto { entry_files, .. } => entry_files,
            moleapi_core::ProtobufSource::Descriptor { .. } => descriptor
                .file
                .iter()
                .filter_map(|f| f.name.clone())
                .filter(|name| !name.starts_with("google/protobuf/"))
                .collect(),
        };
        ensure!(
            !entries.is_empty() && entries.len() <= 32,
            "Protoc input entry count exceeded"
        );
        for path in &entries {
            moleapi_core::validate_proto_path(path)?;
            ensure!(
                !path.starts_with(['-', '@']),
                "Protoc input names cannot be command flags or argument files"
            );
        }
        let directory = tempfile::tempdir()?;
        let output = directory.path().join("generated");
        std::fs::create_dir(&output)?;
        let fds = directory.path().join("source.bin");
        std::fs::write(&fds, &bytes)?;
        let mut command = Command::new(compiler);
        command
            .env_clear()
            .current_dir(directory.path())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .arg(format!("--descriptor_set_in={}", fds.display()))
            .arg(format!("--{language}_out={}", output.display()));
        if language == "kotlin" {
            command.arg(format!("--java_out={}", output.display()));
        }
        if let Some(plugin) = self.grpc_plugins.get(language) {
            if plugin.canonicalize()? != compiler.canonicalize()? {
                command.arg(format!(
                    "--plugin=protoc-gen-grpc_{language}={}",
                    plugin.display()
                ));
            }
            command.arg(format!("--grpc_{language}_out={}", output.display()));
        }
        command.args(&entries);
        #[cfg(windows)]
        if let Some(root) = std::env::var_os("SYSTEMROOT") {
            command.env("SYSTEMROOT", root);
        }
        let mut wrapped = process_wrap::tokio::CommandWrap::from(command);
        wrapped.wrap(process_wrap::tokio::KillOnDrop);
        #[cfg(unix)]
        wrapped.wrap(process_wrap::tokio::ProcessGroup::leader());
        #[cfg(windows)]
        wrapped.wrap(process_wrap::tokio::JobObject);
        let mut child = wrapped.spawn()?;
        let deadline = tokio::time::sleep(Duration::from_secs(30));
        tokio::pin!(deadline);
        let mut tick = tokio::time::interval(Duration::from_millis(100));
        let status = loop {
            tokio::select! {biased;_=cancel.cancelled()=>break None,_=&mut deadline=>break None,_=tick.tick()=>{if artifact::check_output_budget(&output).is_err(){let _=std::pin::Pin::from(child.kill()).await;let _=child.wait().await;bail!("Protoc output exceeded filesystem budget");}},value=child.wait()=>break Some(value?)}
        };
        match status {
            Some(status) if status.success() => {}
            Some(_) => {
                bail!("Protoc/plugin generation failed; check source and runtime compatibility")
            }
            None => {
                let _ = std::pin::Pin::from(child.kill()).await;
                let _ = child.wait().await;
                bail!("Protoc generation cancelled or timed out");
            }
        }
        let mut files = artifact::collect(&output)?;
        files.insert(
            "protobuf-source.json".into(),
            serde_json::to_vec_pretty(&input.specification)?.into(),
        );
        files.insert("descriptor-set.bin".into(), bytes.into());
        let grpc = self.grpc_plugins.contains_key(language);
        files.insert("README.md".into(),format!("Generated with the explicitly configured official protoc for {language}. gRPC plugin output: {grpc}. Install the matching language protobuf{} runtime before compiling. This artifact does not execute target code, download plugins or overwrite authored files. Original source and descriptor are included.\n",if grpc {" and gRPC"}else{""}).into_bytes().into());
        // Query only this explicitly configured executable; no ambient tool discovery.
        let mut version = Command::new(compiler)
            .arg("--version")
            .env_clear()
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()?;
        let mut data = Vec::new();
        let read = async {
            version
                .stdout
                .take()
                .context("Protoc version stream missing")?
                .take(1025)
                .read_to_end(&mut data)
                .await?;
            ensure!(
                data.len() <= 1024 && version.wait().await?.success(),
                "Protoc version query failed"
            );
            Ok::<_, anyhow::Error>(())
        };
        if !matches!(
            tokio::time::timeout(Duration::from_secs(3), read).await,
            Ok(Ok(()))
        ) {
            let _ = version.kill().await;
            let _ = version.wait().await;
            bail!("Protoc version query failed or timed out");
        }
        let engine = String::from_utf8(data)?.trim().to_owned();
        ensure!(
            !engine.is_empty() && !engine.chars().any(char::is_control),
            "Invalid protoc version reply"
        );
        artifact(&input, &format!("{engine};grpc-plugin={grpc}"), files)
    }
}
