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
    if content.len() > 5 * 1024 * 1024 {
        bail!("导入文件超过 5 MiB 限制");
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
    if format != "moleapi"
        && workspace
            .data
            .collections
            .iter()
            .flat_map(|collection| &collection.requests)
            .any(|request| request.protocol != moleapi_core::Protocol::Http)
    {
        bail!("该导出格式尚不能保留 SSE/WebSocket 会话配置，请使用 MoleAPI 格式导出");
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
