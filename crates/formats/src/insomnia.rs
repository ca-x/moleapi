//! Insomnia V4 JSON import through the shared, bounded workspace converter.
use crate::{ImportResult, data, pair, request, uid};
use anyhow::{Context, Result, bail, ensure};
use moleapi_core::{Collection, Environment, Specification};
use regex::Regex;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use url::{Url, form_urlencoded};

const MAX_SOURCE: usize = 5 * 1024 * 1024;
const MAX_REQUESTS: usize = 1_000;
const MAX_FOLDERS: usize = 1_000;
const MAX_ENVIRONMENTS: usize = 100;

pub(super) fn import(content: &str) -> Result<ImportResult> {
    ensure!(
        content.len() <= MAX_SOURCE,
        "Insomnia V4 source exceeds 5 MiB"
    );
    let source: Value = serde_json::from_str(content).context("Invalid Insomnia V4 JSON")?;
    ensure!(
        source["__export_format"] == 4,
        "Insomnia export format must be V4 JSON"
    );
    ensure!(
        source["_type"] == "export",
        "Insomnia V4 export type is missing"
    );
    let resources = source["resources"]
        .as_array()
        .context("Insomnia V4 resources must be an array")?;
    ensure!(
        !resources.is_empty() && resources.len() <= 3_101,
        "Insomnia resource limit exceeded"
    );
    let mut by_id = HashMap::new();
    let mut workspace = None;
    let mut requests = 0usize;
    let mut folders = 0usize;
    let mut environments = 0usize;
    for resource in resources {
        let id = field(resource, "_id")?;
        ensure!(
            !id.is_empty() && id.len() <= 256,
            "Invalid Insomnia resource ID"
        );
        ensure!(
            by_id.insert(id, resource).is_none(),
            "Duplicate Insomnia resource ID {id}"
        );
        match field(resource, "_type")? {
            "workspace" => {
                ensure!(
                    workspace.replace(id).is_none(),
                    "Insomnia export has multiple workspaces"
                );
            }
            "request_group" | "folder" => folders += 1,
            "request" => requests += 1,
            "environment" => environments += 1,
            kind => bail!("Unsupported Insomnia resource type {kind}"),
        }
    }
    ensure!(
        (1..=MAX_REQUESTS).contains(&requests),
        "Insomnia export requires 1–1000 HTTP requests"
    );
    ensure!(
        folders <= MAX_FOLDERS && environments <= MAX_ENVIRONMENTS,
        "Insomnia folder or environment limit exceeded"
    );
    let workspace_id = workspace.context("Insomnia workspace is missing")?;
    let workspace_source = by_id[workspace_id];
    let name = field(workspace_source, "name")?.to_owned();
    ensure!(!name.trim().is_empty(), "Insomnia workspace name is empty");

    let mut folder_ids = HashMap::new();
    for resource in resources {
        if matches!(field(resource, "_type")?, "request_group" | "folder") {
            parent_folder(field(resource, "_id")?, workspace_id, &by_id)?;
            folder_ids.insert(field(resource, "_id")?, uid());
        }
    }
    let mut collections = Vec::with_capacity(folders + 1);
    let mut collection_index = HashMap::new();
    for resource in resources {
        if matches!(field(resource, "_type")?, "request_group" | "folder") {
            let source_id = field(resource, "_id")?;
            let parent = field(resource, "parentId")?;
            let parent_id = if parent == workspace_id {
                None
            } else {
                Some(folder_ids[parent].clone())
            };
            collection_index.insert(source_id, collections.len());
            collections.push(Collection {
                id: folder_ids[source_id].clone(),
                name: field(resource, "name")?.to_owned(),
                description: optional_string(resource, "description")?
                    .unwrap_or_default()
                    .into(),
                requests: Vec::new(),
                parent_id,
                auth: None,
                variables: Vec::new(),
                variables_enabled: None,
                pre_request_script: String::new(),
                post_response_script: String::new(),
            });
        }
    }

    let (parsed_environments, variables) = read_environments(resources, workspace_id)?;
    let placeholders = Regex::new(r"\{\{\s*_\.\s*([A-Za-z_][A-Za-z0-9_]*)\s*\}\}")?;
    let mut warnings = Vec::new();
    let specification_id = uid();
    for resource in resources {
        if field(resource, "_type")? != "request" {
            continue;
        }
        let parent = field(resource, "parentId")?;
        if parent != workspace_id {
            parent_folder(parent, workspace_id, &by_id)?;
        }
        let mut converted = convert_request(resource, &variables, &placeholders, &mut warnings)?;
        converted.specification_id = Some(specification_id.clone());
        let index = if parent == workspace_id {
            if let Some(index) = collection_index.get(workspace_id) {
                *index
            } else {
                let index = collections.len();
                collection_index.insert(workspace_id, index);
                collections.push(Collection {
                    id: uid(),
                    name: name.clone(),
                    description: String::new(),
                    requests: Vec::new(),
                    parent_id: None,
                    auth: None,
                    variables: Vec::new(),
                    variables_enabled: None,
                    pre_request_script: String::new(),
                    post_response_script: String::new(),
                });
                index
            }
        } else {
            *collection_index
                .get(parent)
                .context("Insomnia request parent is missing")?
        };
        collections[index].requests.push(converted);
    }
    let mut result = data(collections, Vec::new());
    if !parsed_environments.is_empty() {
        result.active_environment_id = Some(parsed_environments[0].id.clone());
        result.environments = parsed_environments;
    }
    result.specifications.push(Specification {
        id: specification_id,
        name: name.clone(),
        kind: "insomnia".into(),
        source: content.to_owned(),
        dialect: "v4".into(),
    });
    Ok(ImportResult {
        name,
        data: result,
        warnings,
    })
}

