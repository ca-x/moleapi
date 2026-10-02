use crate::{ImportResult, data, pair, request, uid};
use anyhow::{Context, Result, bail};
use moleapi_core::{Collection, Specification, Workspace};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub(super) fn import(content: &str) -> Result<ImportResult> {
    let document: Value = serde_yaml_ng::from_str(content).context("OpenAPI JSON/YAML 语法无效")?;
    let version = document["openapi"]
        .as_str()
        .context("缺少 openapi 版本字段；请导入 OpenAPI 3.0/3.1")?;
    if version.starts_with("3.0.") {
        let _: openapiv3::OpenAPI =
            serde_json::from_value(document.clone()).context("OpenAPI 3.0 结构无效")?;
    } else if version.starts_with("3.1.") {
        let spec: oas3::Spec =
            serde_json::from_value(document.clone()).context("OpenAPI 3.1 结构无效")?;
        spec.validate_version().context("OpenAPI 版本无效")?;
    } else {
        bail!("不支持 OpenAPI 版本 {version}");
    }
    let name = document["info"]["title"]
        .as_str()
        .unwrap_or("OpenAPI 导入")
        .to_string();
    let spec_id = uid();
    let mut variables = vec![];
    let mut warnings = vec![];
    let server = document["servers"]
        .as_array()
        .and_then(|v| v.first())
        .map(|v| server_url(v, &mut variables))
        .unwrap_or_else(|| "https://api.example.com".into());
    variables.push(pair("base_url", &server));
    let mut groups: BTreeMap<String, Vec<moleapi_core::RequestSpec>> = BTreeMap::new();
    let paths = document["paths"]
        .as_object()
        .context("OpenAPI 缺少 paths 对象")?;
    for (path, raw_item) in paths {
        let item = resolve(&document, raw_item, &mut warnings)?;
        for method in ["get", "post", "put", "patch", "delete", "head", "options"] {
            let Some(operation) = item.get(method) else {
                continue;
            };
            let title = operation["summary"]
                .as_str()
                .or_else(|| operation["operationId"].as_str())
                .unwrap_or(path);
            let mut endpoint = request(
                title.into(),
                method.to_uppercase(),
                format!("{{{{base_url}}}}{path}"),
            )?;
            endpoint.specification_id = Some(spec_id.clone());
            endpoint.operation_id = operation["operationId"].as_str().map(str::to_string);
            endpoint.description = operation["description"].as_str().unwrap_or("").into();
            let mut parameters = vec![];
            if let Some(values) = item["parameters"].as_array() {
                parameters.extend(values.iter());
            }
            if let Some(values) = operation["parameters"].as_array() {
                parameters.extend(values.iter());
            }
            for raw in parameters {
                let parameter = resolve(&document, raw, &mut warnings)?;
                let key = parameter["name"].as_str().unwrap_or("");
                let example = parameter
                    .get("example")
                    .or_else(|| parameter["schema"].get("example"))
                    .or_else(|| parameter["schema"].get("default"));
                let value = example.map(string_value).unwrap_or_default();
                match parameter["in"].as_str().unwrap_or("") {
                    "query" => endpoint.query.push(pair(key, value)),
                    "header" => endpoint.headers.push(pair(key, value)),
                    "path" => {
                        endpoint.url = endpoint
                            .url
                            .replace(&format!("{{{key}}}"), &format!("{{{{{key}}}}}"));
                        if !variables.iter().any(|v| v.key == key) {
                            variables.push(pair(key, value));
                        }
                    }
                    "cookie" => warnings.push(format!(
                        "{method} {path}: Cookie 参数保留在原始规范中，运行前配置 Cookie"
                    )),
                    _ => {}
                }
            }
            if let Some(raw) = operation.get("requestBody") {
                let body = resolve(&document, raw, &mut warnings)?;
                if let Some(media) = body["content"].as_object() {
                    let entry = media
                        .iter()
                        .find(|(key, _)| key.as_str() == "application/json")
                        .or_else(|| media.iter().next());
                    if let Some((mime, value)) = entry {
                        endpoint.headers.push(pair("Content-Type", mime));
                        let example = value.get("example").or_else(|| {
                            value["examples"]
                                .as_object()
                                .and_then(|values| values.values().next())
                                .and_then(|v| v.get("value"))
                        });
                        endpoint.body_kind = if mime.contains("json") {
                            "json"
                        } else if mime == "application/x-www-form-urlencoded" {
                            "form"
                        } else {
                            "text"
                        }
                        .into();
                        endpoint.body = example
                            .map(|v| {
                                if endpoint.body_kind == "json" {
                                    serde_json::to_string_pretty(v).unwrap_or_default()
                                } else {
                                    string_value(v)
                                }
                            })
                            .unwrap_or_else(|| {
                                if endpoint.body_kind == "json" {
                                    "{}".into()
                                } else {
                                    String::new()
                                }
                            });
                        if example.is_none() {
                            warnings.push(format!(
                                "{method} {path}: 无请求示例，Body 需按原始 Schema 填写"
                            ));
                        }
                    }
                }
            }
            if operation
                .get("security")
                .or_else(|| document.get("security"))
                .is_some()
            {
                warnings.push(format!(
                    "{method} {path}: 鉴权定义已保留，请在请求中配置凭据"
                ));
            }
            if let Some(responses) = operation["responses"].as_object() {
                for (status, raw) in responses {
                    let Ok(status) = status.parse::<u16>() else {
                        continue;
                    };
                    let response = resolve(&document, raw, &mut warnings)?;
                    if let Some(media) = response["content"].as_object() {
                        for (mime, value) in media {
                            if let Some(example) = value.get("example") {
                                endpoint.examples.push(moleapi_core::Example {
                                    id: uid(),
                                    name: response["description"]
                                        .as_str()
                                        .unwrap_or("导入示例")
                                        .into(),
                                    status,
                                    headers: vec![pair("Content-Type", mime)],
                                    body: if mime.contains("json") {
                                        serde_json::to_string_pretty(example)?
                                    } else {
                                        string_value(example)
                                    },
                                });
                            }
                        }
                    }
                }
            }
            let tag = operation["tags"]
                .as_array()
                .and_then(|v| v.first())
                .and_then(Value::as_str)
                .unwrap_or("API");
            groups.entry(tag.into()).or_default().push(endpoint);
        }
    }
    let collections = groups
        .into_iter()
        .map(|(name, requests)| Collection {
            id: uid(),
            name,
            description: String::new(),
            requests,
        })
        .collect();
    let mut data = data(collections, variables);
    data.specifications.push(Specification {
        id: spec_id,
        name: name.clone(),
        kind: "openapi".into(),
        source: content.into(),
        dialect: version.into(),
    });
    warnings.sort();
    warnings.dedup();
    Ok(ImportResult {
        name,
        data,
        warnings,
    })
}
fn server_url(server: &Value, variables: &mut Vec<moleapi_core::Pair>) -> String {
    let mut result = server["url"]
        .as_str()
        .unwrap_or("https://api.example.com")
        .to_string();
    if let Some(values) = server["variables"].as_object() {
        for (key, value) in values {
            let default = string_value(&value["default"]);
            result = result.replace(&format!("{{{key}}}"), &default);
            variables.push(pair(key, default));
        }
    }
    result
}
fn resolve<'a>(
    document: &'a Value,
    value: &'a Value,
    warnings: &mut Vec<String>,
) -> Result<&'a Value> {
    let mut current = value;
    let mut seen = std::collections::HashSet::new();
    while let Some(reference) = current["$ref"].as_str() {
        if !seen.insert(reference) {
            bail!("循环引用 {reference}");
        }
        let Some(pointer) = reference.strip_prefix('#') else {
            warnings.push(format!("外部引用 {reference} 已保留，未访问外部网络"));
            return Ok(value);
        };
        current = document
            .pointer(pointer)
            .with_context(|| format!("引用不存在 {reference}"))?;
    }
    Ok(current)
}
fn string_value(value: &Value) -> String {
    value.as_str().map(str::to_string).unwrap_or_else(|| {
        if value.is_null() {
            String::new()
        } else {
            value.to_string()
        }
    })
}

