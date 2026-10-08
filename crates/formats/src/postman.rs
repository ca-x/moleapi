use crate::{ImportResult, data, pair, request, uid};
use anyhow::{Context, Result, bail, ensure};
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
    walk(
        &source,
        &name,
        &mut collections,
        &mut warnings,
        &spec_id,
        None,
        0,
    )?;
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
    container: &Value,
    path: &str,
    collections: &mut Vec<Collection>,
    warnings: &mut Vec<String>,
    spec_id: &str,
    parent_id: Option<&str>,
    depth: usize,
) -> Result<()> {
    anyhow::ensure!(
        depth < moleapi_core::MAX_COLLECTION_DEPTH,
        "Postman folder depth exceeds limit"
    );
    anyhow::ensure!(
        collections.len() < moleapi_core::MAX_COLLECTIONS,
        "Postman folder limit exceeded"
    );
    let collection_id = uid();
    let mut requests = vec![];
    for (index, item) in container["item"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let name = item["name"].as_str().unwrap_or("请求");
        if item.get("item").is_some() {
            walk(
                item,
                &format!("{path} / {name}"),
                collections,
                warnings,
                spec_id,
                Some(&collection_id),
                depth + 1,
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
        result.auth = import_auth(raw.get("auth"), warnings, name).unwrap_or_else(inherit_auth);
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
    anyhow::ensure!(
        collections.len() < moleapi_core::MAX_COLLECTIONS,
        "Postman collection limit exceeded"
    );
    if depth > 0
        && container["variable"]
            .as_array()
            .is_some_and(|v| !v.is_empty())
    {
        warnings.push(format!("{path}: 目录变量保留为源定义，Postman Runtime 尚不执行它们；可在 MoleAPI 目录设置中启用"));
    }
    let mut variables = import_variables(&container["variable"]);
    if depth > 0 {
        for variable in &mut variables {
            variable.secret = Some(true);
        }
    }
    collections.push(Collection {
        variables_enabled: if depth > 0 { Some(false) } else { None },
        parent_id: parent_id.map(str::to_owned),
        auth: import_auth(container.get("auth"), warnings, path),
        variables,
        pre_request_script: event_script(container, "prerequest"),
        post_response_script: event_script(container, "test"),
        id: collection_id,
        name: container["name"].as_str().unwrap_or(path).into(),
        description: text(&container["description"]),
        requests,
    });
    Ok(())
}
fn inherit_auth() -> moleapi_core::Auth {
    moleapi_core::Auth {
        kind: "inherit".into(),
        token: String::new(),
        username: String::new(),
        password: String::new(),
        api_key: None,
        jwt: None,
        oauth2: None,
        aws: None,
        hawk: None,
        oauth1: None,
    }
}
fn import_variables(value: &Value) -> Vec<moleapi_core::Pair> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .map(|value| {
            let mut result = pair(value["key"].as_str().unwrap_or(""), text(&value["value"]));
            result.enabled = !value["disabled"].as_bool().unwrap_or(false);
            result
        })
        .collect()
}
fn import_auth(
    value: Option<&Value>,
    warnings: &mut Vec<String>,
    name: &str,
) -> Option<moleapi_core::Auth> {
    let value = value.filter(|v| !v.is_null())?;
    let mut auth = inherit_auth();
    match value["type"].as_str().unwrap_or("") {
        "bearer" => {
            auth.kind = "bearer".into();
            auth.token = auth_value(value, "bearer", "token");
        }
        "basic" => {
            auth.kind = "basic".into();
            auth.username = auth_value(value, "basic", "username");
            auth.password = auth_value(value, "basic", "password");
        }
        "apikey" => {
            auth.kind = "apikey".into();
            auth.api_key = Some(Box::new(moleapi_core::ApiKeyAuth {
                name: auth_value(value, "apikey", "key"),
                value: auth_value(value, "apikey", "value"),
                location: if auth_value(value, "apikey", "in") == "query" {
                    moleapi_core::AuthLocation::Query
                } else {
                    moleapi_core::AuthLocation::Header
                },
            }));
        }
        "oauth1" => {
            auth.kind = "oauth1".into();
            let mut c = moleapi_core::OAuth1Auth {
                consumer_key: auth_value(value, "oauth1", "consumerKey"),
                consumer_secret: auth_value(value, "oauth1", "consumerSecret"),
                token: auth_value(value, "oauth1", "token"),
                token_secret: auth_value(value, "oauth1", "tokenSecret"),
                private_key: auth_value(value, "oauth1", "privateKey"),
                realm: auth_value(value, "oauth1", "realm"),
                nonce: auth_value(value, "oauth1", "nonce"),
                timestamp: auth_value(value, "oauth1", "timestamp"),
                callback: auth_value(value, "oauth1", "callback"),
                verifier: auth_value(value, "oauth1", "verifier"),
                include_version: auth_value(value, "oauth1", "version") == "1.0",
                include_body_hash: auth_value(value, "oauth1", "includeBodyHash") == "true",
                include_empty_params: auth_value(value, "oauth1", "addEmptyParamsToSign") == "true",
                location: if auth_value(value, "oauth1", "addParamsToHeader") == "true" {
                    moleapi_core::OAuth1Location::Header
                } else {
                    moleapi_core::OAuth1Location::Automatic
                },
                ..Default::default()
            };
            let algorithm = auth_value(value, "oauth1", "signatureMethod");
            if !algorithm.is_empty() {
                c.algorithm = algorithm;
            }
            if auth_value(value, "oauth1", "disableHeaderEncoding") == "true" {
                warnings.push(format!("{name}: OAuth1 disableHeaderEncoding is unsupported; authentication must be configured again"));
                auth.kind = "none".into();
            }
            let version = auth_value(value, "oauth1", "version");
            if [
                "includeBodyHash",
                "addEmptyParamsToSign",
                "addParamsToHeader",
                "disableHeaderEncoding",
            ]
            .iter()
            .any(|field| auth_value(value, "oauth1", field).contains("{{"))
                || !["", "1.0"].contains(&version.as_str())
            {
                warnings.push(format!("{name}: OAuth1 templated flags or non-1.0 version cannot be mapped; authentication must be configured again"));
                auth.kind = "none".into();
            }
            auth.oauth1 = Some(Box::new(c));
        }
        "hawk" => {
            auth.kind = "hawk".into();
            let mut hawk = moleapi_core::HawkAuth {
                id: auth_value(value, "hawk", "authId"),
                key: auth_value(value, "hawk", "authKey"),
                nonce: auth_value(value, "hawk", "nonce"),
                timestamp: auth_value(value, "hawk", "timestamp"),
                ext: auth_value(value, "hawk", "extraData"),
                app: auth_value(value, "hawk", "app"),
                delegation: auth_value(value, "hawk", "delegation"),
                user: auth_value(value, "hawk", "user"),
                include_payload_hash: auth_value(value, "hawk", "includePayloadHash") == "true",
                ..Default::default()
            };
            let algorithm = auth_value(value, "hawk", "algorithm");
            if !algorithm.is_empty() {
                hawk.algorithm = algorithm;
            }
            auth.hawk = Some(Box::new(hawk));
        }
        "awsv4" => {
            auth.kind = "aws".into();
            let mut aws = moleapi_core::AwsAuth {
                access_key: auth_value(value, "awsv4", "accessKey"),
                secret_key: auth_value(value, "awsv4", "secretKey"),
                session_token: auth_value(value, "awsv4", "sessionToken"),
                ..Default::default()
            };
            for (key, field) in [("region", &mut aws.region), ("service", &mut aws.service)] {
                let supplied = auth_value(value, "awsv4", key);
                if !supplied.is_empty() {
                    *field = supplied;
                }
            }
            if auth_value(value, "awsv4", "addAuthDataToQuery") == "true" {
                aws.location = moleapi_core::AuthLocation::Query;
                aws.unsigned_payload = aws.service == "s3";
                warnings.push(format!("{name}: AWS query signing imported with explicit900second expiry; review the native expiry setting"));
            }
            auth.aws = Some(Box::new(aws));
        }
        "noauth" => auth.kind = "none".into(),
        "" => return None,
        other => {
            auth.kind = "none".into();
            warnings.push(format!(
                "{name}: {other} 鉴权定义已保留，需要对应鉴权配置；未执行父级回退"
            ));
        }
    }
    Some(auth)
}
fn export_auth(auth: &moleapi_core::Auth) -> Result<Value> {
    Ok(match auth.kind.as_str() {
        "inherit" => Value::Null,
        "none" => json!({"type":"noauth"}),
        "bearer" => {
            json!({"type":"bearer","bearer":[{"key":"token","value":auth.token,"type":"string"}]})
        }
        "basic" => {
            json!({"type":"basic","basic":[{"key":"username","value":auth.username,"type":"string"},{"key":"password","value":auth.password,"type":"string"}]})
        }
        "apikey" => {
            let key = auth.api_key.as_ref().context("API key settings missing")?;
            json!({"type":"apikey","apikey":[{"key":"key","value":key.name,"type":"string"},{"key":"value","value":key.value,"type":"string"},{"key":"in","value":if key.location==moleapi_core::AuthLocation::Query{"query"}else{"header"},"type":"string"}]})
        }
        "oauth1" => {
            let c = auth.oauth1.as_ref().context("OAuth1 settings missing")?;
            ensure!(
                c.token_id.is_none(),
                "Postman cannot export an owner-bound OAuth1 vault token; use manual credentials"
            );
            ensure!(
                matches!(
                    c.location,
                    moleapi_core::OAuth1Location::Header | moleapi_core::OAuth1Location::Automatic
                ),
                "Postman OAuth1 only maps Header or automatic placement"
            );
            let fields = [
                ("consumerKey", c.consumer_key.as_str()),
                ("consumerSecret", c.consumer_secret.as_str()),
                ("token", c.token.as_str()),
                ("tokenSecret", c.token_secret.as_str()),
                ("privateKey", c.private_key.as_str()),
                ("signatureMethod", c.algorithm.as_str()),
                ("realm", c.realm.as_str()),
                ("nonce", c.nonce.as_str()),
                ("timestamp", c.timestamp.as_str()),
                ("callback", c.callback.as_str()),
                ("verifier", c.verifier.as_str()),
                ("version", if c.include_version { "1.0" } else { "" }),
            ];
            let mut rows: Vec<Value> = fields
                .into_iter()
                .map(|(key, value)| json!({"key":key,"value":value,"type":"string"}))
                .collect();
            for (key, value) in [
                ("includeBodyHash", c.include_body_hash),
                ("addEmptyParamsToSign", c.include_empty_params),
                (
                    "addParamsToHeader",
                    c.location == moleapi_core::OAuth1Location::Header,
                ),
            ] {
                rows.push(json!({"key":key,"value":value,"type":"boolean"}));
            }
            json!({"type":"oauth1","oauth1":rows})
        }
        "hawk" => {
            let hawk = auth.hawk.as_ref().context("Hawk settings missing")?;
            ensure!(
                ["sha1", "sha256"].contains(&hawk.algorithm.to_ascii_lowercase().as_str())
                    || hawk.algorithm.contains("{{"),
                "Hawk algorithm requires native MoleAPI export"
            );
            json!({"type":"hawk","hawk":[{"key":"authId","value":hawk.id,"type":"string"},{"key":"authKey","value":hawk.key,"type":"string"},{"key":"algorithm","value":hawk.algorithm,"type":"string"},{"key":"nonce","value":hawk.nonce,"type":"string"},{"key":"timestamp","value":hawk.timestamp,"type":"string"},{"key":"extraData","value":hawk.ext,"type":"string"},{"key":"app","value":hawk.app,"type":"string"},{"key":"delegation","value":hawk.delegation,"type":"string"},{"key":"user","value":hawk.user,"type":"string"},{"key":"includePayloadHash","value":hawk.include_payload_hash,"type":"boolean"}]})
        }
        "aws" => {
            let aws = auth.aws.as_ref().context("AWS settings missing")?;
            ensure!(
                aws.location == moleapi_core::AuthLocation::Header && !aws.unsigned_payload,
                "AWS presign/unsigned payload settings require native MoleAPI export"
            );
            json!({"type":"awsv4","awsv4":[{"key":"accessKey","value":aws.access_key,"type":"string"},{"key":"secretKey","value":aws.secret_key,"type":"string"},{"key":"sessionToken","value":aws.session_token,"type":"string"},{"key":"region","value":aws.region,"type":"string"},{"key":"service","value":aws.service,"type":"string"},{"key":"addAuthDataToQuery","value":false,"type":"boolean"}]})
        }
        _ => bail!("Selected authentication cannot be preserved in Postman format"),
    })
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

fn export_request(r: &moleapi_core::RequestSpec) -> Result<Value> {
    let mut url = r.url.clone();
    if !r.query.is_empty() {
        let values = r
            .query
            .iter()
            .filter(|p| p.enabled)
            .map(|p| {
                format!(
                    "{}={}",
                    url::form_urlencoded::byte_serialize(p.key.as_bytes()).collect::<String>(),
                    url::form_urlencoded::byte_serialize(p.value.as_bytes()).collect::<String>()
                )
            })
            .collect::<Vec<_>>()
            .join("&");
        if !values.is_empty() {
            url.push(if url.contains('?') { '&' } else { '?' });
            url.push_str(&values);
        }
    }
    let auth = export_auth(&r.auth)?;
    let mut request = json!({"method":r.method,"url":url,"description":r.description,"header":r.headers.iter().map(|h|json!({"key":h.key,"value":h.value,"disabled":!h.enabled})).collect::<Vec<_>>(),"auth":auth});
    if let moleapi_core::Protocol::Graphql {
        document,
        variables,
        variables_source,
        ..
    } = &r.protocol
    {
        request["body"] = json!({"mode":"graphql","graphql":{"query":document,"variables":variables_source.as_deref().map(str::to_owned).unwrap_or_else(||variables.to_string())}});
    } else if r.body_kind == "binary" {
        let file: moleapi_core::BinaryBody =
            serde_json::from_str(&r.body).expect("validated binary source");
        request["body"] = json!({"mode":"file","file":{"src":file.file_name}});
        if !file.mime.is_empty()
            && !r
                .headers
                .iter()
                .any(|h| h.enabled && h.key.eq_ignore_ascii_case("content-type"))
        {
            request["header"]
                .as_array_mut()
                .unwrap()
                .push(json!({"key":"Content-Type","value":file.mime,"disabled":false}));
        }
    } else if r.body_kind == "multipart" {
        let body: moleapi_core::MultipartBody =
            serde_json::from_str(&r.body).expect("validated multipart source");
        request["body"] = json!({"mode":"formdata","formdata":body.parts.iter().map(|p| match &p.value {
                moleapi_core::MultipartValue::Text{text,mime}=>json!({"key":p.name,"type":"text","value":text,"contentType":mime,"disabled":!p.enabled}),
                moleapi_core::MultipartValue::File{file}=>json!({"key":p.name,"type":"file","src":file.file_name,"contentType":file.mime,"disabled":!p.enabled}),
            }).collect::<Vec<_>>()});
    } else if r.body_kind == "form" {
        request["body"] = json!({"mode":"urlencoded","urlencoded":url::form_urlencoded::parse(r.body.as_bytes()).map(|(key,value)|json!({"key":key,"value":value,"type":"text"})).collect::<Vec<_>>()});
    } else if r.body_kind != "none" {
        request["body"] = json!({"mode":"raw","raw":r.body,"options":{"raw":{"language":if r.body_kind=="json"{"json"}else{"text"}}}});
    }
    Ok(
        json!({"name":r.name,"request":request,"event":events(&r.pre_request_script,&r.post_response_script),"response":r.examples.iter().map(|e|json!({"name":e.name,"code":e.status,"body":e.body,"header":e.headers.iter().map(|h|json!({"key":h.key,"value":h.value})).collect::<Vec<_>>(),"originalRequest":request})).collect::<Vec<_>>()}),
    )
}
fn export_collection(
    workspace: &Workspace,
    collection: &Collection,
    depth: usize,
) -> Result<Value> {
    anyhow::ensure!(
        depth < moleapi_core::MAX_COLLECTION_DEPTH,
        "Collection depth exceeds limit"
    );
    let mut items = collection
        .requests
        .iter()
        .map(export_request)
        .collect::<Result<Vec<_>>>()?;
    for child in workspace
        .data
        .collections
        .iter()
        .filter(|c| c.parent_id.as_deref() == Some(collection.id.as_str()))
    {
        items.push(export_collection(workspace, child, depth + 1)?);
    }
    let mut value = json!({"name":collection.name,"description":collection.description,"event":events(&collection.pre_request_script,&collection.post_response_script),"variable":collection.variables.iter().map(|v|json!({"key":v.key,"value":v.value,"type":"string","disabled":!v.enabled})).collect::<Vec<_>>(),"item":items});
    if let Some(auth) = &collection.auth {
        value["auth"] = export_auth(auth)?;
    }
    Ok(value)
}
pub(super) fn export(workspace: &Workspace) -> Result<String> {
    anyhow::ensure!(
        !workspace
            .data
            .collections
            .iter()
            .any(|c| c.parent_id.is_some()
                && c.variables_enabled != Some(false)
                && c.variables.iter().any(|v| v.enabled)),
        "MoleAPI executes folder variables that Postman Runtime currently ignores; disable folder-variable execution or use MoleAPI export"
    );
    let roots = workspace
        .data
        .collections
        .iter()
        .filter(|c| c.parent_id.is_none())
        .collect::<Vec<_>>();
    let direct_root = roots.len() == 1
        && workspace.data.auth.is_none()
        && workspace.data.global_variables.is_empty()
        && workspace.data.pre_request_script.is_empty()
        && workspace.data.post_response_script.is_empty();
    // Postman collection export snapshots the selected environment. Keep source
    // hierarchy when those values cannot override differently-valued folder vars.
    if let Some(environment) = workspace
        .data
        .environments
        .iter()
        .find(|e| Some(&e.id) == workspace.data.active_environment_id.as_ref())
    {
        for row in environment.variables.iter().filter(|v| v.enabled) {
            anyhow::ensure!(
                !workspace
                    .data
                    .collections
                    .iter()
                    .filter(|c| !direct_root || c.parent_id.is_some())
                    .flat_map(|c| &c.variables)
                    .any(|v| v.enabled && v.key == row.key && v.value != row.value),
                "Selected environment conflicts with folder variables; use MoleAPI until separate Postman environment exports are available"
            );
        }
    }
    anyhow::ensure!(
        direct_root
            || !workspace.data.collections.iter().any(
                |c| c.variables_enabled != Some(false) && c.variables.iter().any(|v| v.enabled)
            ),
        "Wrapped collection variables are ignored by Postman Runtime; use a single direct collection or MoleAPI export"
    );
    anyhow::ensure!(
        !direct_root
            || roots[0].variables_enabled != Some(false)
            || !roots[0].variables.iter().any(|v| v.enabled),
        "Source-only root collection variables would become active in Postman; use MoleAPI export until that flag can be preserved"
    );
    let items = roots
        .iter()
        .map(|c| export_collection(workspace, c, 0))
        .collect::<Result<Vec<_>>>()?;
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
    let mut result = json!({"info":{"name":workspace.name,"schema":"https://schema.getpostman.com/json/collection/v2.1.0/collection.json"},"item":items,"variable":variables});
    if direct_root {
        result["info"]["name"] = json!(roots[0].name);
        let collection = export_collection(workspace, roots[0], 0)?;
        for field in ["item", "event", "auth", "description"] {
            if let Some(value) = collection.get(field) {
                result[field] = value.clone();
            }
        }
        let mut vars = collection["variable"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        vars.extend(result["variable"].as_array().into_iter().flatten().cloned());
        result["variable"] = json!(vars);
    } else {
        result["event"] = json!(events(
            &workspace.data.pre_request_script,
            &workspace.data.post_response_script
        ));
        if let Some(auth) = &workspace.data.auth {
            result["auth"] = export_auth(auth)?;
        }
        let mut vars = workspace
            .data
            .global_variables
            .iter()
            .map(|v| json!({"key":v.key,"value":v.value,"type":"string","disabled":!v.enabled}))
            .collect::<Vec<_>>();
        vars.extend(result["variable"].as_array().into_iter().flatten().cloned());
        result["variable"] = json!(vars);
    }
    let mut merged = Vec::<Value>::new();
    let mut positions = std::collections::HashMap::<String, usize>::new();
    for value in result["variable"].as_array().into_iter().flatten() {
        let key = value["key"]
            .as_str()
            .context("Variable key missing")?
            .to_owned();
        if let Some(index) = positions.get(&key) {
            merged[*index] = value.clone();
        } else {
            positions.insert(key, merged.len());
            merged.push(value.clone());
        }
    }
    result["variable"] = json!(merged);
    let schema: Value = serde_json::from_str(include_str!("../schemas/postman21.json"))?;
    jsonschema::validator_for(&schema)?
        .validate(&result)
        .map_err(|e| anyhow::anyhow!("导出结果不符合官方 Postman schema: {e}"))?;
    Ok(serde_json::to_string_pretty(&result)?)
}