fn field<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .with_context(|| format!("Insomnia {key} must be a string"))
}

fn optional_string<'a>(value: &'a Value, key: &str) -> Result<Option<&'a str>> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text)),
        _ => bail!("Insomnia {key} must be a string"),
    }
}

fn parent_folder<'a>(
    id: &'a str,
    workspace_id: &str,
    by_id: &HashMap<&str, &'a Value>,
) -> Result<()> {
    let mut seen = HashSet::new();
    let mut current = id;
    for _ in 0..=64 {
        if current == workspace_id {
            return Ok(());
        }
        ensure!(seen.insert(current), "Cyclic Insomnia folder parent");
        let resource = by_id
            .get(current)
            .context("Dangling Insomnia folder parent")?;
        ensure!(
            matches!(field(resource, "_type")?, "request_group" | "folder"),
            "Insomnia parent is not a folder"
        );
        current = field(resource, "parentId")?;
    }
    bail!("Insomnia folder depth exceeds 64")
}

fn environment_variables(resource: &Value) -> Result<Vec<moleapi_core::Pair>> {
    let values = resource["data"]
        .as_object()
        .context("Insomnia environment data must be an object")?;
    ensure!(
        values.len() <= 256,
        "Insomnia environment has too many values"
    );
    let mut result = Vec::with_capacity(values.len());
    for (key, value) in values {
        ensure!(
            !key.is_empty() && key.len() <= 256,
            "Invalid Insomnia variable name"
        );
        let text = match value {
            Value::String(text) => text.clone(),
            Value::Bool(_) | Value::Number(_) => value.to_string(),
            _ => bail!("Insomnia variable {key} cannot be represented as text"),
        };
        let mut entry = pair(key, text);
        if crate::redact::sensitive(key) {
            entry.secret = Some(true);
        }
        result.push(entry);
    }
    Ok(result)
}

