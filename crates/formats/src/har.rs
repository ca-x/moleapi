//! Bounded HAR 1.2 HTTP request import through the shared workspace converter.
use crate::{ImportResult, data, pair, request, uid};
use anyhow::{Context, Result, bail, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use moleapi_core::{Collection, Example, MAX_BODY, Protocol, Workspace};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use url::{Url, form_urlencoded};

const MAX_ENTRIES: usize = 1_000;
const MAX_EXAMPLE: usize = 1024 * 1024;
const MAX_HAR_BYTES: usize = 20 * 1024 * 1024;

pub(super) fn export(workspace: &Workspace) -> Result<(String, Vec<String>)> {
    ensure!(
        workspace.data.auth.is_none()
            && workspace.data.pre_request_script.trim().is_empty()
            && workspace.data.post_response_script.trim().is_empty(),
        "HAR cannot preserve workspace authentication or scripts; use MoleAPI backup"
    );
    let count: usize = workspace
        .data
        .collections
        .iter()
        .map(|c| c.requests.len())
        .sum();
    ensure!(
        (1..=MAX_ENTRIES).contains(&count),
        "HAR export requires 1–1000 requests"
    );
    let mut entries = Vec::with_capacity(count);
    let mut encoded_bytes = 0usize;
    let mut warnings = BTreeSet::new();
    warnings.insert("HAR is a saved request template, not observed network traffic".into());
    warnings.insert("HAR omits workspace structure and non-wire metadata".into());
    if !workspace.data.environments.is_empty() || !workspace.data.global_variables.is_empty() {
        warnings.insert("HAR omits workspace environments and variables".into());
    }
    for collection in &workspace.data.collections {
        ensure!(
            collection.auth.is_none()
                && collection.pre_request_script.trim().is_empty()
                && collection.post_response_script.trim().is_empty(),
            "HAR cannot preserve collection authentication or scripts; use MoleAPI backup"
        );
        for request in &collection.requests {
            ensure!(
                request.protocol == Protocol::Http
                    && request.auth.kind == "none"
                    && request.network.is_none()
                    && request.pre_request_script.trim().is_empty()
                    && request.post_response_script.trim().is_empty()
                    && request.assertions.is_empty()
                    && request.extractions.is_empty(),
                "HAR cannot preserve this request's protocol, authentication, scripts or policies"
            );
            ensure!(
                !request.method.is_empty()
                    && request.method.len() <= 16
                    && request
                        .method
                        .bytes()
                        .all(|byte| byte.is_ascii_uppercase() || byte == b'-'),
                "HAR request method is invalid"
            );
            ensure!(
                !request.url.contains("{{")
                    && request
                        .query
                        .iter()
                        .filter(|row| row.enabled)
                        .all(|row| !row.key.contains("{{") && !row.value.contains("{{"))
                    && request
                        .headers
                        .iter()
                        .filter(|row| row.enabled)
                        .all(|row| !row.key.contains("{{") && !row.value.contains("{{"))
                    && !request.body.contains("{{"),
                "HAR requires literal request URL, headers, query and body"
            );
            let mut url = Url::parse(&request.url).context("HAR request URL is invalid")?;
            ensure!(
                matches!(url.scheme(), "http" | "https")
                    && url.host_str().is_some()
                    && url.fragment().is_none()
                    && url.username().is_empty()
                    && url.password().is_none(),
                "HAR requires an absolute HTTP(S) URL without credentials or fragment"
            );
            for row in request.query.iter().filter(|row| row.enabled) {
                url.query_pairs_mut().append_pair(&row.key, &row.value);
            }
            if request.use_cookie_jar {
                warnings.insert("HAR omits live Cookie Jar state".into());
            }
            if request.follow_redirects || !request.verify_tls {
                warnings.insert("HAR omits request redirect and TLS policy".into());
            }
            let mut headers = Vec::new();
            let nominated: BTreeSet<String> = request
                .headers
                .iter()
                .filter(|row| row.enabled && row.key.eq_ignore_ascii_case("connection"))
                .flat_map(|row| {
                    row.value
                        .split(',')
                        .map(|name| name.trim().to_ascii_lowercase())
                })
                .collect();
            for row in request.headers.iter().filter(|row| row.enabled) {
                let name = row.key.to_ascii_lowercase();
                ensure!(
                    name != "upgrade" && !name.starts_with("sec-websocket-"),
                    "HAR cannot export WebSocket upgrade requests as HTTP"
                );
                if nominated.contains(&name)
                    || matches!(
                        name.as_str(),
                        "host"
                            | "content-length"
                            | "transfer-encoding"
                            | "connection"
                            | "accept-encoding"
                            | "keep-alive"
                            | "proxy-authorization"
                            | "proxy-connection"
                            | "te"
                            | "trailer"
                    )
                {
                    warnings.insert(format!("HAR transport header {} omitted", row.key));
                } else {
                    headers.push(json!({"name":row.key,"value":row.value}));
                }
            }
            let post_data = export_body(request, &mut headers)?;
            let response = export_example(request, &mut warnings)?;
            let query: Vec<Value> = url
                .query_pairs()
                .map(|(name, value)| json!({"name":name,"value":value}))
                .collect();
            let mut recorded = json!({
                "method":request.method,"url":url.as_str(),"httpVersion":"",
                "cookies":[],"headers":headers,"queryString":query,
                "headersSize":-1,"bodySize":-1
            });
            if let Some(post_data) = post_data {
                recorded["postData"] = post_data;
            }
            let entry = json!({
                "startedDateTime":"1970-01-01T00:00:00.000Z","time":-1,
                "request":recorded,"response":response,"cache":{},
                "timings":{"send":-1,"wait":-1,"receive":-1},
                "comment":"MoleAPI saved request template; no traffic was captured"
            });
            encoded_bytes = encoded_bytes.saturating_add(serde_json::to_vec(&entry)?.len());
            ensure!(encoded_bytes <= MAX_HAR_BYTES, "HAR export exceeds 20 MiB");
            entries.push(entry);
        }
    }
    let content = serde_json::to_string_pretty(&json!({
        "log":{"version":"1.2","creator":{"name":"MoleAPI","version":env!("CARGO_PKG_VERSION")},"entries":entries}
    }))?;
    ensure!(content.len() <= MAX_HAR_BYTES, "HAR export exceeds 20 MiB");
    Ok((content, warnings.into_iter().collect()))
}

fn export_body(
    request: &moleapi_core::RequestSpec,
    headers: &mut Vec<Value>,
) -> Result<Option<Value>> {
    if request.body_kind == "none" {
        ensure!(
            request.body.is_empty(),
            "HAR cannot export body bytes with body kind none"
        );
        return Ok(None);
    }
    let expected = match request.body_kind.as_str() {
        "json" => "application/json",
        "form" => "application/x-www-form-urlencoded",
        "text" => "text/plain",
        _ => bail!("HAR cannot export binary, multipart or other unsupported body modes"),
    };
    ensure!(
        request.body.len() <= MAX_BODY,
        "HAR request body exceeds 5 MiB"
    );
    let mime = headers
        .iter()
        .find_map(|header| {
            (header["name"]
                .as_str()?
                .eq_ignore_ascii_case("content-type"))
            .then(|| header["value"].as_str().unwrap_or_default().to_owned())
        })
        .unwrap_or_else(|| expected.into());
    let kind = mime
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    ensure!(
        request.body_kind == "text"
            && (kind.starts_with("text/")
                || kind.ends_with("+xml")
                || matches!(
                    kind.as_str(),
                    "application/xml" | "application/graphql" | "application/javascript"
                ))
            || request.body_kind == "json"
                && (kind == "application/json" || kind.ends_with("+json"))
            || request.body_kind == "form" && kind == "application/x-www-form-urlencoded",
        "HAR Content-Type disagrees with request body mode"
    );
    if request.body_kind != "text"
        && !headers.iter().any(|header| {
            header["name"]
                .as_str()
                .is_some_and(|name| name.eq_ignore_ascii_case("content-type"))
        })
    {
        headers.push(json!({"name":"Content-Type","value":mime}));
    }
    Ok(Some(json!({"mimeType":mime,"text":request.body})))
}

fn export_example(
    request: &moleapi_core::RequestSpec,
    warnings: &mut BTreeSet<String>,
) -> Result<Value> {
    ensure!(
        request.examples.len() <= 1,
        "HAR cannot select among multiple response examples"
    );
    let Some(example) = request.examples.first() else {
        return Ok(json!({
            "status":0,"statusText":"","httpVersion":"","cookies":[],"headers":[],
            "content":{"size":0,"mimeType":"text/plain"},"redirectURL":"",
            "headersSize":-1,"bodySize":-1,
            "comment":"No saved response example; status is a placeholder"
        }));
    };
    ensure!(
        (100..=599).contains(&example.status),
        "HAR example status is invalid"
    );
    ensure!(
        example.body.len() <= MAX_EXAMPLE,
        "HAR response example exceeds 1 MiB"
    );
    let mut headers = Vec::new();
    let mut mime = "text/plain".to_owned();
    let nominated: BTreeSet<String> = example
        .headers
        .iter()
        .filter(|row| row.enabled && row.key.eq_ignore_ascii_case("connection"))
        .flat_map(|row| {
            row.value
                .split(',')
                .map(|name| name.trim().to_ascii_lowercase())
        })
        .collect();
    for row in example.headers.iter().filter(|row| row.enabled) {
        let name = row.key.to_ascii_lowercase();
        if nominated.contains(&name)
            || matches!(
                name.as_str(),
                "content-encoding" | "content-length" | "transfer-encoding" | "connection"
            )
        {
            warnings.insert(format!("HAR response transport header {} omitted", row.key));
            continue;
        }
        if name == "content-type" {
            mime = row.value.clone();
        }
        headers.push(json!({"name":row.key,"value":row.value}));
    }
    let kind = mime
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    ensure!(
        kind.starts_with("text/")
            || kind == "application/json"
            || kind.ends_with("+json")
            || kind.ends_with("+xml")
            || matches!(kind.as_str(), "application/xml" | "application/javascript"),
        "HAR cannot export a binary response example"
    );
    Ok(json!({
        "status":example.status,"statusText":"","httpVersion":"","cookies":[],
        "headers":headers,"content":{"size":example.body.len(),"mimeType":mime,"text":example.body},
        "redirectURL":"","headersSize":-1,"bodySize":-1
    }))
}

#[derive(Deserialize)]
struct Capture {
    log: Log,
}
#[derive(Deserialize)]
struct Log {
    version: String,
    entries: Vec<Entry>,
}
#[derive(Deserialize)]
struct Entry {
    request: RecordedRequest,
    response: Option<RecordedResponse>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordedRequest {
    method: String,
    url: String,
    #[serde(default)]
    headers: Vec<NameValue>,
    #[serde(default)]
    query_string: Vec<NameValue>,
    #[serde(default)]
    cookies: Vec<NameValue>,
    post_data: Option<PostData>,
}
#[derive(Deserialize)]
struct NameValue {
    name: String,
    value: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PostData {
    mime_type: String,
    text: Option<String>,
    #[serde(default)]
    params: Vec<PostParam>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PostParam {
    name: String,
    value: Option<String>,
    file_name: Option<String>,
}
#[derive(Deserialize)]
struct RecordedResponse {
    status: Option<i64>,
    #[serde(default)]
    headers: Vec<NameValue>,
    content: Option<ResponseContent>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResponseContent {
    mime_type: Option<String>,
    text: Option<String>,
    encoding: Option<String>,
}

pub(super) fn import(content: &str) -> Result<ImportResult> {
    let capture: Capture = serde_json::from_str(content).context("Invalid HAR 1.2 JSON")?;
    ensure!(capture.log.version == "1.2", "HAR version must be 1.2");
    ensure!(
        (1..=MAX_ENTRIES).contains(&capture.log.entries.len()),
        "HAR must contain 1–1000 entries"
    );
    let mut warnings = BTreeSet::new();
    let mut requests = Vec::with_capacity(capture.log.entries.len());
    for (index, entry) in capture.log.entries.into_iter().enumerate() {
        let number = index + 1;
        let recorded = entry.request;
        ensure!(
            !recorded.method.is_empty()
                && recorded.method.len() <= 16
                && recorded
                    .method
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || byte == b'-'),
            "HAR entry {number} has an invalid HTTP method"
        );
        let parsed = Url::parse(&recorded.url)
            .with_context(|| format!("HAR entry {number} has an invalid URL"))?;
        ensure!(
            matches!(parsed.scheme(), "http" | "https")
                && parsed.host_str().is_some()
                && parsed.fragment().is_none(),
            "HAR entry {number} requires an absolute HTTP(S) URL without a fragment"
        );
        if !recorded.query_string.is_empty() {
            let actual: Vec<(String, String)> = parsed
                .query_pairs()
                .map(|(key, value)| (key.into_owned(), value.into_owned()))
                .collect();
            let listed: Vec<(String, String)> = recorded
                .query_string
                .iter()
                .map(|item| (item.name.clone(), item.value.clone()))
                .collect();
            ensure!(
                actual == listed,
                "HAR entry {number} queryString disagrees with URL"
            );
        }
        let name = format!(
            "{} {}",
            recorded.method,
            parsed.path().chars().take(80).collect::<String>()
        );
        let mut imported = request(name, recorded.method, recorded.url)?;
        imported.use_cookie_jar = false;
        imported.follow_redirects = false;
        for header in recorded.headers {
            let lower = header.name.to_ascii_lowercase();
            ensure!(
                lower != "upgrade" && !lower.starts_with("sec-websocket-"),
                "HAR entry {number} contains a protocol upgrade that cannot be imported as HTTP"
            );
            if matches!(
                lower.as_str(),
                "host"
                    | "content-length"
                    | "transfer-encoding"
                    | "connection"
                    | "accept-encoding"
                    | "keep-alive"
                    | "proxy-authorization"
                    | "proxy-connection"
                    | "te"
                    | "trailer"
            ) {
                warnings.insert(format!("HAR transport header {} omitted", header.name));
                continue;
            }
            imported.headers.push(pair(&header.name, &header.value));
        }
        if !recorded.cookies.is_empty() {
            let cookies = recorded
                .cookies
                .iter()
                .map(|cookie| {
                    ensure!(
                        !cookie.name.is_empty()
                            && cookie.name.bytes().all(|byte| {
                                byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
                            })
                            && !cookie.value.contains([';', '\r', '\n']),
                        "HAR entry {number} has an invalid structured cookie"
                    );
                    Ok(format!("{}={}", cookie.name, cookie.value))
                })
                .collect::<Result<Vec<_>>>()?
                .join("; ");
            if let Some(header) = imported
                .headers
                .iter()
                .find(|header| header.key.eq_ignore_ascii_case("cookie"))
            {
                if header.value != cookies {
                    warnings.insert(
                        "HAR structured cookies differ from Cookie header; header retained".into(),
                    );
                }
            } else {
                imported.headers.push(pair("Cookie", &cookies));
            }
        }
        if let Some(post_data) = recorded.post_data {
            import_body(&mut imported, post_data, number)?;
        }
        if let Some(response) = entry.response {
            import_example(&mut imported, response, &mut warnings);
        }
        requests.push(imported);
    }
    Ok(ImportResult {
        name: "HAR import".into(),
        data: data(
            vec![Collection {
                variables_enabled: None,
                parent_id: None,
                auth: None,
                variables: vec![],
                pre_request_script: String::new(),
                post_response_script: String::new(),
                id: uid(),
                name: "HAR".into(),
                description: String::new(),
                requests,
            }],
            vec![],
        ),
        warnings: warnings.into_iter().collect(),
    })
}

fn import_body(
    request: &mut moleapi_core::RequestSpec,
    post: PostData,
    number: usize,
) -> Result<()> {
    let mime = post.mime_type.trim();
    let kind = mime
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    ensure!(
        kind != "multipart/form-data" && kind != "application/octet-stream",
        "HAR entry {number} has a file or multipart body that cannot be imported exactly"
    );
    ensure!(
        post.params.iter().all(|param| param.file_name.is_none()),
        "HAR entry {number} has a file parameter without embedded bytes"
    );
    let form = kind == "application/x-www-form-urlencoded";
    let body = if let Some(text) = post.text {
        if form && !post.params.is_empty() {
            let actual: Vec<(String, String)> = form_urlencoded::parse(text.as_bytes())
                .into_owned()
                .collect();
            ensure!(
                actual == literal_params(&post.params, number)?,
                "HAR entry {number} form params disagree with body text"
            );
        }
        text
    } else if form && !post.params.is_empty() {
        let mut encoder = form_urlencoded::Serializer::new(String::new());
        for (name, value) in literal_params(&post.params, number)? {
            encoder.append_pair(&name, &value);
        }
        encoder.finish()
    } else {
        anyhow::bail!("HAR entry {number} omits request body bytes");
    };
    ensure!(
        body.len() <= MAX_BODY,
        "HAR entry {number} body exceeds 5 MiB"
    );
    let mode = if form {
        "form"
    } else if kind == "application/json" || kind.ends_with("+json") {
        "json"
    } else if kind.starts_with("text/")
        || matches!(
            kind.as_str(),
            "application/xml" | "application/graphql" | "application/javascript"
        )
        || kind.ends_with("+xml")
    {
        "text"
    } else {
        anyhow::bail!("HAR entry {number} body MIME type is not a supported text format");
    };
    if let Some(header) = request
        .headers
        .iter()
        .find(|header| header.key.eq_ignore_ascii_case("content-type"))
    {
        ensure!(
            header
                .value
                .split(';')
                .next()
                .unwrap_or("")
                .trim()
                .eq_ignore_ascii_case(&kind),
            "HAR entry {number} Content-Type disagrees with postData"
        );
    } else if mode != "text" {
        request.headers.push(pair("Content-Type", mime));
    }
    request.body_kind = mode.into();
    request.body = body;
    Ok(())
}

fn literal_params(params: &[PostParam], number: usize) -> Result<Vec<(String, String)>> {
    params
        .iter()
        .map(|param| {
            ensure!(
                param.file_name.is_none(),
                "HAR entry {number} has a file parameter without embedded bytes"
            );
            let value = param
                .value
                .as_ref()
                .with_context(|| format!("HAR entry {number} form parameter has no value"))?;
            Ok((param.name.clone(), value.clone()))
        })
        .collect()
}

fn import_example(
    request: &mut moleapi_core::RequestSpec,
    response: RecordedResponse,
    warnings: &mut BTreeSet<String>,
) {
    let Some(status) = response
        .status
        .filter(|status| (100..=599).contains(status))
    else {
        warnings.insert("HAR response without HTTP status omitted".into());
        return;
    };
    let Some(content) = response.content else {
        return;
    };
    let Some(text) = content.text else {
        return;
    };
    let mime = content
        .mime_type
        .as_deref()
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if !mime.is_empty()
        && !(mime.starts_with("text/")
            || mime == "application/json"
            || mime.ends_with("+json")
            || mime.ends_with("+xml")
            || matches!(mime.as_str(), "application/xml" | "application/javascript"))
    {
        warnings.insert("HAR binary response body omitted".into());
        return;
    }
    let body = if content.encoding.as_deref() == Some("base64") {
        let decoded = match STANDARD.decode(text.as_bytes()) {
            Ok(value) => value,
            Err(_) => {
                warnings.insert("HAR invalid base64 response body omitted".into());
                return;
            }
        };
        match String::from_utf8(decoded) {
            Ok(value) => value,
            Err(_) => {
                warnings.insert("HAR non-UTF-8 response body omitted".into());
                return;
            }
        }
    } else if content.encoding.is_none() {
        text
    } else {
        warnings.insert("HAR response with unsupported encoding omitted".into());
        return;
    };
    if body.len() > MAX_EXAMPLE {
        warnings.insert("HAR response example above 1 MiB omitted".into());
        return;
    }
    request.examples.push(Example {
        id: uid(),
        name: format!("HAR {status}"),
        status: status as u16,
        mock_schema: String::new(),
        mock_name_inference: false,
        mock_schema_link: None,
        mock_script: String::new(),
        mock_match: None,
        delay_ms: 0,
        headers: response
            .headers
            .into_iter()
            .filter_map(|header| {
                if matches!(
                    header.name.to_ascii_lowercase().as_str(),
                    "content-encoding" | "content-length" | "transfer-encoding" | "connection"
                ) {
                    warnings.insert(format!(
                        "HAR response transport header {} omitted",
                        header.name
                    ));
                    None
                } else {
                    Some(pair(&header.name, &header.value))
                }
            })
            .collect(),
        body,
    });
}
