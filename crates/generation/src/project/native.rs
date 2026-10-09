use super::*;
use anyhow::{Context, ensure};
use std::{
    ffi::OsStr,
    io::{Read, Write},
};
pub const WORKER_ARG: &str = "--moleapi-project-worker";
/// Runs before application initialization; the caller installs a native heap cap.
pub fn dispatch_project_worker(limit: impl FnOnce() -> Result<()>) -> Result<bool> {
    if std::env::args_os().nth(1).as_deref() != Some(OsStr::new(WORKER_ARG)) {
        return Ok(false);
    }
    limit()?;
    let mut bytes = Vec::new();
    std::io::stdin()
        .lock()
        .take((SPEC_LIMIT + 65537) as u64)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= SPEC_LIMIT + 65536,
        "Project worker input limit exceeded"
    );
    let input: ProjectInput = serde_json::from_slice(&bytes)
        .map_err(|_| anyhow::anyhow!("Invalid project worker input"))?;
    let result = std::panic::catch_unwind(|| native_project(&input));
    let reply = match result {
        Ok(Ok(artifact)) => serde_json::json!({"artifact":artifact}),
        _ => {
            serde_json::json!({"error":"Rust SDK generation failed; check the specification and target options"})
        }
    };
    let output = serde_json::to_vec(&reply)?;
    ensure!(
        output.len() <= IPC_LIMIT,
        "Project worker reply limit exceeded"
    );
    std::io::stdout().lock().write_all(&output)?;
    Ok(true)
}
fn native_project(input: &ProjectInput) -> Result<ProjectArtifact> {
    validate_project_specification(&input.specification)?;
    validate_options(input)?;
    ensure!(
        input.target == "rust-progenitor",
        "Unsupported native generator"
    );
    ensure!(
        input
            .options
            .keys()
            .all(|k| ["packageName", "packageVersion", "interface"].contains(&k.as_str())),
        "Unsupported native Rust SDK option"
    );
    let package = input
        .options
        .get("packageName")
        .and_then(Value::as_str)
        .unwrap_or("moleapi_sdk");
    ensure!(
        package.len() <= 64
            && package.starts_with(|c: char| c.is_ascii_alphabetic())
            && package
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
        "Invalid Rust package name"
    );
    let version = input
        .options
        .get("packageVersion")
        .and_then(Value::as_str)
        .unwrap_or("0.1.0");
    semver::Version::parse(version).context("Invalid package version")?;
    ensure!(
        input.specification["openapi"]
            .as_str()
            .is_some_and(|v| v.starts_with("3.0.")),
        "Progenitor supports OpenAPI3.0; select an OpenAPI Generator target for3.1"
    );
    let spec: openapiv3::OpenAPI = serde_json::from_value(input.specification.clone())?;
    let mut settings = progenitor::GenerationSettings::default();
    if let Some(style) = input.options.get("interface") {
        settings.with_interface(match style.as_str() {
            Some("builder") => progenitor::InterfaceStyle::Builder,
            Some("positional") => progenitor::InterfaceStyle::Positional,
            _ => anyhow::bail!("Unknown Rust SDK interface style"),
        });
    }
    let mut generator = progenitor::Generator::new(&settings);
    let tokens = generator.generate_tokens(&spec)?;
    let syntax: syn::File = syn::parse2(tokens)?;
    let code = prettyplease::unparse(&syntax);
    let manifest = format!(
        "[package]\nname = {package:?}\nversion = {version:?}\nedition = \"2021\"\n\n[dependencies]\nprogenitor-client = \"=0.15.0\"\nreqwest = {{ version=\"0.13\",default-features=false,features=[\"rustls\",\"json\",\"query\",\"stream\"] }}\nserde = {{version=\"1\",features=[\"derive\"]}}\nserde_json = \"1\"\nfutures = \"0.3\"\nchrono = {{version=\"0.4\",features=[\"serde\"]}}\nuuid = {{version=\"1\",features=[\"serde\",\"v4\"]}}\nbase64 = \"0.22\"\nrand = \"0.9\"\n"
    );
    let files=BTreeMap::from([("Cargo.toml".into(),manifest.into_bytes()),("src/lib.rs".into(),code.into_bytes()),("openapi.json".into(),serde_json::to_vec_pretty(&input.specification)?),("README.md".into(),b"Generated with Progenitor 0.15.0. Configure Client with an explicit service base URL. Generated output is separate from your authored files; regenerate into a new directory and inspect diffs.\n".to_vec())]);
    artifact(
        input,
        "progenitor@0.15.0",
        files
            .into_iter()
            .map(|(path, bytes)| (path, bytes.into()))
            .collect(),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_models_and_methods_are_mature_generator_output() {
        let input = ProjectInput {
            target: "rust-progenitor".into(),
            options: BTreeMap::new(),
            include_secrets: false,
            specification: serde_json::json!({"openapi":"3.0.3","info":{"title":"Fixture","version":"1"},"paths":{"/health":{"get":{"operationId":"health","responses":{"200":{"description":"OK","content":{"application/json":{"schema":{"$ref":"#/components/schemas/Health"}}}}}}}},"components":{"schemas":{"Health":{"type":"object","required":["ok"],"properties":{"ok":{"type":"boolean"}}}}}}),
        };
        let result = native_project(&input).unwrap();
        assert!(
            result
                .files
                .iter()
                .any(|f| f.path == "src/lib.rs" && f.content.contains("pub async fn health"))
        );
        assert!(result.files.iter().any(|f| f.path == "Cargo.toml"));
    }
}