fn read_environments(
    resources: &[Value],
    workspace_id: &str,
) -> Result<(Vec<Environment>, HashMap<String, String>)> {
    let sources: Vec<&Value> = resources
        .iter()
        .filter(|resource| resource["_type"] == "environment")
        .collect();
    if sources.is_empty() {
        return Ok((Vec::new(), HashMap::new()));
    }
    let bases: Vec<&Value> = sources
        .iter()
        .copied()
        .filter(|resource| resource["parentId"] == workspace_id)
        .collect();
    ensure!(
        bases.len() == 1,
        "Insomnia V4 requires exactly one base environment"
    );
    let base = bases[0];
    let base_id = field(base, "_id")?;
    let base_variables = environment_variables(base)?;
    let active_values = base_variables
        .iter()
        .map(|row| (row.key.clone(), row.value.clone()))
        .collect();
    let mut result = vec![Environment {
        id: uid(),
        name: field(base, "name")?.into(),
        variables: base_variables.clone(),
    }];
    for child in sources {
        if child["_id"] == base_id {
            continue;
        }
        ensure!(
            field(child, "parentId")? == base_id,
            "Nested Insomnia environment inheritance is unsupported"
        );
        let mut variables = base_variables.clone();
        for override_value in environment_variables(child)? {
            if let Some(existing) = variables
                .iter_mut()
                .find(|row| row.key == override_value.key)
            {
                *existing = override_value;
            } else {
                variables.push(override_value);
            }
        }
        result.push(Environment {
            id: uid(),
            name: field(child, "name")?.into(),
            variables,
        });
    }
    Ok((result, active_values))
}

fn normalize_url(
    source: &str,
    variables: &HashMap<String, String>,
    placeholders: &Regex,
) -> Result<String> {
    ensure!(
        source.len() <= 8192,
        "Insomnia request URL exceeds 8192 bytes"
    );
    let mut rewritten = String::with_capacity(source.len());
    let mut previous = 0;
    let mut resolved = String::with_capacity(source.len());
    for captures in placeholders.captures_iter(source) {
        let whole = captures
            .get(0)
            .context("Invalid Insomnia URL placeholder")?;
        let key = captures
            .get(1)
            .context("Invalid Insomnia variable name")?
            .as_str();
        let value = variables.get(key).with_context(|| {
            format!("Insomnia URL variable {key} is not in the base environment")
        })?;
        rewritten.push_str(&source[previous..whole.start()]);
        rewritten.push_str(&format!("{{{{{key}}}}}"));
        resolved.push_str(&source[previous..whole.start()]);
        resolved.push_str(value);
        previous = whole.end();
    }
    rewritten.push_str(&source[previous..]);
    resolved.push_str(&source[previous..]);
    ensure!(
        !rewritten.contains("{{ _.") && !rewritten.contains("{%") && !resolved.contains("{{"),
        "Unsupported Insomnia URL template"
    );
    let url = Url::parse(&resolved).context("Insomnia request URL is not absolute")?;
    ensure!(
        matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none(),
        "Insomnia request URL must be HTTP(S) without credentials or fragment"
    );
    Ok(rewritten)
}

fn rows<'a>(resource: &'a Value, key: &str) -> Result<Vec<&'a Value>> {
    match resource.get(key) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(values)) if values.len() <= 1_000 => Ok(values.iter().collect()),
        _ => bail!("Insomnia {key} must be an array of at most 1000 rows"),
    }
}

fn literal(value: &str) -> Result<()> {
    ensure!(
        !value.contains("{{") && !value.contains("{%"),
        "Unsupported Insomnia template expression"
    );
    Ok(())
}

fn enabled(row: &Value) -> Result<bool> {
    match row.get("disabled") {
        None | Some(Value::Null) => Ok(true),
        Some(Value::Bool(disabled)) => Ok(!disabled),
        _ => bail!("Insomnia disabled flag must be a boolean"),
    }
}

