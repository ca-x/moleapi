//! Native protobuf/gRPC generation: protox compiles imports, Tonic/Prost own emitters.
use super::*;
use anyhow::ensure;
use base64::{Engine, engine::general_purpose::STANDARD};
use prost::Message;
use sha2::{Digest, Sha256};
pub(crate) fn native_protobuf(input: &ProjectInput) -> Result<ProjectArtifact> {
    validate_options(input)?;
    ensure!(
        input
            .options
            .keys()
            .all(|k| ["packageName", "packageVersion"].contains(&k.as_str())),
        "Unsupported protobuf project option"
    );
    let package = input
        .options
        .get("packageName")
        .and_then(Value::as_str)
        .unwrap_or("moleapi_grpc");
    ensure!(
        package.len() <= 64
            && package.starts_with(|c: char| c.is_ascii_alphabetic())
            && package
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
        "Invalid protobuf package name"
    );
    let version = input
        .options
        .get("packageVersion")
        .and_then(Value::as_str)
        .unwrap_or("0.1.0");
    semver::Version::parse(version)?;
    ensure!(
        serde_json::to_vec(&input.specification)?.len() <= SPEC_LIMIT,
        "Protobuf generation source exceeds limit"
    );
    let spec = moleapi_core::Specification {
        id: "generation".into(),
        name: "Protobuf".into(),
        kind: "protobuf".into(),
        dialect: "proto3".into(),
        source: serde_json::to_string(&input.specification)?,
    };
    let pool = moleapi_core::protobuf_pool(&spec)?;
    let descriptor = pool.encode_to_vec();
    let mut fds = prost_types::FileDescriptorSet::decode(descriptor.as_slice())?;
    for file in &mut fds.file {
        file.source_code_info = None;
    }
    let temporary = tempfile::tempdir()?;
    let output = temporary.path().join("generated");
    std::fs::create_dir(&output)?;
    // compile_fds never invokes protoc or performs filesystem import resolution.
    tonic_prost_build::configure()
        .build_client(true)
        .build_server(true)
        .out_dir(&output)
        .include_file("mod.rs")
        .emit_rerun_if_changed(false)
        .compile_fds(fds)?;
    let mut files = artifact::collect(&output)?
        .into_iter()
        .map(|(path, file)| (format!("src/generated/{path}"), file))
        .collect::<BTreeMap<_, _>>();
    files.insert(
        "src/lib.rs".into(),
        b"pub mod generated;\npub use generated::*;\n"
            .to_vec()
            .into(),
    );
    let manifest = format!(
        "[package]\nname={package:?}\nversion={version:?}\nedition=\"2021\"\n\n[dependencies]\nprost=\"0.14\"\nprost-types=\"0.14\"\ntonic={{version=\"0.14\",features=[\"transport\"]}}\ntonic-prost=\"0.14\"\n"
    );
    files.insert("Cargo.toml".into(), manifest.into_bytes().into());
    files.insert(
        "protobuf-source.json".into(),
        serde_json::to_vec_pretty(&input.specification)?.into(),
    );
    files.insert("descriptor-set.bin".into(), descriptor.clone().into());
    files.insert("README.md".into(),format!("Generated using tonic-prost-build0.14.6 and protox. Both client and server interfaces are available under generated modules. Implement server traits in authored code, and connect clients with an explicit endpoint. No compiler/plugin download or unbundled import occurs. Descriptor SHA256: {:x}\n",Sha256::digest(&descriptor)).into_bytes().into());
    // Keep a text-safe descriptor copy for repeatable generation on other tooling.
    files.insert(
        "descriptor-set.base64.txt".into(),
        STANDARD.encode(descriptor).into_bytes().into(),
    );
    artifact(input, "tonic-prost-build@0.14.6", files)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> ProjectInput {
        ProjectInput {
            target: "rust-tonic".into(),
            options: BTreeMap::new(),
            include_secrets: false,
            templates: None,
            specification: serde_json::json!({"kind":"proto","files":[{"path":"types.proto","content":"syntax=\"proto3\"; package fixture; message Request {string text=1;}"},{"path":"service.proto","content":"syntax=\"proto3\"; package fixture; import \"types.proto\"; service Echo {rpc Call(Request) returns(Request);}"}],"entry_files":["service.proto"]}),
        }
    }
    #[test]
    fn native_compiler_emits_client_server_and_repeatable_descriptor_from_bundled_imports() {
        let mut original = input();
        original.specification = serde_json::from_str(include_str!(
            "../../../../tools/projects/fixtures/protobuf-imports.json"
        ))
        .unwrap();
        let artifact = native_protobuf(&original).unwrap();
        assert!(artifact.files.iter().any(|f| f.path.ends_with("fixture.rs")
            && f.content.contains("EchoServiceClient")
            && f.content.contains("EchoServiceServer")));
        let descriptor = artifact
            .files
            .iter()
            .find(|f| f.path == "descriptor-set.base64.txt")
            .unwrap();
        let mut second = original;
        second.specification =
            serde_json::json!({"kind":"descriptor","descriptor_set_base64":descriptor.content});
        let repeated = native_protobuf(&second).unwrap();
        assert!(
            repeated
                .files
                .iter()
                .any(|f| f.path.ends_with("fixture.rs") && f.content.contains("EchoServiceClient"))
        );
        let source = artifact
            .files
            .iter()
            .find(|f| f.path.ends_with("fixture.rs"))
            .unwrap();
        let regenerated = repeated
            .files
            .iter()
            .find(|f| f.path == source.path)
            .unwrap();
        assert_eq!(source.content, regenerated.content);
        for method in [
            "async fn call",
            "async fn watch",
            "async fn collect",
            "async fn chat",
        ] {
            assert!(
                source.content.contains(method),
                "Missing generated Rust {method}"
            );
        }
    }
    #[test]
    fn unbundled_and_traversal_imports_do_not_fall_back_to_host_filesystem() {
        let mut input = input();
        input.specification["files"][1]["content"] =
            Value::String("syntax=\"proto3\"; import \"/etc/passwd\";".into());
        assert!(native_protobuf(&input).is_err());
        input.specification["files"][0]["path"] = Value::String("../outside.proto".into());
        assert!(native_protobuf(&input).is_err());
    }
}
