//! Native/hosted capability disclosure for the dedicated classic Dubbo editor.
use crate::{
    ApiError, AppState,
    auth::Identity,
    storage,
    workspaces::{owned, reconcile_sessions},
};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use moleapi_core::{
    DubboArgument, DubboConfig, NetworkPolicy, Protocol, RequestSpec, Specification, Workspace,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use uuid::Uuid;

#[derive(Serialize)]
pub(crate) struct Capability {
    available: bool,
    engine: &'static str,
}

pub(crate) async fn capability(State(state): State<AppState>) -> Json<Capability> {
    Json(Capability {
        available: state.dubbo_runtime.is_some()
            && state
                .project_runtime
                .java_path()
                .is_some_and(|path| path.is_file()),
        engine: "Apache Dubbo 3.3.6 classic/Hessian2",
    })
}

async fn read_metadata(
    config: &DubboConfig,
    policy: NetworkPolicy,
    timeout: Duration,
) -> anyhow::Result<moleapi_protocols::dubbo::DubboMetadataPreview> {
    if url::Url::parse(&config.metadata_center_url)?.scheme() == "nacos" {
        moleapi_protocols::dubbo::preview_nacos_metadata(
            &config.metadata_center_url,
            config,
            &config.metadata_application,
            policy,
            timeout,
        )
        .await
    } else {
        moleapi_protocols::dubbo::preview_zookeeper_metadata(
            &config.metadata_center_url,
            config,
            &config.metadata_application,
            config.metadata_auth.as_ref(),
            policy,
            timeout,
        )
        .await
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MetadataPreviewInput {
    request: RequestSpec,
}

pub(crate) async fn preview_metadata(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(workspace): Path<String>,
    Json(input): Json<MetadataPreviewInput>,
) -> Result<Json<moleapi_protocols::dubbo::DubboMetadataPreview>, ApiError> {
    owned(&state, &owner.0, &workspace).await?;
    moleapi_core::validate_request(&input.request, false)
        .map_err(|error| ApiError::bad(error.to_string()))?;
    let Protocol::Dubbo { config } = &input.request.protocol else {
        return Err(ApiError::bad("Metadata preview requires a Dubbo request"));
    };
    let policy = NetworkPolicy {
        allow_private_network: state.local || state.config.allow_private_network,
    };
    let preview = read_metadata(
        config,
        policy,
        Duration::from_millis(input.request.timeout_ms.min(15_000)),
    )
    .await
    .map_err(|error| ApiError::bad(error.to_string()))?;
    Ok(Json(preview))
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ImportAction {
    Skip,
    Add,
    Replace,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MethodChoice {
    signature: String,
    action: ImportAction,
    #[serde(default)]
    replace_request_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MetadataImportInput {
    request: RequestSpec,
    collection_id: String,
    expected_revision: i64,
    document_sha256: String,
    decisions: Vec<MethodChoice>,
}

fn signature(method: &moleapi_protocols::dubbo::DubboMetadataMethod) -> String {
    format!("{}({})", method.name, method.parameter_types.join(","))
}

fn argument_draft(
    type_name: &str,
    types: &[moleapi_protocols::dubbo::DubboMetadataType],
) -> Result<String, ApiError> {
    let definitions = types
        .iter()
        .map(|definition| (definition.type_name.as_str(), definition))
        .collect::<HashMap<_, _>>();
    fn value(
        type_name: &str,
        definitions: &HashMap<&str, &moleapi_protocols::dubbo::DubboMetadataType>,
        active: &mut HashSet<String>,
        depth: usize,
        nodes: &mut usize,
    ) -> Result<serde_json::Value, ApiError> {
        if depth >= 16 || *nodes >= 256 {
            return Err(ApiError::bad("Metadata DTO draft exceeds nesting limit"));
        }
        let base = type_name.split('<').next().unwrap_or(type_name);
        if type_name.ends_with("[]")
            || matches!(
                base,
                "java.util.List" | "java.util.Set" | "java.util.Collection" | "java.lang.Iterable"
            )
        {
            return Ok(json!([]));
        }
        if matches!(base, "java.util.Map" | "java.util.SortedMap") {
            return Ok(json!({}));
        }
        let Some(definition) = definitions.get(type_name) else {
            return Ok(serde_json::Value::Null);
        };
        if definition.properties.is_empty() || !active.insert(type_name.to_owned()) {
            return Ok(serde_json::Value::Null);
        }
        *nodes += 1;
        let mut fields = serde_json::Map::new();
        for property in &definition.properties {
            fields.insert(
                property.name.clone(),
                value(&property.type_name, definitions, active, depth + 1, nodes)?,
            );
        }
        active.remove(type_name);
        Ok(json!({"$class":type_name,"$":fields}))
    }
    let mut active = HashSet::new();
    let mut nodes = 0;
    let draft = value(type_name, &definitions, &mut active, 0, &mut nodes)?;
    let source = draft.to_string();
    if source.len() > 64 * 1024 {
        return Err(ApiError::bad(
            "Metadata DTO draft exceeds argument size limit",
        ));
    }
    Ok(source)
}

fn matching_request(
    request: &RequestSpec,
    source: &DubboConfig,
    method: &moleapi_protocols::dubbo::DubboMetadataMethod,
) -> bool {
    let Protocol::Dubbo { config } = &request.protocol else {
        return false;
    };
    config.service == source.service
        && config.version == source.version
        && config.group == source.group
        && config.method == method.name
        && config
            .arguments
            .iter()
            .map(|argument| argument.type_name.as_str())
            .eq(method.parameter_types.iter().map(String::as_str))
}

pub(crate) async fn import_metadata(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(workspace): Path<String>,
    Json(input): Json<MetadataImportInput>,
) -> Result<Json<Workspace>, ApiError> {
    if !(1..i64::MAX).contains(&input.expected_revision) {
        return Err(ApiError::bad("Invalid expected revision"));
    }
    if input.document_sha256.len() != 64
        || !input.document_sha256.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(ApiError::bad("Invalid metadata document digest"));
    }
    let mut w = owned(&state, &owner.0, &workspace).await?;
    if w.revision != input.expected_revision {
        return Err(ApiError::conflict("Workspace revision changed"));
    }
    let collection = w
        .data
        .collections
        .iter()
        .find(|row| row.id == input.collection_id)
        .ok_or_else(|| ApiError::bad("Metadata target collection not found"))?;
    let source = collection
        .requests
        .iter()
        .find(|row| row.id == input.request.id)
        .ok_or_else(|| ApiError::bad("Metadata source request not found"))?;
    if source != &input.request {
        return Err(ApiError::conflict(
            "Save the Dubbo source request before importing metadata",
        ));
    }
    moleapi_core::validate_request(source, false)
        .map_err(|error| ApiError::bad(error.to_string()))?;
    let Protocol::Dubbo { config } = &source.protocol else {
        return Err(ApiError::bad("Metadata import requires a Dubbo request"));
    };
    let source = source.clone();
    let config = config.as_ref().clone();
    let policy = NetworkPolicy {
        allow_private_network: state.local || state.config.allow_private_network,
    };
    let preview = read_metadata(
        &config,
        policy,
        Duration::from_millis(source.timeout_ms.min(15_000)),
    )
    .await
    .map_err(|error| ApiError::bad(error.to_string()))?;
    if preview.document_sha256 != input.document_sha256 {
        return Err(ApiError::conflict(
            "Metadata definition changed after preview",
        ));
    }
    if input.decisions.len() != preview.methods.len() {
        return Err(ApiError::bad("Select a decision for every metadata method"));
    }
    let mut choices = HashMap::new();
    for choice in input.decisions {
        if choices.insert(choice.signature.clone(), choice).is_some() {
            return Err(ApiError::bad("Duplicate metadata import decision"));
        }
    }
    let mut signatures = HashSet::new();
    let specification_id = Uuid::new_v4().to_string();
    let collection = w
        .data
        .collections
        .iter_mut()
        .find(|row| row.id == input.collection_id)
        .ok_or_else(|| ApiError::bad("Metadata target collection not found"))?;
    let mut changed = false;
    for method in &preview.methods {
        let operation = signature(method);
        if !signatures.insert(operation.clone()) {
            return Err(ApiError::bad(
                "Metadata contains duplicate method signatures",
            ));
        }
        let choice = choices
            .remove(&operation)
            .ok_or_else(|| ApiError::bad("Missing metadata method decision"))?;
        let matches = collection
            .requests
            .iter()
            .filter(|row| matching_request(row, &config, method))
            .map(|row| row.id.clone())
            .collect::<Vec<_>>();
        if matches!(choice.action, ImportAction::Skip) {
            if choice.replace_request_id.is_some() {
                return Err(ApiError::bad("Skip cannot target an existing request"));
            }
            continue;
        }
        let target_id = match choice.action {
            ImportAction::Add => {
                if choice.replace_request_id.is_some() {
                    return Err(ApiError::bad("Add cannot target an existing request"));
                }
                Uuid::new_v4().to_string()
            }
            ImportAction::Replace => {
                let id = choice
                    .replace_request_id
                    .ok_or_else(|| ApiError::bad("Choose the request to replace"))?;
                if !matches.contains(&id) {
                    return Err(ApiError::bad("Replacement does not match metadata method"));
                }
                id
            }
            ImportAction::Skip => unreachable!(),
        };
        let mut generated = source.clone();
        generated.id = target_id.clone();
        generated.name = operation.clone();
        generated.description = format!("Dubbo provider metadata: {}", preview.application);
        generated.pre_request_script.clear();
        generated.post_response_script.clear();
        generated.assertions.clear();
        generated.extractions.clear();
        generated.examples.clear();
        generated.specification_id = Some(specification_id.clone());
        generated.operation_id = Some(operation);
        if let Protocol::Dubbo { config } = &mut generated.protocol {
            config.method = method.name.clone();
            config.arguments = method
                .parameter_types
                .iter()
                .map(|name| -> Result<DubboArgument, ApiError> {
                    Ok(DubboArgument {
                        id: Uuid::new_v4().to_string(),
                        type_name: name.clone(),
                        value_source: argument_draft(name, &preview.types)?,
                        secret: false,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            config.attachments.clear();
            if target_id != source.id {
                config.metadata_auth = None;
            }
        }
        moleapi_core::validate_request(&generated, false)
            .map_err(|error| ApiError::bad(error.to_string()))?;
        if matches!(choice.action, ImportAction::Replace) {
            let row = collection
                .requests
                .iter_mut()
                .find(|row| row.id == target_id)
                .ok_or_else(|| ApiError::bad("Replacement request disappeared"))?;
            *row = generated;
        } else {
            collection.requests.push(generated);
        }
        changed = true;
    }
    if !choices.is_empty() {
        return Err(ApiError::bad("Unknown metadata method decision"));
    }
    if !changed {
        return Ok(Json(w));
    }
    let metadata_config_group =
        if url::Url::parse(&config.metadata_center_url).is_ok_and(|url| url.scheme() == "nacos") {
            if config.metadata_group.is_empty() {
                "dubbo"
            } else {
                &config.metadata_group
            }
        } else {
            ""
        };
    w.data.specifications.push(Specification {
        id: specification_id,
        name: format!("Dubbo {} ({})", preview.service, preview.application),
        kind: "dubbo-metadata".into(),
        dialect: "dubbo-3.3.6".into(),
        source: json!({"format":"moleapi-dubbo-metadata-source-v1",
            "url":config.metadata_center_url,"application":preview.application,
            "namespace":config.metadata_namespace,
            "config_group":metadata_config_group,
            "path":preview.metadata_path,"raw":preview.raw_document,
            "document_sha256":preview.document_sha256,
            "source_request_id":source.id,
            "workspace_revision_at_import":input.expected_revision})
        .to_string(),
    });
    moleapi_core::validate_workspace(&w.data).map_err(|error| ApiError::bad(error.to_string()))?;
    if !state.local {
        moleapi_core::scrub_local_values(&mut w.data);
    }
    let w = storage::replace(&state.db, &owner.0, w, input.expected_revision)
        .await?
        .ok_or_else(|| ApiError::conflict("Workspace revision changed"))?;
    reconcile_sessions(&state, &owner.0, &w).await;
    Ok(Json(w))
}

#[cfg(test)]
mod tests {
    use super::argument_draft;
    use moleapi_protocols::dubbo::{DubboMetadataProperty, DubboMetadataType};

    #[test]
    fn metadata_dto_fields_become_editable_typed_argument() {
        let types = vec![DubboMetadataType {
            type_name: "fixture.Profile".into(),
            properties: vec![
                DubboMetadataProperty {
                    name: "name".into(),
                    type_name: "java.lang.String".into(),
                },
                DubboMetadataProperty {
                    name: "age".into(),
                    type_name: "int".into(),
                },
            ],
            items: vec![],
            enums: vec![],
        }];
        let draft: serde_json::Value =
            serde_json::from_str(&argument_draft("fixture.Profile", &types).unwrap()).unwrap();
        assert_eq!(draft["$class"], "fixture.Profile");
        assert!(draft["$"]["name"].is_null());
        assert!(draft["$"]["age"].is_null());
        assert_eq!(argument_draft("java.lang.String", &types).unwrap(), "null");
    }

    #[test]
    fn metadata_nested_dto_collection_enum_and_cycle_have_editable_drafts() {
        let types = vec![
            DubboMetadataType {
                type_name: "fixture.Profile".into(),
                properties: [
                    ("address", "fixture.Address"),
                    ("tier", "fixture.Tier"),
                    ("tags", "java.util.List<java.lang.String>"),
                    (
                        "addresses",
                        "java.util.Map<java.lang.String,fixture.Address>",
                    ),
                    ("parent", "fixture.Profile"),
                ]
                .into_iter()
                .map(|(name, type_name)| DubboMetadataProperty {
                    name: name.into(),
                    type_name: type_name.into(),
                })
                .collect(),
                items: vec![],
                enums: vec![],
            },
            DubboMetadataType {
                type_name: "fixture.Address".into(),
                properties: vec![DubboMetadataProperty {
                    name: "street".into(),
                    type_name: "java.lang.String".into(),
                }],
                items: vec![],
                enums: vec![],
            },
            DubboMetadataType {
                type_name: "fixture.Tier".into(),
                properties: vec![],
                items: vec![],
                enums: vec!["FREE".into(), "PAID".into()],
            },
        ];
        let draft: serde_json::Value =
            serde_json::from_str(&argument_draft("fixture.Profile", &types).unwrap()).unwrap();
        assert_eq!(draft["$"]["address"]["$class"], "fixture.Address");
        assert!(draft["$"]["address"]["$"]["street"].is_null());
        assert!(draft["$"]["tier"].is_null());
        assert_eq!(draft["$"]["tags"], serde_json::json!([]));
        assert_eq!(draft["$"]["addresses"], serde_json::json!({}));
        assert!(draft["$"]["parent"].is_null());
    }
}