fn convert_request(
    resource: &Value,
    variables: &HashMap<String, String>,
    placeholders: &Regex,
    warnings: &mut Vec<String>,
) -> Result<moleapi_core::RequestSpec> {
    for key in [
        "preRequestScript",
        "afterResponseScript",
        "scripts",
        "tests",
    ] {
        if let Some(value) = resource.get(key) {
            ensure!(
                value.is_null() || value == "" || value == &Value::Array(Vec::new()),
                "Insomnia {key} cannot be mapped safely"
            );
        }
    }
    let method = field(resource, "method")?;
    ensure!(
        !method.is_empty()
            && method.len() <= 16
            && method
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte == b'-'),
        "Invalid Insomnia HTTP method"
    );
    let url = normalize_url(field(resource, "url")?, variables, placeholders)?;
    let mut converted = request(field(resource, "name")?.into(), method.into(), url.clone())?;
    converted.description = optional_string(resource, "description")?
        .unwrap_or_default()
        .into();
    let mut parameters = Vec::new();
    for row in rows(resource, "parameters")? {
        let name = field(row, "name")?;
        let value = field(row, "value")?;
        literal(name)?;
        literal(value)?;
        let mut item = pair(name, value);
        item.enabled = enabled(row)?;
        parameters.push(item);
    }
    let query = url.split_once('?').map(|(_, query)| query).unwrap_or("");
    if !query.is_empty() {
        let encoded: Vec<(String, String)> = form_urlencoded::parse(query.as_bytes())
            .into_owned()
            .collect();
        let listed: Vec<(String, String)> = parameters
            .iter()
            .filter(|row| row.enabled)
            .map(|row| (row.key.clone(), row.value.clone()))
            .collect();
        ensure!(
            listed.is_empty() || listed == encoded,
            "Insomnia URL and parameter list disagree"
        );
        if parameters.iter().any(|row| !row.enabled) {
            warnings.push("Insomnia disabled query parameters omitted from the URL query".into());
        }
    } else {
        converted.query = parameters;
    }
    let header_rows = rows(resource, "headers")?;
    let mut nominated = HashSet::new();
    for row in &header_rows {
        if enabled(row)? && field(row, "name")?.eq_ignore_ascii_case("connection") {
            for name in field(row, "value")?.split(',') {
                nominated.insert(name.trim().to_ascii_lowercase());
            }
        }
    }
    for row in header_rows {
        let name = field(row, "name")?;
        let value = field(row, "value")?;
        literal(name)?;
        literal(value)?;
        ensure!(
            !name.eq_ignore_ascii_case("upgrade")
                && !name.to_ascii_lowercase().starts_with("sec-websocket-"),
            "Insomnia WebSocket upgrade request is unsupported"
        );
        let mut item = pair(name, value);
        item.enabled = enabled(row)?;
        if nominated.contains(&name.to_ascii_lowercase())
            || matches!(
                name.to_ascii_lowercase().as_str(),
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
            if item.enabled {
                warnings.push(format!("Insomnia transport header {name} omitted"));
            }
            continue;
        }
        if crate::redact::sensitive(name) {
            item.secret = Some(true);
        }
        converted.headers.push(item);
    }
    if converted
        .headers
        .iter()
        .any(|row| row.enabled && row.key.eq_ignore_ascii_case("cookie"))
    {
        converted.use_cookie_jar = false;
    }
    read_auth(resource, &mut converted)?;
    read_body(resource, &mut converted)?;
    Ok(converted)
}

