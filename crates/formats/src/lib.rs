mod curl;
mod openapi;
mod postman;
mod redact;

use anyhow::{Context, Result, bail};
use moleapi_core::{Workspace, WorkspaceData, validate_workspace};
use serde::Serialize;

#[derive(Serialize)]
pub struct ImportResult {
    pub name: String,
    pub data: WorkspaceData,
    pub warnings: Vec<String>,
}
#[derive(Serialize)]
pub struct ExportResult {
    pub filename: String,
    pub content: String,
    pub mime: String,
}

pub fn import(format: &str, content: &str) -> Result<ImportResult> {
    if content.len() > 20 * 1024 * 1024 {
        bail!("导入文件超过 20 MiB 限制");
    }
    let result = match format {
        "openapi" => openapi::import(content)?,
        "postman" => postman::import(content)?,
        "curl" => curl::import(content)?,
        "moleapi" => {
            let value: serde_json::Value =
                serde_json::from_str(content).context("无效 MoleAPI JSON")?;
            let name = value
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("导入工作区")
                .to_owned();
            let data = serde_json::from_value(value.get("data").unwrap_or(&value).clone())
                .context("无效工作区结构")?;
            ImportResult {
                name,
                data,
                warnings: vec![],
            }
        }
        _ => bail!("不支持的导入格式"),
    };
    validate_workspace(&result.data)?;
    Ok(result)
}

pub fn export(workspace: &Workspace, format: &str, include_secrets: bool) -> Result<ExportResult> {
    for request in workspace.data.collections.iter().flat_map(|c| &c.requests) {
        moleapi_core::validate_structured_body(request, true)?;
    }
    if matches!(format, "postman" | "openapi")
        && workspace
            .data
            .collections
            .iter()
            .flat_map(|c| &c.requests)
            .any(|r| r.body_kind.contains("{{"))
    {
        bail!("Dynamic body modes require MoleAPI format; external formats cannot preserve them");
    }
    let unsupported = workspace
        .data
        .collections
        .iter()
        .flat_map(|collection| &collection.requests)
        .any(|request| match &request.protocol {
            moleapi_core::Protocol::Http => {
                matches!(request.body_kind.as_str(), "binary" | "multipart") && format != "postman"
            }
            moleapi_core::Protocol::Graphql {
                document,
                variables,
                variables_source,
                operation_name,
                connection_params,
                subscription_url,
            } => {
                if format != "postman" {
                    return true;
                }
                let actual = match variables_source {
                    Some(source) => serde_json::from_str(source)
                        .ok()
                        .filter(serde_json::Value::is_object),
                    None => Some((**variables).clone()),
                };
                let selected = actual.and_then(|variables| {
                    moleapi_core::GraphqlPayload {
                        query: document.clone(),
                        variables,
                        operation_name: operation_name.clone(),
                    }
                    .selected_operation()
                    .ok()
                });
                operation_name.is_some()
                    || !connection_params
                        .as_object()
                        .is_some_and(|map| map.is_empty())
                    || subscription_url.is_some()
                    || !matches!(
                        selected,
                        Some(
                            async_graphql_parser::types::OperationType::Query
                                | async_graphql_parser::types::OperationType::Mutation
                        )
                    )
            }
            _ => true,
        });
    let unsupported_specification = workspace
        .data
        .specifications
        .iter()
        .any(|spec| spec.kind == "protobuf");
    if matches!(format, "postman" | "openapi") && (unsupported || unsupported_specification) {
        bail!("该导出格式尚不能保留当前专用协议或服务定义配置，请使用 MoleAPI 格式导出");
    }
    if format == "postman"
        && include_secrets
        && workspace
            .data
            .collections
            .iter()
            .flat_map(|c| &c.requests)
            .any(|r| matches!(r.body_kind.as_str(), "binary" | "multipart"))
    {
        bail!(
            "Postman cannot embed selected file bytes; use MoleAPI for a complete file-body backup"
        );
    }
    let workspace = if include_secrets {
        workspace.clone()
    } else {
        redact::workspace(workspace)
    };
    let (suffix, content) = match format {
        "moleapi" => ("moleapi.json", serde_json::to_string_pretty(&workspace)?),
        "postman" => ("postman_collection.json", postman::export(&workspace)?),
        "openapi" => ("openapi.json", openapi::export(&workspace)?),
        _ => bail!("不支持的导出格式"),
    };
    let filename = workspace
        .name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    Ok(ExportResult {
        filename: format!(
            "{}.{suffix}",
            if filename.trim_matches('_').is_empty() {
                "workspace"
            } else {
                &filename
            }
        ),
        content,
        mime: "application/json".into(),
    })
}

pub(crate) fn uid() -> String {
    uuid::Uuid::new_v4().to_string()
}
pub(crate) fn pair(key: impl Into<String>, value: impl Into<String>) -> moleapi_core::Pair {
    moleapi_core::Pair {
        id: uid(),
        key: key.into(),
        value: value.into(),
        enabled: true,
        secret: None,
        local_value: None,
    }
}
pub(crate) fn request(
    name: String,
    method: String,
    url: String,
) -> Result<moleapi_core::RequestSpec> {
    Ok(serde_json::from_value(
        serde_json::json!({"id":uid(),"name":name,"method":method,"url":url,"description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":30000,"follow_redirects":true,"verify_tls":true,"assertions":[],"examples":[]}),
    )?)
}
pub(crate) fn data(
    collections: Vec<moleapi_core::Collection>,
    variables: Vec<moleapi_core::Pair>,
) -> WorkspaceData {
    let environment_id = uid();
    WorkspaceData {
        global_variables: vec![],
        pre_request_script: String::new(),
        post_response_script: String::new(),
        schema_version: 1,
        collections,
        environments: vec![moleapi_core::Environment {
            id: environment_id.clone(),
            name: "导入环境".into(),
            variables,
        }],
        active_environment_id: Some(environment_id),
        specifications: vec![],
    }
}

/// Screen a saved HTTP request for generated previews without resolving variables.
pub fn generation_request(
    workspace: &Workspace,
    request_id: &str,
    include_secrets: bool,
) -> Result<moleapi_core::RequestSpec> {
    redact::generation_request(workspace, request_id, include_secrets)
}
