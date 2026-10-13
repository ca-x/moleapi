//! Explicit in-memory source sets, resolved by schematools without its resource loaders.
use super::*;
use anyhow::Context;
use schematools::{process::dereference::Dereferencer, schema::Schema, storage::SchemaStorage};
use url::Url;

pub const SOURCE_BUNDLE_FORMAT: &str = "moleapi-openapi-source-v1";
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenapiSourceFile {
    pub path: String,
    pub content: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenapiSourceBundle {
    pub format: String,
    pub entry_file: String,
    pub files: Vec<OpenapiSourceFile>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BundledOpenapi {
    pub specification: Value,
    /// Semantic source documents also participate in default credential screening.
    pub documents: Vec<Value>,
}
pub fn parse_openapi_source_bundle(source: &str) -> Result<Option<OpenapiSourceBundle>> {
    ensure!(source.len() <= SPEC_LIMIT, "OpenAPI source exceeds 1 MiB");
    let Ok(value) = serde_json::from_str::<Value>(source) else {
        return Ok(None);
    };
    if value["format"] != SOURCE_BUNDLE_FORMAT {
        return Ok(None);
    }
    Ok(Some(serde_json::from_value(value)?))
}
fn uri(path: &str) -> Result<Url> {
    artifact::safe_path(path)?;
    let mut url = Url::parse("moleapi://source-bundle/")?;
    url.path_segments_mut()
        .map_err(|_| anyhow::anyhow!("Invalid source bundle base"))?
        .extend(path.split('/'));
    Ok(url)
}
fn check_document(value: &Value) -> Result<()> {
    ensure!(
        serde_json::to_vec(value)?.len() <= SPEC_LIMIT,
        "Parsed source size limit exceeded"
    );
    let mut pending = vec![(value, 0usize)];
    let mut nodes = 0;
    while let Some((value, depth)) = pending.pop() {
        nodes += 1;
        ensure!(
            nodes <= 50000 && depth <= 64,
            "Source bundle complexity limit exceeded"
        );
        match value {
            Value::Object(object) => {
                pending.extend(object.values().map(|value| (value, depth + 1)))
            }
            Value::Array(array) => pending.extend(array.iter().map(|value| (value, depth + 1))),
            _ => {}
        }
    }
    Ok(())
}
fn discriminator_mappings(
    value: &mut Value,
    base: &Url,
    names: &std::collections::BTreeSet<String>,
    aliases: &BTreeMap<Url, String>,
    dictionary: bool,
) -> Result<()> {
    match value {
        Value::Object(object) => {
            if !dictionary
                && let Some(mapping) = object
                    .get_mut("discriminator")
                    .and_then(|value| value.get_mut("mapping"))
                    .and_then(Value::as_object_mut)
            {
                for value in mapping.values_mut() {
                    let reference = value
                        .as_str()
                        .context("Invalid source discriminator mapping")?;
                    if names.contains(reference) {
                        continue;
                    }
                    let url = schematools::storage::ref_to_url(base, reference)
                        .context("Invalid discriminator URI")?;
                    let name = aliases.get(&url).context(
                        "Source discriminator target must have an explicit schema component alias",
                    )?;
                    *value = Value::String(name.clone());
                }
            }
            for (key, value) in object {
                if !super::reference_policy::literal_member(key, value, dictionary) {
                    discriminator_mappings(
                        value,
                        base,
                        names,
                        aliases,
                        !dictionary && super::reference_policy::dictionary_member(key),
                    )?;
                }
            }
        }
        Value::Array(array) => {
            for value in array {
                discriminator_mappings(value, base, names, aliases, false)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn references(
    value: &Value,
    base: &Url,
    documents: &BTreeMap<Url, Value>,
    dictionary: bool,
) -> Result<()> {
    match value {
        Value::Object(object) => {
            ensure!(
                dictionary
                    || !object.keys().any(|key| [
                        "$id",
                        "$anchor",
                        "$dynamicAnchor",
                        "$dynamicRef",
                        "$recursiveRef"
                    ]
                    .contains(&key.as_str())),
                "JSON Schema resource IDs/anchors/dynamic references need a supported source-bundle resolver"
            );
            if let Some(reference) = object.get("$ref").filter(|_| !dictionary) {
                ensure!(
                    object
                        .keys()
                        .all(|key| ["$ref", "summary", "description"].contains(&key.as_str())),
                    "Source-bundle reference siblings require verified schema composition support"
                );
                let reference = reference
                    .as_str()
                    .context("Source reference must be a string")?;
                let mut target = schematools::storage::ref_to_url(base, reference)
                    .context("Invalid source reference URI")?;
                let fragment = target.fragment().map(str::to_owned);
                target.set_fragment(None);
                let document = documents
                    .get(&target)
                    .context("Reference is outside the supplied source bundle")?;
                if let Some(pointer) = fragment {
                    let pointer = schematools::storage::decode_pointer_fragment(&pointer)
                        .context("Invalid URI-fragment JSON Pointer or unsupported named anchor")?;
                    ensure!(
                        document.pointer(&pointer).is_some(),
                        "Source reference pointer not found"
                    );
                }
            }
            if let Some(reference) = object.get("operationRef").filter(|_| !dictionary) {
                let reference = reference.as_str().context("Invalid operation reference")?;
                let mut uri = schematools::storage::ref_to_url(base, reference)
                    .context("Invalid operation reference URI")?;
                let pointer =
                    schematools::storage::decode_pointer_fragment(uri.fragment().unwrap_or(""))
                        .context("Invalid operation reference fragment")?;
                uri.set_fragment(None);
                let target = documents
                    .get(&uri)
                    .and_then(|document| document.pointer(&pointer))
                    .context("Operation reference target not found in supplied sources")?;
                ensure!(
                    target.get("responses").is_some_and(Value::is_object),
                    "Operation reference must target an operation object"
                );
            }
            for (key, value) in object {
                if !super::reference_policy::literal_member(key, value, dictionary) {
                    references(
                        value,
                        base,
                        documents,
                        !dictionary && super::reference_policy::dictionary_member(key),
                    )?;
                }
            }
        }
        Value::Array(array) => {
            for value in array {
                references(value, base, documents, false)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn absolutize_operations(value: &mut Value, base: &Url, dictionary: bool) -> Result<()> {
    match value {
        Value::Object(object) => {
            if !dictionary && let Some(reference) = object.get_mut("operationRef") {
                *reference = schematools::storage::ref_to_url(
                    base,
                    reference.as_str().context("Invalid operation reference")?,
                )
                .context("Invalid operation URI")?
                .to_string()
                .into();
            }
            for (key, value) in object {
                if !super::reference_policy::literal_member(key, value, dictionary) {
                    absolutize_operations(
                        value,
                        base,
                        !dictionary && super::reference_policy::dictionary_member(key),
                    )?;
                }
            }
        }
        Value::Array(array) => {
            for value in array {
                absolutize_operations(value, base, false)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn relocate_operations(
    value: &mut Value,
    locations: &std::collections::HashMap<String, String>,
    dictionary: bool,
) -> Result<()> {
    match value {
        Value::Object(object) => {
            if !dictionary && let Some(reference) = object.get_mut("operationRef") {
                let uri = reference
                    .as_str()
                    .context("Invalid bundled operation reference")?;
                let (source, location) = locations
                    .iter()
                    .filter(|(source, _)| {
                        uri.strip_prefix(source.as_str()).is_some_and(|tail| {
                            tail.is_empty() || tail.starts_with('/') || tail.starts_with('#')
                        })
                    })
                    .max_by_key(|(source, _)| source.len())
                    .context("Operation target is not included in the assembled document")?;
                let tail = uri
                    .strip_prefix(source)
                    .unwrap()
                    .strip_prefix('#')
                    .unwrap_or_else(|| uri.strip_prefix(source).unwrap());
                let tail = schematools::storage::decode_pointer_fragment(tail)
                    .context("Invalid assembled operation reference fragment")?;
                *reference = format!(
                    "#{}",
                    schematools::storage::encode_pointer_fragment(&format!("{location}{tail}"))
                )
                .into();
            }
            for (key, value) in object {
                if !super::reference_policy::literal_member(key, value, dictionary) {
                    relocate_operations(
                        value,
                        locations,
                        !dictionary && super::reference_policy::dictionary_member(key),
                    )?;
                }
            }
        }
        Value::Array(array) => {
            for value in array {
                relocate_operations(value, locations, false)?;
            }
        }
        _ => {}
    }
    Ok(())
}
pub fn bundle_openapi(input: OpenapiSourceBundle) -> Result<BundledOpenapi> {
    ensure!(
        input.format == SOURCE_BUNDLE_FORMAT && !input.files.is_empty() && input.files.len() <= 64,
        "Invalid OpenAPI source bundle/count"
    );
    let entry = uri(&input.entry_file)?;
    let mut documents = BTreeMap::new();
    let mut total = 0usize;
    for file in input.files {
        total = total
            .checked_add(file.content.len())
            .context("Source bundle size overflow")?;
        ensure!(total <= SPEC_LIMIT, "OpenAPI sources exceed 1 MiB");
        let url = uri(&file.path)?;
        let value: Value =
            serde_yaml_ng::from_str(&file.content).context("Invalid source JSON/YAML")?;
        check_document(&value)?;
        ensure!(
            documents.insert(url, value).is_none(),
            "Duplicate OpenAPI source path"
        );
    }
    ensure!(
        serde_json::to_vec(&documents.values().collect::<Vec<_>>())?.len() <= SPEC_LIMIT,
        "Parsed source bundle exceeds 1 MiB"
    );
    let original_documents = documents.values().cloned().collect::<Vec<_>>();
    if documents
        .get(&entry)
        .and_then(|root| root["openapi"].as_str())
        .is_some_and(|version| version.starts_with("3.1."))
    {
        documents = super::schema_resources::normalize(&documents, &entry)?;
    }
    for (url, document) in &documents {
        references(
            document,
            url,
            &documents,
            super::reference_policy::root_dictionary(document),
        )?;
    }
    let root = documents
        .get(&entry)
        .context("OpenAPI entry file is not in the source bundle")?;
    ensure!(
        root.is_object()
            && root["openapi"]
                .as_str()
                .is_some_and(|version| version.starts_with("3.0.") || version.starts_with("3.1.")),
        "Source entry requires OpenAPI3.0/3.1"
    );
    let names: std::collections::BTreeSet<String> = root
        .pointer("/components/schemas")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|object| object.keys().cloned())
        .collect();
    let mut aliases = BTreeMap::new();
    for (name, value) in root
        .pointer("/components/schemas")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        if let Some(reference) = value.get("$ref").and_then(Value::as_str)
            && let Some(url) = schematools::storage::ref_to_url(&entry, reference)
        {
            aliases.insert(url, name.clone());
        }
        let mut pointer = entry.clone();
        let encoded = name.replace('~', "~0").replace('/', "~1");
        pointer.set_fragment(Some(&schematools::storage::encode_pointer_fragment(
            &format!("/components/schemas/{encoded}"),
        )));
        aliases.insert(pointer, name.clone());
    }
    for (url, value) in &mut documents {
        let dictionary = super::reference_policy::root_dictionary(value);
        discriminator_mappings(value, url, &names, &aliases, dictionary)?;
        absolutize_operations(value, url, dictionary)?;
    }
    let root = documents.get(&entry).unwrap();
    // schematools names repeated references at their first use. Resolve model
    // declarations first so SDK emitters retain references under components.
    let mut body = root.as_object().unwrap().clone();
    let mut ordered = serde_json::Map::new();
    if let Some(components) = body.remove("components") {
        ordered.insert("components".into(), components);
    }
    ordered.extend(body);
    let mut literals =
        super::reference_policy::LiteralValues::new(original_documents.clone().into_iter());
    let mut ordered = Value::Object(ordered);
    literals.mask(&mut ordered, false);
    let mut schema = Schema::from_json_at(ordered, entry.clone());
    let mut masked_documents = documents.clone();
    for document in masked_documents.values_mut() {
        let dictionary = super::reference_policy::root_dictionary(document);
        literals.mask(document, dictionary);
    }
    let storage = SchemaStorage::from_documents(
        masked_documents
            .iter()
            .map(|(url, document)| Schema::from_json_at(document.clone(), url.clone()))
            .collect(),
    );
    let mut locations = Dereferencer::options()
        .with_skip_root_internal_references(true)
        .process_with_references(&mut schema, &storage);
    locations.insert(entry.to_string(), String::new());
    let mut specification = schema.get_body().clone();
    relocate_operations(&mut specification, &locations, false)?;
    literals.restore(&mut specification);
    check_document(&specification)?;
    validate_project_specification(&specification)?;
    Ok(BundledOpenapi {
        specification,
        documents: original_documents,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source(path: &str, content: Value) -> OpenapiSourceFile {
        OpenapiSourceFile {
            path: path.into(),
            content: content.to_string(),
        }
    }
    fn fixture() -> OpenapiSourceBundle {
        OpenapiSourceBundle {
            format: SOURCE_BUNDLE_FORMAT.into(),
            entry_file: "api/root.json".into(),
            files: vec![
                source(
                    "api/root.json",
                    serde_json::json!({"openapi":"3.0.3","info":{"title":"Bundle","version":"1"},"paths":{"/tree":{"$ref":"paths/tree.yaml"}},"components":{"schemas":{"Tree":{"$ref":"../models/tree.json"}}}}),
                ),
                source(
                    "api/paths/tree.yaml",
                    serde_json::json!({"get":{"operationId":"tree","responses":{"200":{"description":"Tree","content":{"application/json":{"schema":{"$ref":"../../models/tree.json"}}}}}}}),
                ),
                source(
                    "models/tree.json",
                    serde_json::json!({"type":"object","properties":{"name":{"type":"string"},"child":{"$ref":"tree.json","nullable":true}}}),
                ),
            ],
        }
    }
    #[test]
    fn bundles_relative_path_items_and_recursive_models_using_mature_storage() {
        let mut input = fixture();
        // Optional3.0 reference annotations are expressed at the schema use site.
        let mut tree: Value = serde_json::from_str(&input.files[2].content).unwrap();
        tree["properties"]["child"]
            .as_object_mut()
            .unwrap()
            .remove("nullable");
        input.files[2].content = tree.to_string();
        let mut path: Value = serde_json::from_str(&input.files[1].content).unwrap();
        let literal =
            serde_json::json!({"$ref":"file:///not-a-schema","nested":{"$id":"literal-id"}});
        path["get"]["responses"]["200"]["content"]["application/json"]["example"] = literal.clone();
        input.files[1].content = path.to_string();
        let original = serde_json::to_value(&input).unwrap();
        let result = bundle_openapi(input.clone()).unwrap();
        assert_eq!(
            result.specification["paths"]["/tree"]["get"]["operationId"],
            "tree"
        );
        assert_eq!(
            result.specification["paths"]["/tree"]["get"]["responses"]["200"]["content"]["application/json"]
                ["example"],
            literal
        );
        assert!(result.specification.to_string().contains("#/"));
        assert!(!result.specification.to_string().contains("moleapi://"));
        assert_eq!(original, serde_json::to_value(input).unwrap());
        let spec: openapiv3::OpenAPI = serde_json::from_value(result.specification).unwrap();
        let mut generator = progenitor::Generator::default();
        generator.generate_tokens(&spec).unwrap();
    }
    #[test]
    fn refuses_unknown_host_files_remote_refs_missing_pointers_and_unhandled_siblings() {
        let input = fixture();
        assert!(bundle_openapi(input.clone()).is_err());
        for reference in [
            "https://example.com/schema",
            "file:///etc/passwd",
            "../../outside.json",
            "../models/tree.json#/missing",
        ] {
            let mut input = input.clone();
            let mut value: Value = serde_json::from_str(&input.files[0].content).unwrap();
            value["components"]["schemas"]["Tree"]["$ref"] = reference.into();
            input.files[0].content = value.to_string();
            assert!(bundle_openapi(input).is_err());
        }
    }
    #[test]
    fn discriminator_file_mappings_use_declared_component_names_and_never_remain_external() {
        let mut input = fixture();
        let mut tree: Value = serde_json::from_str(&input.files[2].content).unwrap();
        tree["properties"]["child"]
            .as_object_mut()
            .unwrap()
            .remove("nullable");
        tree["discriminator"] =
            serde_json::json!({"propertyName":"kind","mapping":{"tree":"tree.json"}});
        input.files[2].content = tree.to_string();
        let result = bundle_openapi(input.clone()).unwrap();
        assert_eq!(
            result.specification["components"]["schemas"]["Tree"]["discriminator"]["mapping"]["tree"],
            "Tree"
        );
        tree["discriminator"]["mapping"]["tree"] = "https://example.com/unprovided".into();
        input.files[2].content = tree.to_string();
        assert!(bundle_openapi(input).is_err());
        let mut plain = result.specification;
        plain["components"]["schemas"]["Tree"]["discriminator"]["mapping"]["tree"] =
            "file:///unprovided".into();
        assert!(validate_project_specification(&plain).is_err());
    }
    #[test]
    fn source_local_operation_links_relocate_to_the_assembled_path() {
        let root = serde_json::json!({"openapi":"3.0.3","info":{"title":"Links","version":"1"},"paths":{"/x":{"$ref":"path.json"}}});
        let path = serde_json::json!({"get":{"responses":{"200":{"description":"OK","links":{"self":{"operationRef":"#/get"}}}}}});
        let input = OpenapiSourceBundle {
            format: SOURCE_BUNDLE_FORMAT.into(),
            entry_file: "root.json".into(),
            files: vec![source("root.json", root), source("path.json", path)],
        };
        let result = bundle_openapi(input).unwrap();
        assert_eq!(
            result.specification["paths"]["/x"]["get"]["responses"]["200"]["links"]["self"]["operationRef"],
            "#/paths/~1x/get"
        );
        assert!(result.specification.pointer("/paths/~1x/get").is_some());
    }
    #[test]
    fn repeated_model_refs_keep_each_sites_summary_and_description() {
        let root = serde_json::json!({"openapi":"3.1.0","info":{"title":"Annotations","version":"1"},"paths":{},"components":{"schemas":{"First":{"$ref":"model.json","description":"first site"},"Second":{"$ref":"model.json","description":"second site","summary":"second summary"}}}});
        let input = OpenapiSourceBundle {
            format: SOURCE_BUNDLE_FORMAT.into(),
            entry_file: "root.json".into(),
            files: vec![
                source("root.json", root),
                source("model.json", serde_json::json!({"type":"object"})),
            ],
        };
        let result = bundle_openapi(input).unwrap();
        assert_eq!(
            result.specification["components"]["schemas"]["Second"]["description"],
            "second site"
        );
        assert_eq!(
            result.specification["components"]["schemas"]["Second"]["summary"],
            "second summary"
        );
    }

    #[test]
    fn encoded_source_pointers_resolve_unicode_percent_slash_and_equivalent_aliases() {
        let reference = "types.json#%2Fdefinitions%2FA%20caf%C3%A9~1%25";
        let root = serde_json::json!({"openapi":"3.0.3","info":{"title":"Encoded","version":"1"},"paths":{"/thing":{"get":{"operationId":"thing","responses":{"200":{"description":"OK","content":{"application/json":{"schema":{"$ref":"types.json#/definitions/A%20caf%c3%a9~1%25"}}}}}}}},"components":{"schemas":{"Thing":{"$ref":reference}}}});
        let types = serde_json::json!({"definitions":{"A café/%":{"type":"object","properties":{"literal%2F":{"type":"string"},"tilde~value":{"type":"integer"}}}}});
        let input = OpenapiSourceBundle {
            format: SOURCE_BUNDLE_FORMAT.into(),
            entry_file: "root.json".into(),
            files: vec![source("root.json", root), source("types.json", types)],
        };
        let original = serde_json::to_value(&input).unwrap();
        let result = bundle_openapi(input.clone()).unwrap();
        assert_eq!(
            result.specification["components"]["schemas"]["Thing"]["properties"]["literal%2F"]["type"],
            "string"
        );
        assert_eq!(
            result.specification["paths"]["/thing"]["get"]["responses"]["200"]["content"]["application/json"]
                ["schema"]["$ref"],
            "#/components/schemas/Thing"
        );
        assert_eq!(original, serde_json::to_value(input).unwrap());
        let spec: openapiv3::OpenAPI = serde_json::from_value(result.specification).unwrap();
        progenitor::Generator::default()
            .generate_tokens(&spec)
            .unwrap();
    }

    #[test]
    fn encoded_operation_links_relocate_with_uri_fragment_semantics() {
        let root = serde_json::json!({"openapi":"3.0.3","info":{"title":"Links","version":"1"},"paths":{"/café /%":{"$ref":"path.json#%2Froute%20~1%25"}}});
        let path = serde_json::json!({"route /%":{"get":{"responses":{"200":{"description":"OK","links":{"self":{"operationRef":"#%2Froute%20~1%25/get"}}}}}}});
        let result = bundle_openapi(OpenapiSourceBundle {
            format: SOURCE_BUNDLE_FORMAT.into(),
            entry_file: "root.json".into(),
            files: vec![source("root.json", root), source("path.json", path)],
        })
        .unwrap();
        let reference = result.specification["paths"]["/café /%"]["get"]["responses"]["200"]["links"]["self"]["operationRef"].as_str().unwrap();
        let pointer =
            schematools::storage::decode_pointer_fragment(reference.strip_prefix('#').unwrap())
                .unwrap();
        assert_eq!(pointer, "/paths/~1café ~1%/get");
        assert!(result.specification.pointer(&pointer).is_some());
    }

    #[test]
    fn source_pointer_decoding_is_single_pass_and_rejects_invalid_uri_or_pointer_escapes() {
        let root = serde_json::json!({"openapi":"3.0.3","info":{"title":"Percent","version":"1"},"paths":{},"components":{"schemas":{"Thing":{"$ref":"types.json#/literal%252F"}}}});
        let mut input = OpenapiSourceBundle {
            format: SOURCE_BUNDLE_FORMAT.into(),
            entry_file: "root.json".into(),
            files: vec![
                source("root.json", root.clone()),
                source(
                    "types.json",
                    serde_json::json!({"literal%2F":{"type":"string"},"literal/":{"type":"integer"}}),
                ),
            ],
        };
        assert_eq!(
            bundle_openapi(input.clone()).unwrap().specification["components"]["schemas"]["Thing"]
                ["type"],
            "string"
        );
        for fragment in [
            "/literal%",
            "/literal%GG",
            "/literal%FF",
            "/literal~2",
            "/literal%7E2",
            "/missing%252F",
        ] {
            let mut changed = root.clone();
            changed["components"]["schemas"]["Thing"]["$ref"] =
                format!("types.json#{fragment}").into();
            input.files[0].content = changed.to_string();
            assert!(bundle_openapi(input.clone()).is_err(), "{fragment}");
        }
    }
    #[test]
    fn static_resource_anchor_recursion_bundles_and_keeps_original_documents() {
        let root = serde_json::json!({"openapi":"3.1.0","info":{"title":"Resources","version":"1"},"paths":{},"components":{"schemas":{"Node":{"$ref":"https://schemas.test/node#node"},"Other":{"$ref":"model.json","description":"retrieval alias"}}}});
        let model = serde_json::json!({"$id":"https://schemas.test/node","$anchor":"node","type":"object","properties":{"next":{"$ref":"#node"},"name":{"type":"string"}},"example":{"$id":"literal","$ref":"outside.json"}});
        let input = OpenapiSourceBundle {
            format: SOURCE_BUNDLE_FORMAT.into(),
            entry_file: "root.json".into(),
            files: vec![
                source("root.json", root.clone()),
                source("model.json", model.clone()),
            ],
        };
        let result = bundle_openapi(input).unwrap();
        assert_eq!(
            result.specification["components"]["schemas"]["Node"]["properties"]["next"]["$ref"],
            "#/components/schemas/Node"
        );
        assert_eq!(
            result.specification["components"]["schemas"]["Other"]["description"],
            "retrieval alias"
        );
        assert_eq!(
            result.specification["components"]["schemas"]["Node"]["example"],
            model["example"]
        );
        assert!(result.documents.contains(&root));
        assert!(result.documents.contains(&model));
        assert!(
            result.specification["components"]["schemas"]["Node"]
                .get("$id")
                .is_none()
        );
    }

    #[test]
    fn static_resources_keep_dynamic_and_30_metadata_explicitly_unsupported() {
        {
            let schema = serde_json::json!({"$dynamicRef":"#node"});
            let root = serde_json::json!({"openapi":"3.1.0","info":{"title":"Unsupported","version":"1"},"paths":{},"components":{"schemas":{"Node":schema}}});
            let input = OpenapiSourceBundle {
                format: SOURCE_BUNDLE_FORMAT.into(),
                entry_file: "root.json".into(),
                files: vec![
                    source("root.json", root),
                    source("model.json", serde_json::json!({"type":"string"})),
                ],
            };
            assert!(bundle_openapi(input).is_err());
        }
        let root = serde_json::json!({"openapi":"3.0.3","info":{"title":"Unsupported","version":"1"},"paths":{},"components":{"schemas":{"Node":{"$id":"https://schemas.test/node","type":"string"}}}});
        assert!(
            bundle_openapi(OpenapiSourceBundle {
                format: SOURCE_BUNDLE_FORMAT.into(),
                entry_file: "root.json".into(),
                files: vec![source("root.json", root)]
            })
            .is_err()
        );
    }
    fn assert_sibling_validation_equivalent(
        model: Value,
        restriction: Value,
        instances: &[(Value, bool)],
    ) -> BundledOpenapi {
        let root = serde_json::json!({"openapi":"3.1.0","info":{"title":"Sibling constraints","version":"1"},"paths":{},"components":{"schemas":{
            "Base":{"$ref":"moleapi://source-bundle/model.json"}, "Restricted":restriction
        }}});
        let input = OpenapiSourceBundle {
            format: SOURCE_BUNDLE_FORMAT.into(),
            entry_file: "root.json".into(),
            files: vec![
                source("root.json", root.clone()),
                source("model.json", model.clone()),
            ],
        };
        let registry = jsonschema::Registry::new()
            .draft(jsonschema::Draft::Draft202012)
            .retriever(referencing::DefaultRetriever)
            .extend([(
                "moleapi://source-bundle/model.json",
                jsonschema::Resource::from_contents(model.clone()),
            )])
            .unwrap()
            .prepare()
            .unwrap();
        let original = jsonschema::options()
            .with_draft(jsonschema::Draft::Draft202012)
            .with_registry(&registry)
            .build(&restriction)
            .unwrap();
        let result = bundle_openapi(input).unwrap();
        let wrapper = serde_json::json!({"$ref":"#/components/schemas/Restricted","components":result.specification["components"]});
        let projected = jsonschema::options()
            .with_draft(jsonschema::Draft::Draft202012)
            .build(&wrapper)
            .unwrap();
        for (instance, accepted) in instances {
            assert_eq!(
                original.is_valid(instance),
                *accepted,
                "original: {instance}"
            );
            assert_eq!(
                projected.is_valid(instance),
                *accepted,
                "projection: {instance}"
            );
        }
        assert!(result.documents.contains(&root));
        assert!(result.documents.contains(&model));
        result
    }

    #[test]
    fn reference_siblings_preserve_external_string_constraints_annotations_and_literals() {
        let literal = serde_json::json!({"$ref":"outside.json","$id":"literal"});
        let result = assert_sibling_validation_equivalent(
            serde_json::json!({"$id":"https://schemas.test/model","type":"string","minLength":1}),
            serde_json::json!({"$ref":"https://schemas.test/model","maxLength":3,"pattern":"^[a-z]+$","description":"narrow string","example":literal,"x-literal":literal}),
            &[
                (serde_json::json!("abc"), true),
                (serde_json::json!(""), false),
                (serde_json::json!("abcd"), false),
                (serde_json::json!("AB"), false),
                (serde_json::json!(12), false),
            ],
        );
        let restricted = &result.specification["components"]["schemas"]["Restricted"];
        assert_eq!(restricted["description"], "narrow string");
        assert_eq!(restricted["example"], literal);
        assert_eq!(restricted["x-literal"], literal);
        assert!(restricted.get("$ref").is_none());
        assert_eq!(restricted["allOf"][0]["$ref"], "#/components/schemas/Base");
    }

    #[test]
    fn reference_siblings_keep_evaluated_properties_on_the_outer_schema() {
        assert_sibling_validation_equivalent(
            serde_json::json!({"$id":"https://schemas.test/model","type":"object","properties":{"id":{"type":"integer"}},"required":["id"]}),
            serde_json::json!({"$ref":"https://schemas.test/model","properties":{"name":{"type":"string"}},"required":["name"],"unevaluatedProperties":false}),
            &[
                (serde_json::json!({"id":1,"name":"ok"}), true),
                (serde_json::json!({"id":1}), false),
                (serde_json::json!({"name":"ok"}), false),
                (serde_json::json!({"id":1,"name":"ok","extra":1}), false),
                (serde_json::json!({"id":"bad","name":"ok"}), false),
            ],
        );
    }

    #[test]
    fn reference_siblings_keep_evaluated_items_and_existing_allof_branches() {
        let result = assert_sibling_validation_equivalent(
            serde_json::json!({"$id":"https://schemas.test/model","type":"array","prefixItems":[{"type":"string"}]}),
            serde_json::json!({"$ref":"https://schemas.test/model","allOf":[{"minItems":1}],"unevaluatedItems":false}),
            &[
                (serde_json::json!(["ok"]), true),
                (serde_json::json!([]), false),
                (serde_json::json!(["ok", 1]), false),
                (serde_json::json!([1]), false),
            ],
        );
        assert_eq!(
            result.specification["components"]["schemas"]["Restricted"]["allOf"][0]["minItems"],
            1
        );
        assert_eq!(
            result.specification["components"]["schemas"]["Restricted"]["allOf"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn reference_siblings_preserve_recursion_and_boolean_schema_targets() {
        assert_sibling_validation_equivalent(
            serde_json::json!({"$id":"https://schemas.test/model","$anchor":"node","type":"object","properties":{"next":{"$ref":"#node"},"name":{"type":"string"}}}),
            serde_json::json!({"$ref":"https://schemas.test/model#node","maxProperties":1}),
            &[
                (serde_json::json!({"name":"ok"}), true),
                (serde_json::json!({"next":{"name":"ok"}}), true),
                (serde_json::json!({"next":{"name":1}}), false),
                (serde_json::json!({"name":"ok","next":{}}), false),
            ],
        );
        for accepted in [true, false] {
            assert_sibling_validation_equivalent(
                Value::Bool(accepted),
                serde_json::json!({"$ref":"moleapi://source-bundle/model.json","type":"string"}),
                &[
                    (serde_json::json!("ok"), accepted),
                    (serde_json::json!(1), false),
                ],
            );
        }
    }

    #[test]
    fn non_schema_reference_objects_and_30_siblings_still_reject_unsupported_constraints() {
        for version in ["3.0.3", "3.1.0"] {
            let mut root = serde_json::json!({"openapi":version,"info":{"title":"Reference objects","version":"1"},"paths":{},"components":{"schemas":{"Restricted":{"$ref":"model.json","maxLength":3}}}});
            if version == "3.1.0" {
                root["components"] = serde_json::json!({"responses":{"Response":{"$ref":"responses.json#/Response","content":{"application/json":{}}}}});
            }
            let input = OpenapiSourceBundle {
                format: SOURCE_BUNDLE_FORMAT.into(),
                entry_file: "root.json".into(),
                files: vec![
                    source("root.json", root),
                    source("model.json", serde_json::json!({"type":"string"})),
                    source(
                        "responses.json",
                        serde_json::json!({"Response":{"description":"OK"}}),
                    ),
                ],
            };
            assert!(bundle_openapi(input).is_err(), "{version}");
        }
    }
}
