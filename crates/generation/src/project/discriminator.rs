//! Promote supplied discriminator resources to stable model components; emitters stay upstream.
use super::*;
use anyhow::Context;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use url::Url;

fn component_uri(document: &Url, name: &str) -> Url {
    let mut uri = document.clone();
    let name = name.replace('~', "~0").replace('/', "~1");
    uri.set_fragment(Some(&schematools::storage::encode_pointer_fragment(
        &format!("/components/schemas/{name}"),
    )));
    uri
}
fn reference_uri(document: &Url, reference: &str) -> Result<Url> {
    let mut uri = schematools::storage::ref_to_url(document, reference)
        .context("Invalid discriminator/reference URI")?;
    let pointer = schematools::storage::decode_pointer_fragment(uri.fragment().unwrap_or(""))
        .context("Invalid discriminator URI-fragment pointer")?;
    let encoded = schematools::storage::encode_pointer_fragment(&pointer);
    uri.set_fragment(if pointer.is_empty() {
        None
    } else {
        Some(&encoded)
    });
    Ok(uri)
}
fn walk(
    value: &mut Value,
    dictionary: bool,
    visit: &mut impl FnMut(&mut serde_json::Map<String, Value>) -> Result<()>,
) -> Result<()> {
    match value {
        Value::Object(object) => {
            if !dictionary {
                visit(object)?;
            }
            for (key, value) in object {
                if !reference_policy::literal_member(key, value, dictionary) {
                    walk(
                        value,
                        !dictionary && reference_policy::dictionary_member(key),
                        visit,
                    )?;
                }
            }
        }
        Value::Array(array) => {
            for value in array {
                walk(value, false, visit)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn mapping(
    object: &mut serde_json::Map<String, Value>,
) -> Option<&mut serde_json::Map<String, Value>> {
    object
        .get_mut("discriminator")?
        .get_mut("mapping")?
        .as_object_mut()
}
fn names(document: &Value) -> BTreeSet<String> {
    document
        .pointer("/components/schemas")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|object| object.keys().cloned())
        .collect()
}
fn supplied_schema<'a>(
    documents: &'a BTreeMap<Url, Value>,
    schemas: &BTreeMap<Url, HashSet<usize>>,
    uri: &Url,
) -> Result<&'a Value> {
    let mut document = uri.clone();
    document.set_fragment(None);
    let pointer = schematools::storage::decode_pointer_fragment(uri.fragment().unwrap_or(""))
        .context("Invalid discriminator pointer")?;
    let value = documents
        .get(&document)
        .and_then(|value| value.pointer(&pointer))
        .context("Discriminator target is outside supplied schemas")?;
    ensure!(
        value.is_object() || value.is_boolean(),
        "Discriminator target must be a schema"
    );
    if let Some(nodes) = schemas.get(&document) {
        ensure!(
            nodes.contains(&(std::ptr::from_ref(value) as usize)),
            "Discriminator target is not an OpenAPI schema"
        );
    }
    Ok(value)
}
fn alias_name(uri: &Url, used: &mut BTreeSet<String>) -> String {
    let pointer = schematools::storage::decode_pointer_fragment(uri.fragment().unwrap_or(""))
        .unwrap_or_default();
    let hint = if pointer.is_empty() {
        uri.path_segments()
            .and_then(Iterator::last)
            .unwrap_or("SourceModel")
            .to_owned()
    } else {
        pointer
            .rsplit('/')
            .next()
            .unwrap()
            .replace("~1", "/")
            .replace("~0", "~")
    };
    let hint = hint
        .strip_suffix(".json")
        .or_else(|| hint.strip_suffix(".yaml"))
        .or_else(|| hint.strip_suffix(".yml"))
        .unwrap_or(&hint);
    let hint: String = hint
        .chars()
        .take(64)
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect();
    let hint = hint.trim_matches('_');
    let prefix = if hint.is_empty()
        || !hint.starts_with(|character: char| character.is_ascii_alphabetic())
    {
        "SourceModel_"
    } else {
        ""
    };
    let digest = format!("{:x}", Sha256::digest(uri.as_str().as_bytes()));
    let base = format!("{prefix}{hint}_{}", &digest[..12]);
    let mut candidate = base.clone();
    let mut suffix = 1;
    while !used.insert(candidate.clone()) {
        candidate = format!("{base}_{suffix}");
        suffix += 1;
    }
    candidate
}

/// Return whether promotion requires preserving canonical entry model pointers at every depth.
pub(super) fn normalize(documents: &mut BTreeMap<Url, Value>, entry: &Url) -> Result<bool> {
    let schema_nodes = documents
        .iter()
        .filter(|(_, document)| document.get("openapi").is_some())
        .map(|(url, document)| {
            (
                url.clone(),
                schema_resources::openapi_schema_nodes(document),
            )
        })
        .collect();
    let root_names = names(&documents[entry]);
    let mut aliases = BTreeMap::new();
    for name in &root_names {
        aliases.insert(component_uri(entry, name), name.clone());
        if let Some(reference) = documents[entry]["components"]["schemas"][name]["$ref"].as_str() {
            aliases.insert(reference_uri(entry, reference)?, name.clone());
        }
    }
    let mut targets = BTreeSet::new();
    let mut resolved = BTreeMap::new();
    for (url, document) in documents.iter_mut() {
        let local_names = names(document);
        let dictionary = reference_policy::root_dictionary(document);
        walk(document, dictionary, &mut |object| {
            if let Some(mapping) = mapping(object) {
                for value in mapping.values_mut() {
                    let reference = value.as_str().context("Invalid discriminator mapping")?;
                    let uri = if url != entry && local_names.contains(reference) {
                        component_uri(url, reference)
                    } else if root_names.contains(reference) {
                        continue;
                    } else {
                        reference_uri(url, reference)?
                    };
                    resolved.insert((url.clone(), reference.to_owned()), uri.clone());
                    targets.insert(uri);
                }
            }
            Ok(())
        })?;
    }
    let mut used = root_names;
    let mut promoted = BTreeMap::new();
    for uri in targets {
        supplied_schema(documents, &schema_nodes, &uri)?;
        if let std::collections::btree_map::Entry::Vacant(alias) = aliases.entry(uri.clone()) {
            ensure!(
                promoted.len() < 256,
                "Discriminator subtype alias limit exceeded"
            );
            let name = alias_name(&uri, &mut used);
            alias.insert(name.clone());
            promoted.insert(uri, name);
        }
    }
    for (url, document) in documents.iter_mut() {
        let dictionary = reference_policy::root_dictionary(document);
        walk(document, dictionary, &mut |object| {
            if let Some(mapping) = mapping(object) {
                for value in mapping.values_mut() {
                    let reference = value.as_str().context("Invalid discriminator mapping")?;
                    if let Some(uri) = resolved.get(&(url.clone(), reference.to_owned())) {
                        *value = aliases
                            .get(uri)
                            .context("Missing discriminator alias")?
                            .clone()
                            .into();
                    }
                }
            }
            if let Some(reference) = object.get_mut("$ref") {
                let uri =
                    reference_uri(url, reference.as_str().context("Invalid schema reference")?)?;
                if let Some(name) = promoted.get(&uri) {
                    *reference = component_uri(entry, name).to_string().into();
                }
            }
            Ok(())
        })?;
    }
    if promoted.is_empty() {
        return Ok(false);
    }
    let root = documents
        .get_mut(entry)
        .unwrap()
        .as_object_mut()
        .context("Invalid source entry")?;
    let components = root
        .entry("components")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .context("Invalid components")?;
    let schemas = components
        .entry("schemas")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .context("Invalid schema components")?;
    let authored = std::mem::take(schemas);
    for (uri, name) in promoted {
        schemas.insert(name, serde_json::json!({"$ref":uri.as_str()}));
    }
    schemas.extend(authored);
    ensure!(
        serde_json::to_vec(&documents.values().collect::<Vec<_>>())?.len() <= 2 * SPEC_LIMIT,
        "Discriminator projection size limit exceeded"
    );
    Ok(true)
}