fn read_auth(source: &Value, request: &mut moleapi_core::RequestSpec) -> Result<()> {
    let Some(auth) = source.get("authentication") else {
        return Ok(());
    };
    if auth.is_null() {
        return Ok(());
    }
    let object = auth
        .as_object()
        .context("Insomnia authentication must be an object")?;
    if object.is_empty() {
        return Ok(());
    }
    let kind = field(auth, "type")?;
    if kind == "none" {
        ensure!(object.len() == 1, "Unsupported Insomnia no-auth settings");
        return Ok(());
    }
    ensure!(
        !request
            .headers
            .iter()
            .any(|row| row.enabled && row.key.eq_ignore_ascii_case("authorization")),
        "Insomnia auth conflicts with explicit Authorization header"
    );
    match kind {
        "basic" => {
            ensure!(
                object
                    .keys()
                    .all(|key| matches!(key.as_str(), "type" | "username" | "password")),
                "Unsupported Insomnia Basic auth settings"
            );
            let username = field(auth, "username")?;
            let password = field(auth, "password")?;
            literal(username)?;
            literal(password)?;
            request.auth.kind = "basic".into();
            request.auth.username = username.into();
            request.auth.password = password.into();
        }
        "bearer" => {
            ensure!(
                object
                    .keys()
                    .all(|key| matches!(key.as_str(), "type" | "token")),
                "Unsupported Insomnia Bearer auth settings"
            );
            let token = field(auth, "token")?;
            literal(token)?;
            request.auth.kind = "bearer".into();
            request.auth.token = token.into();
        }
        kind => bail!("Unsupported Insomnia authentication type {kind}"),
    }
    Ok(())
}

fn read_body(source: &Value, request: &mut moleapi_core::RequestSpec) -> Result<()> {
    let Some(body) = source.get("body") else {
        return Ok(());
    };
    if body.is_null() {
        return Ok(());
    }
    let object = body
        .as_object()
        .context("Insomnia body must be an object")?;
    if object.is_empty() {
        return Ok(());
    }
    ensure!(
        object
            .keys()
            .all(|key| matches!(key.as_str(), "mimeType" | "text" | "params")),
        "Unsupported Insomnia body source or file reference"
    );
    let mime = optional_string(body, "mimeType")?.unwrap_or("text/plain");
    literal(mime)?;
    let essence = mime
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let text = optional_string(body, "text")?;
    let params = rows(body, "params")?;
    let value = match essence.as_str() {
        "application/json" | "application/problem+json" => {
            ensure!(
                params.is_empty(),
                "Insomnia JSON body cannot have form parameters"
            );
            let value = text.context("Insomnia JSON body text is missing")?;
            literal(value)?;
            serde_json::from_str::<Value>(value).context("Invalid Insomnia JSON body")?;
            request.body_kind = "json".into();
            value.to_owned()
        }
        "application/x-www-form-urlencoded" => {
            let mut serializer = form_urlencoded::Serializer::new(String::new());
            for row in params {
                ensure!(
                    row.get("fileName").is_none() && row["type"] != "file",
                    "Insomnia form file bytes are unavailable"
                );
                if !enabled(row)? {
                    continue;
                }
                let name = field(row, "name")?;
                let value = field(row, "value")?;
                literal(name)?;
                literal(value)?;
                serializer.append_pair(name, value);
            }
            let encoded = serializer.finish();
            let value = match text {
                Some(text) => {
                    literal(text)?;
                    ensure!(
                        encoded.is_empty() || text == encoded,
                        "Insomnia form text and parameters disagree"
                    );
                    text.to_owned()
                }
                None => encoded,
            };
            request.body_kind = "form".into();
            value
        }
        kind if kind.starts_with("text/") || kind == "application/xml" => {
            ensure!(
                params.is_empty(),
                "Insomnia text body cannot have form parameters"
            );
            let value = text.context("Insomnia text body is missing")?;
            literal(value)?;
            request.body_kind = "text".into();
            value.to_owned()
        }
        _ => bail!("Unsupported Insomnia body MIME type {mime}"),
    };
    ensure!(
        value.len() <= moleapi_core::MAX_BODY,
        "Insomnia body exceeds 5 MiB"
    );
    request.body = value;
    if !request
        .headers
        .iter()
        .any(|row| row.enabled && row.key.eq_ignore_ascii_case("content-type"))
    {
        request.headers.push(pair("Content-Type", mime));
    }
    Ok(())
}