pub(super) fn export(workspace: &Workspace) -> Result<String> {
    if let Some(source) = workspace
        .data
        .specifications
        .iter()
        .find(|s| s.kind == "openapi")
    {
        let mut value: Value = serde_yaml_ng::from_str(&source.source)?;
        // Canonical schemas/references/extensions remain unchanged. Requests are exported separately in MoleAPI JSON.
        value["info"]["title"] = json!(workspace.name);
        return Ok(serde_json::to_string_pretty(&value)?);
    }
    let mut paths = serde_json::Map::new();
    let mut servers = vec![];
    for collection in &workspace.data.collections {
        for request in &collection.requests {
            let path = if let Some(path) = request.url.strip_prefix("{{base_url}}") {
                path.to_string()
            } else {
                let url = url::Url::parse(&request.url).context("导出 OpenAPI 前须填写合法 URL")?;
                let origin = url.origin().ascii_serialization();
                if !servers.iter().any(|v: &Value| v["url"] == origin) {
                    servers.push(json!({"url":origin}));
                }
                url.path().to_string()
            };
            let path = if path.starts_with('/') {
                path
            } else {
                format!("/{path}")
            };
            let entry = paths.entry(path).or_insert_with(|| json!({}));
            let parameters=request.query.iter().filter(|p|p.enabled).map(|p|json!({"name":p.key,"in":"query","schema":{"type":"string"},"example":p.value})).chain(request.headers.iter().filter(|p|p.enabled).map(|p|json!({"name":p.key,"in":"header","schema":{"type":"string"},"example":p.value}))).collect::<Vec<_>>();
            let mut operation = json!({"summary":request.name,"description":request.description,"tags":[collection.name],"parameters":parameters,"responses":{"default":{"description":"响应"}}});
            if request.body_kind != "none" {
                let mime = if request.body_kind == "json" {
                    "application/json"
                } else if request.body_kind == "form" {
                    "application/x-www-form-urlencoded"
                } else {
                    "text/plain"
                };
                let example = if request.body_kind == "json" {
                    serde_json::from_str::<Value>(&request.body).unwrap_or(json!(request.body))
                } else {
                    json!(request.body)
                };
                operation["requestBody"] = json!({"content":{mime:{"example":example}}});
            }
            entry[request.method.to_lowercase()] = operation;
        }
    }
    if servers.is_empty()
        && let Some(environment) = workspace
            .data
            .environments
            .iter()
            .find(|e| Some(&e.id) == workspace.data.active_environment_id.as_ref())
        && let Some(base) = environment
            .variables
            .iter()
            .find(|p| p.key == "base_url" && p.enabled)
    {
        servers.push(json!({"url":base.value}));
    }
    let document = json!({"openapi":"3.1.0","info":{"title":workspace.name,"version":"1.0.0"},"servers":servers,"paths":paths});
    let _: oas3::Spec = serde_json::from_value(document.clone())?;
    Ok(serde_json::to_string_pretty(&document)?)
}
