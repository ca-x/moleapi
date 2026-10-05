use crate::{ImportResult, data, pair, request, uid};
use anyhow::{Context, Result, bail};
use moleapi_core::{Collection, Specification, Workspace};
use serde_json::{Value, json};

pub(super) fn import(content: &str) -> Result<ImportResult> {
    let source: Value = serde_json::from_str(content).context("Postman Collection JSON 无效")?;
    let schema = source["info"]["schema"]
        .as_str()
        .context("缺少 Postman info.schema")?;
    if !schema.contains("v2.1.0") {
        bail!("此入口接受 Postman 2.1 JSON；其他版本请使用对应格式入口");
    }
    let official: Value = serde_json::from_str(include_str!("../schemas/postman21.json"))
        .context("内置官方 Collection schema 无效")?;
    let validator = jsonschema::validator_for(&official).context("无法加载官方 Postman schema")?;
    if let Err(error) = validator.validate(&source) {
        bail!("Postman Collection 结构无效: {error}");
    }
    let name = source["info"]["name"]
        .as_str()
        .unwrap_or("Postman 导入")
        .to_string();
    let mut collections = vec![];
    let mut warnings = vec![];
    let spec_id = uid();
    let ancestors = source.get("auth").cloned().into_iter().collect::<Vec<_>>();
    walk(
        &source["item"],
        &name,
        &mut collections,
        &mut warnings,
        &spec_id,
        &ancestors,
    )?;
    let variables: Vec<moleapi_core::Pair> = source["variable"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|value| {
            let mut result = pair(value["key"].as_str().unwrap_or(""), text(&value["value"]));
            result.enabled = !value["disabled"].as_bool().unwrap_or(false);
            result
        })
        .collect();
    for collection in &mut collections {
        collection.variables = variables.clone();
        collection.pre_request_script = event_script(&source, "prerequest");
        collection.post_response_script = event_script(&source, "test");
    }
    let mut data = data(collections, vec![]);
    data.specifications.push(Specification {
        id: spec_id,
        name: name.clone(),
        kind: "postman".into(),
        source: content.into(),
        dialect: "2.1.0".into(),
    });
    Ok(ImportResult {
        name,
        data,
        warnings,
    })
}
fn walk(
    items: &Value,
    path: &str,
    collections: &mut Vec<Collection>,
    warnings: &mut Vec<String>,
    spec_id: &str,
    ancestors: &[Value],
) -> Result<()> {
    let mut requests = vec![];
    for (index, item) in items.as_array().into_iter().flatten().enumerate() {
        let name = item["name"].as_str().unwrap_or("请求");
        if item.get("item").is_some() {
            let mut parents = ancestors.to_vec();
            if let Some(auth) = item.get("auth") {
                parents.push(auth.clone());
            }
            walk(
                &item["item"],
                &format!("{path} / {name}"),
                collections,
                warnings,
                spec_id,
                &parents,
            )?;
            continue;
        }
        let raw = &item["request"];
        let request_url = if raw.is_string() {
            raw.as_str().unwrap_or("").to_string()
        } else if raw["url"].is_string() {
            raw["url"].as_str().unwrap_or("").to_string()
        } else {
            raw["url"]["raw"].as_str().unwrap_or("").to_string()
        };
        let mut result = request(
            name.into(),
            raw["method"].as_str().unwrap_or("GET").into(),
            request_url,
        )?;
        result.specification_id = Some(spec_id.into());
        result.operation_id = Some(format!("{path}/{index}"));
        result.description = text(&raw["description"]);
        if let Some(headers) = raw["header"].as_array() {
            for header in headers {
                let mut h = pair(header["key"].as_str().unwrap_or(""), text(&header["value"]));
                h.enabled = !header["disabled"].as_bool().unwrap_or(false);
                result.headers.push(h);
            }
        }
        let mode = raw["body"]["mode"].as_str().unwrap_or("");
        match mode {
            "graphql" => {
                let gql = &raw["body"]["graphql"];
                let raw_variables = gql.get("variables").cloned().unwrap_or_else(|| json!({}));
                let source = if let Some(value) = raw_variables.as_str() {
                    value.to_owned()
                } else {
                    raw_variables.to_string()
                };
                let variables = serde_json::from_str::<Value>(&source)
                    .ok()
                    .filter(Value::is_object)
                    .unwrap_or_else(|| json!({}));
                result.protocol = serde_json::from_value(
                    json!({"kind":"graphql","document":gql["query"].as_str().unwrap_or(""),"variables":variables,"variables_source":source,"connection_params":{}}),
                )?;
            }
            "raw" => {
                result.body = text(&raw["body"]["raw"]);
                result.body_kind = if raw["body"]["options"]["raw"]["language"] == "json"
                    || result.headers.iter().any(|h| {
                        h.key.eq_ignore_ascii_case("content-type") && h.value.contains("json")
                    }) {
                    "json"
                } else {
                    "text"
                }
                .into();
            }
            "urlencoded" => {
                let mut serializer = url::form_urlencoded::Serializer::new(String::new());
                for p in raw["body"]["urlencoded"].as_array().into_iter().flatten() {
                    if !p["disabled"].as_bool().unwrap_or(false) {
                        serializer.append_pair(p["key"].as_str().unwrap_or(""), &text(&p["value"]));
                    }
                }
                result.body_kind = "form".into();
                result.body = serializer.finish();
            }
            "file" => {
                let src = raw["body"]["file"]["src"].as_str().unwrap_or("");
                result.body_kind = "binary".into();
                result.body = serde_json::to_string(&moleapi_core::BinaryBody {
                    file_name: src.rsplit(['/', '\\']).next().unwrap_or("").into(),
                    mime: result
                        .headers
                        .iter()
                        .find(|h| h.enabled && h.key.eq_ignore_ascii_case("content-type"))
                        .map(|h| h.value.clone())
                        .unwrap_or_default(),
                    base64: None,
                })?;
                warnings.push(format!(
                    "{name}: select the original binary file again; imported paths are never read"
                ));
            }
            "formdata" => {
                let mut parts = Vec::new();
                for entry in raw["body"]["formdata"].as_array().into_iter().flatten() {
                    let value = if entry["type"] == "file" {
                        let mut paths: Vec<String> = match &entry["src"] {
                            Value::Array(values) => values
                                .iter()
                                .filter_map(Value::as_str)
                                .map(str::to_owned)
                                .collect(),
                            Value::String(value) => vec![value.clone()],
                            _ => vec![String::new()],
                        };
                        if paths.is_empty() {
                            paths.push(String::new());
                        }
                        for path in paths {
                            parts.push(moleapi_core::MultipartPart {
                                id: uid(),
                                name: text(&entry["key"]),
                                enabled: !entry["disabled"].as_bool().unwrap_or(false),
                                value: moleapi_core::MultipartValue::File {
                                    file: moleapi_core::BinaryBody {
                                        file_name: path
                                            .rsplit(['/', '\\'])
                                            .next()
                                            .unwrap_or("")
                                            .into(),
                                        mime: text(&entry["contentType"]),
                                        base64: None,
                                    },
                                },
                            });
                        }
                        warnings.push(format!(
                            "{name}: select multipart files again; imported paths are never read"
                        ));
                        continue;
                    } else {
                        moleapi_core::MultipartValue::Text {
                            text: text(&entry["value"]),
                            mime: text(&entry["contentType"]),
                        }
                    };
                    parts.push(moleapi_core::MultipartPart {
                        id: uid(),
                        name: text(&entry["key"]),
                        enabled: !entry["disabled"].as_bool().unwrap_or(false),
                        value,
                    });
                }
                result.body_kind = "multipart".into();
                result.body = serde_json::to_string(&moleapi_core::MultipartBody { parts })?;
                for header in &mut result.headers {
                    if header.key.eq_ignore_ascii_case("content-type") {
                        header.enabled = false;
                    }
                }
            }
            "" => {}
            other => warnings.push(format!(
                "{name}: {other} Body 已保留在原始集合，需要对应协议编辑器"
            )),
        }
        let auth = raw.get("auth").or_else(|| ancestors.last());
        if let Some(auth) = auth {
            match auth["type"].as_str().unwrap_or("") {
                "bearer" => {
                    result.auth.kind = "bearer".into();
                    result.auth.token = auth_value(auth, "bearer", "token");
                }
                "basic" => {
                    result.auth.kind = "basic".into();
                    result.auth.username = auth_value(auth, "basic", "username");
                    result.auth.password = auth_value(auth, "basic", "password");
                }
                "apikey" => {
                    let key = auth_value(auth, "apikey", "key");
                    let value = auth_value(auth, "apikey", "value");
                    if auth_value(auth, "apikey", "in") == "query" {
                        let separator = if result.url.contains('?') { '&' } else { '?' };
                        result.url = format!(
                            "{}{separator}{}",
                            result.url,
                            url::form_urlencoded::Serializer::new(String::new())
                                .append_pair(&key, &value)
                                .finish()
                        );
                    } else {
                        result.headers.push(pair(key, value));
                    }
                }
                "noauth" | "" => {}
                other => warnings.push(format!("{name}: {other} 鉴权定义已保留，需要对应鉴权配置")),
            }
        }
        result.pre_request_script = event_script(item, "prerequest");
        result.post_response_script = event_script(item, "test");
        for response in item["response"].as_array().into_iter().flatten() {
            if let Some(status) = response["code"].as_u64().filter(|s| *s >= 100 && *s <= 599) {
                let headers = response["header"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|h| pair(h["key"].as_str().unwrap_or(""), text(&h["value"])))
                    .collect();
                result.examples.push(moleapi_core::Example {
                    id: uid(),
                    name: response["name"].as_str().unwrap_or("示例").into(),
                    status: status as u16,
                    headers,
                    body: text(&response["body"]),
                });
            }
        }
        requests.push(result);
    }
    if !requests.is_empty() {
        collections.push(Collection {
            variables: vec![],
            pre_request_script: String::new(),
            post_response_script: String::new(),
            id: uid(),
            name: path.into(),
            description: String::new(),
            requests,
        });
    }
    Ok(())
}
fn event_script(item: &Value, listen: &str) -> String {
    item["event"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|event| event["listen"] == listen)
        .flat_map(|event| {
            let script = &event["script"]["exec"];
            if let Some(lines) = script.as_array() {
                lines
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            } else {
                script.as_str().map(str::to_owned).into_iter().collect()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
fn events(pre: &str, post: &str) -> Vec<Value> {
    [("prerequest",pre),("test",post)].into_iter().filter(|(_,script)|!script.is_empty()).map(|(listen,script)|json!({"listen":listen,"script":{"type":"text/javascript","exec":script.lines().collect::<Vec<_>>()}})).collect()
}
fn text(value: &Value) -> String {
    value.as_str().map(str::to_string).unwrap_or_else(|| {
        if value.is_null() {
            String::new()
        } else {
            value.to_string()
        }
    })
}
fn auth_value(auth: &Value, kind: &str, key: &str) -> String {
    auth[kind]
        .as_array()
        .into_iter()
        .flatten()
        .find(|p| p["key"] == key)
        .map(|p| text(&p["value"]))
        .unwrap_or_default()
}

pub(super) fn export(workspace: &Workspace) -> Result<String> {
    let items=workspace.data.collections.iter().map(|collection|json!({"name":collection.name,"description":collection.description,"event":events(&collection.pre_request_script,&collection.post_response_script),"variable":collection.variables.iter().map(|v|json!({"key":v.key,"value":v.value,"type":"string","disabled":!v.enabled})).collect::<Vec<_>>(),"item":collection.requests.iter().map(|r| {
        let mut url=r.url.clone();if !r.query.is_empty(){let values=r.query.iter().filter(|p|p.enabled).map(|p|format!("{}={}",url::form_urlencoded::byte_serialize(p.key.as_bytes()).collect::<String>(),url::form_urlencoded::byte_serialize(p.value.as_bytes()).collect::<String>())).collect::<Vec<_>>().join("&");if !values.is_empty(){url.push(if url.contains('?'){'&'}else{'?'});url.push_str(&values);}}
        let auth=match r.auth.kind.as_str(){"bearer"=>json!({"type":"bearer","bearer":[{"key":"token","value":r.auth.token,"type":"string"}]}),"basic"=>json!({"type":"basic","basic":[{"key":"username","value":r.auth.username,"type":"string"},{"key":"password","value":r.auth.password,"type":"string"}]}),_=>json!({"type":"noauth"})};
        let mut request=json!({"method":r.method,"url":url,"description":r.description,"header":r.headers.iter().map(|h|json!({"key":h.key,"value":h.value,"disabled":!h.enabled})).collect::<Vec<_>>(),"auth":auth});
        if let moleapi_core::Protocol::Graphql{document,variables,variables_source,..}=&r.protocol {
            request["body"]=json!({"mode":"graphql","graphql":{"query":document,"variables":variables_source.as_deref().map(str::to_owned).unwrap_or_else(||variables.to_string())}});
        } else if r.body_kind=="binary" {
            let file:moleapi_core::BinaryBody=serde_json::from_str(&r.body).expect("validated binary source");
            request["body"]=json!({"mode":"file","file":{"src":file.file_name}});
            if !file.mime.is_empty() && !r.headers.iter().any(|h|h.enabled && h.key.eq_ignore_ascii_case("content-type")) {
                request["header"].as_array_mut().unwrap().push(json!({"key":"Content-Type","value":file.mime,"disabled":false}));
            }
        } else if r.body_kind=="multipart" {
            let body:moleapi_core::MultipartBody=serde_json::from_str(&r.body).expect("validated multipart source");
            request["body"]=json!({"mode":"formdata","formdata":body.parts.iter().map(|p| match &p.value {
                moleapi_core::MultipartValue::Text{text,mime}=>json!({"key":p.name,"type":"text","value":text,"contentType":mime,"disabled":!p.enabled}),
                moleapi_core::MultipartValue::File{file}=>json!({"key":p.name,"type":"file","src":file.file_name,"contentType":file.mime,"disabled":!p.enabled}),
            }).collect::<Vec<_>>()});
        } else if r.body_kind=="form" {
            request["body"]=json!({"mode":"urlencoded","urlencoded":url::form_urlencoded::parse(r.body.as_bytes()).map(|(key,value)|json!({"key":key,"value":value,"type":"text"})).collect::<Vec<_>>()});
        } else if r.body_kind!="none"{request["body"]=json!({"mode":"raw","raw":r.body,"options":{"raw":{"language":if r.body_kind=="json"{"json"}else{"text"}}}});}
        json!({"name":r.name,"request":request,"event":events(&r.pre_request_script,&r.post_response_script),"response":r.examples.iter().map(|e|json!({"name":e.name,"code":e.status,"body":e.body,"header":e.headers.iter().map(|h|json!({"key":h.key,"value":h.value})).collect::<Vec<_>>(),"originalRequest":request})).collect::<Vec<_>>()})
    }).collect::<Vec<_>>() })).collect::<Vec<_>>();
    let variables = workspace
        .data
        .environments
        .iter()
        .find(|e| Some(&e.id) == workspace.data.active_environment_id.as_ref())
        .map(|e| {
            e.variables
                .iter()
                .map(|v| json!({"key":v.key,"value":v.value,"type":"string","disabled":!v.enabled}))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let result = json!({"info":{"name":workspace.name,"schema":"https://schema.getpostman.com/json/collection/v2.1.0/collection.json"},"item":items,"variable":variables});
    let schema: Value = serde_json::from_str(include_str!("../schemas/postman21.json"))?;
    jsonschema::validator_for(&schema)?
        .validate(&result)
        .map_err(|e| anyhow::anyhow!("导出结果不符合官方 Postman schema: {e}"))?;
    Ok(serde_json::to_string_pretty(&result)?)
}
