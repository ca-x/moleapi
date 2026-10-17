//! Bounded HAR 1.2 HTTP request import through the shared workspace converter.
use crate::{ImportResult, data, pair, request, uid};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use moleapi_core::{Collection, Example, MAX_BODY};
use serde::Deserialize;
use std::collections::BTreeSet;
use url::{Url, form_urlencoded};

const MAX_ENTRIES: usize = 1_000;
const MAX_EXAMPLE: usize = 1024 * 1024;

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
    } else {
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
        mock_script: String::new(),
        mock_match: None,
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
