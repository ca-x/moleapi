//! Quicktype owns model emitters and schema processing in a host-free VM.
use super::*;
use rquickjs::{Context, Function, Promise, Runtime};
use std::time::{Duration, Instant};

pub const ENGINE: &str = "quicktype-core@26.0.0";
const BUNDLE: &str = include_str!("../../../../vendor/model-engine/engine.js");

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelOption {
    pub name: String,
    pub r#type: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<String>>,
}

pub(super) fn catalog() -> Result<Vec<ProjectTarget>> {
    let manifest: Value = serde_json::from_str(include_str!(
        "../../../../vendor/model-engine/manifest.json"
    ))?;
    Ok(serde_json::from_value(manifest["targets"].clone())?)
}

pub(super) fn validate(input: &ProjectInput) -> Result<()> {
    let target = catalog()?
        .into_iter()
        .find(|target| target.id == input.target)
        .ok_or_else(|| anyhow::anyhow!("Unknown model language"))?;
    ensure!(input.options.len() <= 64, "Model option limit exceeded");
    for (key, value) in &input.options {
        if key == "schemaName" {
            let name = value
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Invalid model name"))?;
            ensure!(
                name == "*"
                    || name.len() <= 256
                        && input
                            .specification
                            .pointer("/components/schemas")
                            .and_then(Value::as_object)
                            .is_some_and(|schemas| schemas.contains_key(name)),
                "Selected model not found"
            );
            continue;
        }
        let option = target
            .model_options
            .iter()
            .find(|option| option.name == *key)
            .ok_or_else(|| anyhow::anyhow!("Unknown model renderer option"))?;
        match option.r#type.as_str() {
            "boolean" => ensure!(value.is_boolean(), "Model option requires a boolean"),
            "enum" => ensure!(
                value.as_str().is_some_and(|text| option
                    .values
                    .as_ref()
                    .is_some_and(|values| values.iter().any(|value| value == text))),
                "Unknown model option value"
            ),
            "string" => ensure!(
                value.as_str().is_some_and(|text| text.len() <= 128
                    && text
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || "_.:-".contains(ch))),
                "Invalid model naming option"
            ),
            _ => anyhow::bail!("Unsupported model option type"),
        }
    }
    Ok(())
}

pub(super) fn generate(input: &ProjectInput) -> Result<ProjectArtifact> {
    validate_project_specification(&input.specification)?;
    validate(input)?;
    let runtime = Runtime::new()?;
    runtime.set_memory_limit(128 * 1024 * 1024);
    runtime.set_max_stack_size(1024 * 1024);
    let deadline = Instant::now() + Duration::from_secs(2);
    runtime.set_interrupt_handler(Some(Box::new(move || Instant::now() >= deadline)));
    let context = Context::full(&runtime)?;
    context.with(|ctx| -> Result<()> {
        ctx.eval::<(), _>(BUNDLE)
            .map_err(|_| anyhow::anyhow!("Cannot initialize model engine"))?;
        Ok(())
    })?;
    let deadline = Instant::now() + Duration::from_secs(5);
    runtime.set_interrupt_handler(Some(Box::new(move || Instant::now() >= deadline)));
    let source = serde_json::to_string(input)?;
    let rendered = context.with(|ctx| -> Result<String> {
        let function: Function = ctx.globals().get("moleapiModelGenerate")?;
        let promise: Promise = function.call((source,))?;
        promise
            .finish()
            .map_err(|_| anyhow::anyhow!("Model generation failed or exceeded its budget"))
    })?;
    ensure!(rendered.len() <= IPC_LIMIT, "Model output limit exceeded");
    let files: BTreeMap<String, String> = serde_json::from_str(&rendered)?;
    artifact(
        input,
        ENGINE,
        files
            .into_iter()
            .map(|(path, text)| (path, text.into_bytes().into()))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input(target: &str) -> ProjectInput {
        ProjectInput {
            target: target.into(),
            options: BTreeMap::new(),
            include_secrets: false,
            specification: serde_json::json!({"openapi":"3.0.3","info":{"title":"Models","version":"1"},"paths":{},"components":{"schemas":{"Pet":{"type":"object","required":["id","status"],"properties":{"id":{"type":"integer"},"status":{"type":"string","enum":["ready","done"]},"note":{"type":"string","nullable":true},"friend":{"$ref":"#/components/schemas/Pet"}}}}}}),
        }
    }
    #[test]
    fn every_reference_model_language_generates_recursive_nullable_enum_types() {
        let targets = catalog().unwrap();
        assert_eq!(targets.len(), 18);
        for target in targets {
            let artifact = generate(&input(&target.id))
                .unwrap_or_else(|error| panic!("{}: {error}", target.id));
            assert!(
                artifact
                    .files
                    .iter()
                    .any(|file| file.path != "moleapi-generation.json"
                        && file.content.contains("Pet")),
                "{}",
                target.id
            );
        }
    }
    #[test]
    fn selections_options_and_external_sources_are_bounded() {
        let mut input = input("model-typescript");
        input.options.insert("schemaName".into(), "missing".into());
        assert!(validate(&input).is_err());
        input.options.clear();
        input
            .options
            .insert("runtime-typecheck".into(), "true".into());
        assert!(validate(&input).is_err());
        input
            .options
            .insert("runtime-typecheck".into(), true.into());
        assert!(generate(&input).is_ok());
        input.specification["components"]["schemas"]["Pet"]["properties"]["friend"]["$ref"] =
            "https://private.invalid/model.json".into();
        assert!(generate(&input).is_err());
    }
}
