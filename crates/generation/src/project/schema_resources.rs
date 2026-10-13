//! Resolve supplied static schema resources before the existing document bundler.
use super::*;
use anyhow::Context;
use referencing::{Draft, Registry, Resolver, ResourceRef, Uri};
use std::collections::{BTreeSet, HashMap, HashSet};
use url::Url;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Location {
    document: Url,
    pointer: String,
}
fn address(value: &Value) -> usize {
    std::ptr::from_ref(value) as usize
}
fn child(pointer: &str, name: &str) -> String {
    format!("{pointer}/{}", name.replace('~', "~0").replace('/', "~1"))
}
fn schema_roots(value: &Value, pointer: &str, dictionary: bool, roots: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                if reference_policy::literal_member(key, value, dictionary) {
                    continue;
                }
                let path = child(pointer, key);
                if !dictionary && key == "schema" && (value.is_object() || value.is_boolean()) {
                    roots.push(path.clone());
                }
                if !dictionary
                    && key == "schemas"
                    && let Some(schemas) = value.as_object()
                {
                    roots.extend(schemas.keys().map(|key| child(&path, key)));
                }
                schema_roots(
                    value,
                    &path,
                    !dictionary && reference_policy::dictionary_member(key),
                    roots,
                );
            }
        }
        Value::Array(array) => {
            for (index, value) in array.iter().enumerate() {
                schema_roots(value, &format!("{pointer}/{index}"), false, roots);
            }
        }
        _ => {}
    }
}
fn mark_schemas(value: &Value, nodes: &mut HashSet<usize>) {
    if !nodes.insert(address(value)) {
        return;
    }
    for child in Draft::Draft202012.subresources_of(value) {
        mark_schemas(child, nodes);
    }
}
fn index_locations(value: &Value, location: &Location, index: &mut HashMap<usize, Location>) {
    index.insert(address(value), location.clone());
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                index_locations(
                    value,
                    &Location {
                        document: location.document.clone(),
                        pointer: child(&location.pointer, key),
                    },
                    index,
                );
            }
        }
        Value::Array(array) => {
            for (position, value) in array.iter().enumerate() {
                index_locations(
                    value,
                    &Location {
                        document: location.document.clone(),
                        pointer: format!("{}/{position}", location.pointer),
                    },
                    index,
                );
            }
        }
        _ => {}
    }
}
fn insert_identity(
    identities: &mut BTreeMap<String, Location>,
    uri: String,
    location: &Location,
) -> Result<()> {
    if let Some(previous) = identities.get(&uri) {
        ensure!(
            previous == location,
            "Duplicate schema resource ID or anchor"
        );
    } else {
        identities.insert(uri, location.clone());
    }
    Ok(())
}
fn declarations(
    value: &Value,
    base: &Uri<String>,
    location: &Location,
    dictionary: bool,
    nodes: &HashSet<usize>,
    identities: &mut BTreeMap<String, Location>,
) -> Result<()> {
    match value {
        Value::Object(object) => {
            let mut base = base.clone();
            if !dictionary {
                ensure!(
                    !object.keys().any(|key| [
                        "$dynamicRef",
                        "$dynamicAnchor",
                        "$recursiveRef",
                        "$recursiveAnchor"
                    ]
                    .contains(&key.as_str())),
                    "Dynamic/recursive schema resources require supported generation semantics"
                );
                if object.contains_key("$id") || object.contains_key("$anchor") {
                    ensure!(
                        nodes.contains(&address(value)),
                        "Resource metadata must occur in a schema subresource"
                    );
                }
                if nodes.contains(&address(value)) {
                    if let Some(schema) = object.get("$schema") {
                        ensure!(
                            schema.as_str().is_some_and(|schema| [
                                "https://json-schema.org/draft/2020-12/schema",
                                "http://json-schema.org/draft/2020-12/schema",
                                "https://spec.openapis.org/oas/3.1/dialect/base"
                            ]
                            .contains(&schema.trim_end_matches('#'))),
                            "Unsupported source schema dialect"
                        );
                    }
                    if let Some(id) = object.get("$id") {
                        let id = id.as_str().context("Schema resource ID must be a string")?;
                        let id = id.strip_suffix('#').unwrap_or(id);
                        base = referencing::uri::resolve_against(&base.borrow(), id)
                            .context("Invalid schema resource ID")?;
                        ensure!(
                            !base.as_str().contains('#'),
                            "Schema resource ID cannot contain a fragment"
                        );
                        insert_identity(identities, base.to_string(), location)?;
                    }
                    if let Some(anchor) = object.get("$anchor") {
                        let anchor = anchor.as_str().context("Schema anchor must be a string")?;
                        ensure!(
                            !anchor.is_empty()
                                && anchor.len() <= 256
                                && anchor
                                    .bytes()
                                    .next()
                                    .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
                                && anchor.bytes().all(|byte| byte.is_ascii_alphanumeric()
                                    || matches!(byte, b'_' | b'-' | b'.')),
                            "Invalid schema anchor name"
                        );
                        insert_identity(identities, format!("{base}#{anchor}"), location)?;
                    }
                }
            }
            for (key, value) in object {
                if !reference_policy::literal_member(key, value, dictionary) {
                    declarations(
                        value,
                        &base,
                        &Location {
                            document: location.document.clone(),
                            pointer: child(&location.pointer, key),
                        },
                        !dictionary && reference_policy::dictionary_member(key),
                        nodes,
                        identities,
                    )?;
                }
            }
        }
        Value::Array(array) => {
            for (index, value) in array.iter().enumerate() {
                declarations(
                    value,
                    base,
                    &Location {
                        document: location.document.clone(),
                        pointer: format!("{}/{index}", location.pointer),
                    },
                    false,
                    nodes,
                    identities,
                )?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn target(
    reference: &str,
    resolver: &Resolver<'_>,
    document: &Url,
    locations: &HashMap<usize, Location>,
) -> Result<String> {
    // Normalize encoded leading slashes before the registry's pointer/anchor dispatch.
    // The shared decoder validates escapes; the registry then decodes exactly once.
    let canonical;
    let reference = if let Some((uri, fragment)) = reference.rsplit_once('#') {
        if fragment.starts_with('/') || fragment.to_ascii_lowercase().starts_with("%2f") {
            let pointer = schematools::storage::decode_pointer_fragment(fragment)
                .context("Invalid URI-fragment JSON Pointer")?;
            canonical = format!(
                "{uri}#{}",
                schematools::storage::encode_pointer_fragment(&pointer)
            );
            canonical.as_str()
        } else {
            // Reuse strict UTF8/percent-triplet decoding for plain-name anchors.
            let decoded = schematools::storage::decode_pointer_fragment(&format!("/{fragment}"))
                .context("Invalid URI-fragment schema anchor")?;
            canonical = format!("{uri}#{}", &decoded[1..]);
            canonical.as_str()
        }
    } else {
        reference
    };
    let resolved = resolver
        .lookup(reference)
        .context("Schema reference is not in supplied resources")?;
    let location = locations
        .get(&address(resolved.contents()))
        .context("Resolved resource has no supplied source location")?;
    let fragment = schematools::storage::encode_pointer_fragment(&location.pointer);
    Ok(if &location.document == document {
        format!("#{fragment}")
    } else {
        format!("{}#{fragment}", location.document)
    })
}
struct RewriteContext<'a> {
    locations: &'a HashMap<usize, Location>,
    names: &'a BTreeSet<String>,
    schemas: &'a HashSet<usize>,
}

// Keep evaluation-dependent siblings on the outer schema. Appending leaves
// pointers into authored allOf branches stable while including the ref's annotations.
fn compose_reference_siblings(object: &mut serde_json::Map<String, Value>) -> Result<()> {
    if object.contains_key("$ref")
        && object
            .keys()
            .any(|key| !["$ref", "summary", "description"].contains(&key.as_str()))
    {
        let reference = object.remove("$ref").unwrap();
        let branches = object
            .entry("allOf")
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
            .context("Schema allOf must be an array")?;
        branches.push(serde_json::json!({"$ref": reference}));
    }
    Ok(())
}

fn rewrite(
    original: &Value,
    value: &mut Value,
    resolver: &Resolver<'_>,
    document: &Url,
    dictionary: bool,
    context: &RewriteContext<'_>,
) -> Result<()> {
    let is_schema = context.schemas.contains(&address(original));
    match (original, value) {
        (Value::Object(original), Value::Object(object)) => {
            // Enter the original declaration before rewriting its references and descendants.
            let mut resolver = resolver.clone();
            if !dictionary {
                if let Some(id) = original.get("$id") {
                    let resource = serde_json::json!({"$id":id});
                    resolver = resolver
                        .in_subresource(ResourceRef::new(&resource, Draft::Draft202012))
                        .context("Invalid schema resource scope")?;
                }
                if let Some(reference) = original.get("$ref") {
                    object.insert(
                        "$ref".into(),
                        target(
                            reference
                                .as_str()
                                .context("Schema reference must be a string")?,
                            &resolver,
                            document,
                            context.locations,
                        )?
                        .into(),
                    );
                }
                if let Some(mapping) = object
                    .get_mut("discriminator")
                    .and_then(|value| value.get_mut("mapping"))
                    .and_then(Value::as_object_mut)
                {
                    for value in mapping.values_mut() {
                        let reference =
                            value.as_str().context("Invalid discriminator reference")?;
                        if !context.names.contains(reference) {
                            *value =
                                target(reference, &resolver, document, context.locations)?.into();
                        }
                    }
                }
                object.remove("$id");
                object.remove("$anchor");
            }
            for (key, original) in original {
                if !reference_policy::literal_member(key, original, dictionary)
                    && let Some(value) = object.get_mut(key)
                {
                    rewrite(
                        original,
                        value,
                        &resolver,
                        document,
                        !dictionary && reference_policy::dictionary_member(key),
                        context,
                    )?;
                }
            }
            if is_schema {
                compose_reference_siblings(object)?;
            }
        }
        (Value::Array(original), Value::Array(array)) => {
            for (original, value) in original.iter().zip(array) {
                rewrite(original, value, resolver, document, false, context)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn normalize(
    documents: &BTreeMap<Url, Value>,
    entry: &Url,
) -> Result<BTreeMap<Url, Value>> {
    for document in documents.values() {
        if let Some(dialect) = document.get("jsonSchemaDialect") {
            ensure!(
                dialect.as_str().is_some_and(|dialect| [
                    "https://spec.openapis.org/oas/3.1/dialect/base",
                    "https://json-schema.org/draft/2020-12/schema",
                ]
                .contains(&dialect.trim_end_matches('#'))),
                "Unsupported OpenAPI schema dialect"
            );
        }
    }
    let mut nodes = HashSet::new();
    let mut roots = BTreeMap::new();
    for (url, document) in documents {
        let mut candidates = Vec::new();
        if document.get("openapi").is_none() {
            candidates.push(String::new());
        }
        schema_roots(
            document,
            "",
            reference_policy::root_dictionary(document),
            &mut candidates,
        );
        candidates.sort_by_key(String::len);
        let mut selected = Vec::new();
        for pointer in candidates {
            let value = document
                .pointer(&pointer)
                .context("Invalid schema root location")?;
            if !nodes.contains(&address(value)) {
                mark_schemas(value, &mut nodes);
                selected.push(pointer);
            }
        }
        roots.insert(url.clone(), selected);
    }
    let mut identities = BTreeMap::new();
    for url in documents.keys() {
        identities.insert(
            url.to_string(),
            Location {
                document: url.clone(),
                pointer: String::new(),
            },
        );
    }
    for (url, document) in documents {
        declarations(
            document,
            &referencing::uri::from_str(url.as_str())?,
            &Location {
                document: url.clone(),
                pointer: String::new(),
            },
            reference_policy::root_dictionary(document),
            &nodes,
            &mut identities,
        )?;
    }
    let mut projections = documents.clone();
    let mut additions = Vec::new();
    for (url, document) in &mut projections {
        if document.get("openapi").is_none() {
            continue;
        }
        let definitions = document
            .as_object_mut()
            .context("Invalid OpenAPI registry document")?
            .entry("$defs")
            .or_insert_with(|| serde_json::json!({}))
            .as_object_mut()
            .context("Invalid registry definitions")?;
        for (position, pointer) in roots[url].iter().enumerate() {
            let name = format!("__moleapi_registry_{position}");
            ensure!(
                !definitions.contains_key(&name),
                "Reserved schema registry name collision"
            );
            definitions.insert(
                name.clone(),
                documents[url].pointer(pointer).unwrap().clone(),
            );
            additions.push((url.clone(), child("/$defs", &name), pointer.clone()));
        }
    }
    ensure!(
        serde_json::to_vec(&projections.values().collect::<Vec<_>>())?.len() <= 3 * SPEC_LIMIT,
        "Schema registry projection size limit exceeded"
    );
    let mut locations = HashMap::new();
    for (url, document) in &projections {
        index_locations(
            document,
            &Location {
                document: url.clone(),
                pointer: String::new(),
            },
            &mut locations,
        );
    }
    for (url, projected, original) in additions {
        index_locations(
            projections[&url].pointer(&projected).unwrap(),
            &Location {
                document: url,
                pointer: original,
            },
            &mut locations,
        );
    }
    let registry =
        Registry::new()
            .draft(Draft::Draft202012)
            .retriever(referencing::DefaultRetriever)
            .extend(projections.iter().map(|(url, document)| {
                (url.as_str(), ResourceRef::new(document, Draft::Draft202012))
            }))?
            .prepare()
            .context("Invalid supplied schema resource registry")?;
    let names = documents[entry]
        .pointer("/components/schemas")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|object| object.keys().cloned())
        .collect();
    let context = RewriteContext {
        locations: &locations,
        names: &names,
        schemas: &nodes,
    };
    let mut normalized = documents.clone();
    for (url, value) in &mut normalized {
        let resolver = registry.resolver(referencing::uri::from_str(url.as_str())?);
        rewrite(
            &documents[url],
            value,
            &resolver,
            url,
            reference_policy::root_dictionary(&documents[url]),
            &context,
        )?;
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry() -> Url {
        Url::parse("moleapi://source-bundle/api.json").unwrap()
    }
    fn root(schemas: Value) -> Value {
        json!({"openapi":"3.1.0","info":{"title":"Resources","version":"1"},"paths":{},"components":{"schemas":schemas}})
    }
    fn documents(schemas: Value, model: Value) -> BTreeMap<Url, Value> {
        BTreeMap::from([
            (entry(), root(schemas)),
            (entry().join("model.json").unwrap(), model),
        ])
    }

    #[test]
    fn nested_relative_ids_static_anchors_and_retrieval_aliases_rebase_to_sources() {
        let model = json!({"$id":"https://schemas.example.test/base/root#","$defs":{
            "Node":{"$id":"nested/node","$anchor":"node","type":"object","properties":{
                "self":{"$ref":"#node"},"leaf":{"$ref":"leaf#value"}}},
            "Leaf":{"$id":"nested/leaf","$anchor":"value","type":"string"}
        }});
        let input = documents(
            json!({
                "Declared":{"$ref":"https://schemas.example.test/base/nested/node#node"},
                "Retrieved":{"$ref":"model.json#/$defs/Node"},
                "Encoded":{"$ref":"model.json#%2F$defs%2FNode"},
            "EncodedAnchor":{"$ref":"https://schemas.example.test/base/nested/node#%6eode"}
            }),
            model.clone(),
        );
        let normalized = normalize(&input, &entry()).unwrap();
        let expected = "moleapi://source-bundle/model.json#/%24defs/Node";
        for name in ["Declared", "Retrieved", "Encoded", "EncodedAnchor"] {
            assert_eq!(
                normalized[&entry()]["components"]["schemas"][name]["$ref"],
                expected
            );
        }
        let projected = &normalized[&entry().join("model.json").unwrap()];
        assert_eq!(
            projected["$defs"]["Node"]["properties"]["self"]["$ref"],
            "#/%24defs/Node"
        );
        assert_eq!(
            projected["$defs"]["Node"]["properties"]["leaf"]["$ref"],
            "#/%24defs/Leaf"
        );
        assert!(projected.get("$id").is_none());
        assert!(projected["$defs"]["Node"].get("$anchor").is_none());
        assert_eq!(input[&entry().join("model.json").unwrap()], model);
    }

    #[test]
    fn component_ids_and_schema_fields_are_discovered_without_touching_literals() {
        let literal = json!({"$id":"https://outside.test/literal","$anchor":"invalid space","$dynamicRef":"#bad","$ref":"missing.json"});
        let mut doc = root(json!({
            "Node":{"$id":"https://schemas.example.test/node","$anchor":"node","type":"object", "properties":{"name":{"type":"string"}},"example":literal,"default":literal,"x-resource":literal},
            "Alias":{"$ref":"https://schemas.example.test/node#node"},
            "schema":{"type":"string"}
        }));
        doc["paths"] = json!({"/node":{"get":{"responses":{"200":{"description":"OK","content":{"application/json":{"schema":{"$ref":"https://schemas.example.test/node"}}}}}}}});
        let input = BTreeMap::from([(entry(), doc)]);
        let normalized = normalize(&input, &entry()).unwrap();
        let node = &normalized[&entry()]["components"]["schemas"]["Node"];
        for key in ["example", "default", "x-resource"] {
            assert_eq!(node[key], literal);
        }
        assert_eq!(
            normalized[&entry()]["components"]["schemas"]["Alias"]["$ref"],
            "#/components/schemas/Node"
        );
        assert_eq!(
            normalized[&entry()]["paths"]["/node"]["get"]["responses"]["200"]["content"]["application/json"]
                ["schema"]["$ref"],
            "#/components/schemas/Node"
        );
        assert!(
            input[&entry()]["components"]["schemas"]["Node"]
                .get("$id")
                .is_some()
        );
    }

    #[test]
    fn identities_and_dialects_are_validated_before_resolution() {
        for schemas in [
            json!({"A":{"$id":"https://schemas.test/duplicate"},"B":{"$id":"https://schemas.test/duplicate"}}),
            json!({"A":{"$anchor":"same"},"B":{"$anchor":"same"}}),
            json!({"A":{"$id":"https://schemas.test/id#fragment"}}),
            json!({"A":{"$id":12}}),
            json!({"A":{"$id":"http://[invalid"}}),
            json!({"A":{"$anchor":"invalid space"}}),
            json!({"A":{"$anchor":"1start"}}),
            json!({"A":{"$anchor":false}}),
            json!({"A":{"$schema":"https://json-schema.org/draft-07/schema"}}),
            json!({"A":{"$dynamicRef":"#node"}}),
            json!({"A":{"$dynamicAnchor":"node"}}),
            json!({"A":{"$recursiveRef":"#"}}),
            json!({"A":{"$recursiveAnchor":true}}),
        ] {
            let input = BTreeMap::from([(entry(), root(schemas.clone()))]);
            assert!(normalize(&input, &entry()).is_err(), "{schemas}");
        }
        let mut doc = root(json!({"A":{"type":"string"}}));
        doc["jsonSchemaDialect"] = "https://outside.test/custom-dialect".into();
        assert!(normalize(&BTreeMap::from([(entry(), doc)]), &entry()).is_err());
    }

    #[test]
    fn missing_resources_anchors_and_invalid_pointer_escapes_fail_without_fetching() {
        let model = json!({"$id":"https://schemas.test/model","type":"object","$defs":{"literal%2F":{"type":"string"}}});
        for reference in [
            "https://outside.test/missing",
            "missing.json",
            "model.json#missing",
            "model.json#bad%GG",
            "model.json#/$defs/literal%",
            "model.json#/$defs/literal%GG",
            "model.json#/$defs/literal%FF",
            "model.json#/$defs/literal~2",
            "model.json#/$defs/missing",
            "../../outside.json",
        ] {
            let input = documents(json!({"A":{"$ref":reference}}), model.clone());
            assert!(normalize(&input, &entry()).is_err(), "{reference}");
        }
        let input = documents(
            json!({"A":{"$ref":"model.json#/$defs/literal%252F"}}),
            model,
        );
        assert_eq!(
            normalize(&input, &entry()).unwrap()[&entry()]["components"]["schemas"]["A"]["$ref"],
            "moleapi://source-bundle/model.json#/%24defs/literal%252F"
        );
    }

    #[test]
    fn discriminator_resource_targets_rebase_but_component_names_stay_names() {
        let input = documents(
            json!({
                "Node":{"$ref":"https://schemas.test/node#node"},
                "Union":{"oneOf":[{"$ref":"https://schemas.test/node#node"}],"discriminator":{"propertyName":"kind","mapping":{"node":"https://schemas.test/node#node","named":"Node"}}}
            }),
            json!({"$id":"https://schemas.test/node","$anchor":"node","type":"object"}),
        );
        let normalized = normalize(&input, &entry()).unwrap();
        let mapping =
            &normalized[&entry()]["components"]["schemas"]["Union"]["discriminator"]["mapping"];
        assert_eq!(mapping["node"], "moleapi://source-bundle/model.json#");
        assert_eq!(mapping["named"], "Node");
    }
    #[test]
    fn composed_siblings_leave_authored_allof_pointer_indices_stable() {
        let input = documents(
            json!({
                "Restricted":{"$ref":"model.json","allOf":[{"type":"string","maxLength":3}],"minLength":1},
                "FirstBranch":{"$ref":"#/components/schemas/Restricted/allOf/0"}
            }),
            json!({"type":"string"}),
        );
        let normalized = normalize(&input, &entry()).unwrap();
        let schemas = &normalized[&entry()]["components"]["schemas"];
        assert_eq!(
            schemas["Restricted"]["allOf"][0],
            json!({"type":"string","maxLength":3})
        );
        assert_eq!(
            schemas["Restricted"]["allOf"][1]["$ref"],
            "moleapi://source-bundle/model.json#"
        );
        assert_eq!(
            schemas["FirstBranch"]["$ref"],
            "#/components/schemas/Restricted/allOf/0"
        );
    }

    #[test]
    fn composed_siblings_reject_invalid_allof_arrays() {
        for allof in [json!({"type":"string"}), json!(false), json!("bad")] {
            let input = documents(
                json!({"Restricted":{"$ref":"model.json","allOf":allof}}),
                json!({"type":"string"}),
            );
            assert!(normalize(&input, &entry()).is_err());
        }
    }
}
