use crate::{ENGINE, generate_har};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use moleapi_core::{Protocol, Workspace};
use serde::Serialize;
use serde_json::json;
#[derive(Debug, Serialize)]
pub struct Snippet {
    pub engine: &'static str,
    pub target: String,
    pub client: String,
    pub code: String,
    pub warnings: Vec<String>,
    pub include_secrets: bool,
}
pub fn generate_request(
    workspace: &Workspace,
    request_id: &str,
    target: &str,
    client: &str,
    include_secrets: bool,
) -> Result<Snippet> {
    let mut request = moleapi_formats::generation_request(workspace, request_id, include_secrets)?;
    moleapi_core::validate_structured_body(&request, true)?;
    ensure!(
        matches!(request.protocol, Protocol::Http),
        "Request snippets currently require an HTTP request"
    );
    if request.auth.kind == "apikey" {
        let key = request
            .auth
            .api_key
            .as_ref()
            .context("API key settings missing")?;
        if key.location == moleapi_core::AuthLocation::Query {
            // Preserve unresolved URL templates; the mature form decoder owns query encoding.
            let (without_fragment, fragment) = request
                .url
                .split_once('#')
                .map(|(base, part)| (base, Some(part)))
                .unwrap_or((&request.url, None));
            if let Some((base, query)) = without_fragment.split_once('?') {
                let retained = url::form_urlencoded::parse(query.as_bytes())
                    .filter(|(name, _)| name != &key.name)
                    .collect::<Vec<_>>();
                let query = url::form_urlencoded::Serializer::new(String::new())
                    .extend_pairs(retained.iter().map(|(k, v)| (k.as_ref(), v.as_ref())))
                    .finish();
                request.url = format!(
                    "{base}{}{}",
                    if query.is_empty() {
                        String::new()
                    } else {
                        format!("?{query}")
                    },
                    fragment.map(|f| format!("#{f}")).unwrap_or_default()
                );
            }
        }
        let rows = if key.location == moleapi_core::AuthLocation::Query {
            &mut request.query
        } else {
            &mut request.headers
        };
        rows.retain(|row| {
            if key.location == moleapi_core::AuthLocation::Header {
                !row.key.eq_ignore_ascii_case(&key.name)
            } else {
                row.key != key.name
            }
        });
        rows.push(moleapi_core::Pair {
            id: "selected-api-key".into(),
            key: key.name.clone(),
            value: key.value.clone(),
            enabled: true,
            secret: Some(true),
            local_value: None,
        });
        request.auth.kind = "none".into();
    }
    let mut url = request.url.clone();
    let query = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(
            request
                .query
                .iter()
                .filter(|p| p.enabled && !p.key.is_empty())
                .map(|p| (&p.key, &p.value)),
        )
        .finish();
    if !query.is_empty() {
        let (base, fragment) = url
            .split_once('#')
            .map(|(a, b)| (a.to_owned(), Some(b.to_owned())))
            .unwrap_or((url.clone(), None));
        url = format!(
            "{base}{}{query}",
            if base.contains('?') { "&" } else { "?" }
        );
        if let Some(fragment) = fragment {
            url.push('#');
            url.push_str(&fragment);
        }
    }
    if target == "python" && client == "python3" {
        ensure!(
            url::Url::parse(&url)
                .is_ok_and(|u| matches!(u.scheme(), "http" | "https") && u.host_str().is_some()),
            "Python/http.client requires a concrete absolute HTTP URL; replace the base URL template first"
        );
    }
    let mut headers = Vec::new();
    let mut names = std::collections::BTreeSet::new();
    for row in request
        .headers
        .iter()
        .filter(|p| p.enabled && !p.key.is_empty())
    {
        let key = row.key.to_ascii_lowercase();
        if [
            "host",
            "content-length",
            "transfer-encoding",
            "connection",
            "proxy-authorization",
            "proxy-connection",
            "upgrade",
            "trailer",
            "te",
        ]
        .contains(&key.as_str())
            || (key == "authorization" && request.auth.kind != "none")
        {
            continue;
        }
        ensure!(
            names.insert(key) || (target == "shell" && client == "curl"),
            "Repeated headers are not supported by this snippet adapter; use shell/curl"
        );
        headers.push(json!({"name":row.key,"value":row.value}));
    }
    if request.body_kind == "form" && !(target == "shell" && client == "curl") {
        let mut names = std::collections::BTreeSet::new();
        for (name, _) in url::form_urlencoded::parse(request.body.as_bytes()) {
            ensure!(
                names.insert(name.into_owned()),
                "Repeated form fields are not supported by this snippet adapter; use shell/curl"
            );
        }
    }
    ensure!(
        !(matches!(target, "js" | "node")
            && matches!(client, "fetch" | "ofetch" | "undici")
            && matches!(request.method.as_str(), "GET" | "HEAD")
            && request.body_kind != "none"),
        "Fetch adapters cannot send GET/HEAD bodies; choose a different HTTP library"
    );
    match request.auth.kind.as_str() {
        "none" => {},
        "bearer" => headers.push(json!({"name":"Authorization","value":format!("Bearer {}",request.auth.token)})),
        "basic" => headers.push(json!({"name":"Authorization","value":format!("Basic {}",STANDARD.encode(format!("{}:{}",request.auth.username,request.auth.password)))})),
        _ => anyhow::bail!("This authentication mode is not supported by request snippets"),
    }
    // Match shared transport content-type defaults; several generators require
    // the explicit header rather than inferring it from HAR postData.
    if (request.body_kind == "json" || request.body_kind == "form")
        && !headers.iter().any(|h| {
            h["name"]
                .as_str()
                .is_some_and(|s| s.eq_ignore_ascii_case("content-type"))
        })
    {
        headers.push(json!({"name":"Content-Type","value":if request.body_kind=="json"{"application/json"}else{"application/x-www-form-urlencoded"}}));
    }
    // Old upstream adapters interpolate the URL into single-quoted literals.
    // Reject these drafts instead of emitting executable source with broken quoting.
    ensure!(
        !url.contains(['\'', '\"', '\\', '\n', '\r']),
        "Snippet URL contains characters unsupported by upstream adapters; percent-encode them first"
    );
    let post_data = match request.body_kind.as_str() {
        "none" => serde_json::Value::Null,
        "json" | "text" => {
            json!({"mimeType":if request.body_kind=="json"{"application/json"}else{"text/plain"},"text":request.body})
        }
        "form" => {
            json!({"mimeType":"application/x-www-form-urlencoded","params":url::form_urlencoded::parse(request.body.as_bytes()).map(|(name,value)|json!({"name":name,"value":value})).collect::<Vec<_>>()})
        }
        "multipart" => {
            let body: moleapi_core::MultipartBody = serde_json::from_str(&request.body)?;
            let params = body
                .parts
                .into_iter()
                .filter(|part| part.enabled)
                .map(|part| match part.value {
                    moleapi_core::MultipartValue::Text { text, mime } => {
                        let mut value = json!({"name":part.name,"value":text});
                        if !mime.is_empty() {
                            value["contentType"] = mime.into();
                        }
                        value
                    }
                    moleapi_core::MultipartValue::File { file } => {
                        let mut value = json!({"name":part.name,"fileName":file.file_name});
                        if !file.mime.is_empty() {
                            value["contentType"] = file.mime.into();
                        }
                        value
                    }
                })
                .collect::<Vec<_>>();
            json!({"mimeType":"multipart/form-data","params":params})
        }
        "binary" => {
            let file: moleapi_core::BinaryBody = serde_json::from_str(&request.body)?;
            let mime = if let Some(value) = headers
                .iter()
                .find(|header| {
                    header["name"]
                        .as_str()
                        .is_some_and(|name| name.eq_ignore_ascii_case("content-type"))
                })
                .and_then(|header| header["value"].as_str())
            {
                value.to_string()
            } else if file.mime.is_empty() {
                "application/octet-stream".into()
            } else {
                file.mime
            };
            let name = if file.file_name.is_empty() {
                "{{FILE_NAME}}".into()
            } else {
                file.file_name
            };
            json!({"mimeType":mime,"fileName":name})
        }
        _ => anyhow::bail!("This body mode is not supported by request snippets"),
    };
    if request.body_kind == "binary"
        && !headers.iter().any(|header| {
            header["name"]
                .as_str()
                .is_some_and(|name| name.eq_ignore_ascii_case("content-type"))
        })
    {
        headers.push(json!({"name":"Content-Type","value":post_data["mimeType"]}));
    }
    let har = json!({"method":request.method,"url":url,"httpVersion":"HTTP/1.1","headers":headers,"queryString":[],"cookies":[],"postData":post_data});
    let code = generate_har(target, client, &har)?;
    let mut warnings = vec![
        "生成静态请求示例；不执行脚本、不解析环境变量、不附加浏览器私有覆盖。请检查占位符。".into(),
        "各语言的超时、TLS、重定向策略由其 HTTP 库决定；此轮示例尚不映射这些设置。".into(),
    ];
    if matches!(request.body_kind.as_str(), "multipart" | "binary") {
        warnings.push("文件上传代码引用文件名，不嵌入保存的文件字节。运行代码前请在目标环境准备对应文件，并检查库的 MIME 类型和文件名处理。".into());
    }
    Ok(Snippet {
        engine: ENGINE,
        target: target.into(),
        client: client.into(),
        code,
        warnings,
        include_secrets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raw_binary_catalog_generates_file_readers_without_embedding_saved_bytes() {
        let body =
            json!({"file_name":"upload.bin","mime":"application/octet-stream","base64":"AP+AClg="})
                .to_string();
        let w = workspace("binary", &body);
        let mut count = 0;
        for target in crate::catalog().unwrap() {
            for client in target
                .clients
                .into_iter()
                .filter(|client| client.binary_file)
            {
                let code = generate_request(&w, "r", &target.target, &client.client, false)
                    .unwrap()
                    .code;
                assert!(
                    code.contains("upload.bin")
                        && !code.contains("AP+AClg=")
                        && !code.contains("<file contents here>"),
                    "{}/{}: {code}",
                    target.target,
                    client.client
                );
                count += 1;
            }
        }
        assert_eq!(count, 12);
        assert_eq!(w.data.collections[0].requests[0].body, body);
    }
    #[test]
    fn multipart_catalog_uses_mature_emitters_for_every_selected_library() {
        let body=json!({"parts":[{"id":"label","name":"label","value":{"kind":"text","text":"fixture","mime":""}},{"id":"upload","name":"upload","value":{"kind":"file","file":{"file_name":"upload.bin","mime":"application/octet-stream","base64":null}}}]}).to_string();
        let w = workspace("multipart", &body);
        for target in crate::catalog().unwrap() {
            for client in target.clients {
                let result = generate_request(&w, "r", &target.target, &client.client, true);
                assert!(
                    result.is_ok(),
                    "{}/{}: {result:?}",
                    target.target,
                    client.client
                );
                assert!(!result.unwrap().code.is_empty());
            }
        }
    }
    #[test]
    fn multipart_generation_preserves_structure_and_default_privacy_without_file_bytes() {
        let body=json!({"parts":[{"id":"secret","name":"password","value":{"kind":"text","text":"file","mime":""}},{"id":"copy","name":"copy","value":{"kind":"text","text":"file","mime":""}},{"id":"upload","name":"upload","value":{"kind":"file","file":{"file_name":"upload.bin","mime":"application/octet-stream","base64":"AP+AClg="}}},{"id":"off","name":"disabled","enabled":false,"value":{"kind":"text","text":"must-not-send","mime":""}}]}).to_string();
        let w = workspace("multipart", &body);
        let selected = moleapi_formats::generation_request(&w, "r", false).unwrap();
        let parsed: moleapi_core::MultipartBody = serde_json::from_str(&selected.body).unwrap();
        assert!(
            matches!(&parsed.parts[2].value,moleapi_core::MultipartValue::File{file} if file.base64.is_none())
        );
        assert!(
            matches!(&parsed.parts[1].value,moleapi_core::MultipartValue::Text{text,..} if text!="file")
        );
        for (target, client) in [
            ("shell", "curl"),
            ("node", "fetch"),
            ("go", "native"),
            ("csharp", "httpclient"),
            ("python", "requests"),
            ("node", "native"),
            ("node", "request"),
            ("node", "unirest"),
        ] {
            let result = generate_request(&w, "r", target, client, false).unwrap();
            assert!(
                !result.code.contains("AP+AClg=") && !result.code.contains("must-not-send"),
                "{target}/{client}: {}",
                result.code
            );
            assert!(result.code.contains("upload.bin"));
        }
        assert_eq!(w.data.collections[0].requests[0].body, body);
    }
    fn workspace(body_kind: &str, body: &str) -> Workspace {
        serde_json::from_value(json!({"id":"w","name":"Fixture","revision":1,"updated_at":"now","data":{"schema_version":1,"collections":[{"id":"c","name":"Collection","description":"","requests":[{"id":"r","name":"Request","method":"POST","url":"http://127.0.0.1/echo?original=one","description":"","query":[{"id":"q","key":"q","value":"space & + unicode 鼹鼠","enabled":true}],"headers":[],"body_kind":body_kind,"body":body,"auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":false,"verify_tls":true,"assertions":[],"examples":[]}]}],"environments":[],"active_environment_id":null}})).unwrap()
    }
    #[test]
    fn api_key_snippet_replaces_url_query_values_without_resolving_templates() {
        let mut w = workspace("none", "");
        let r = &mut w.data.collections[0].requests[0];
        r.url = "{{base_url}}/echo?access=stale&ac%63ess=old&keep=yes#fragment".into();
        r.auth.kind = "apikey".into();
        r.auth.api_key = Some(Box::new(moleapi_core::ApiKeyAuth {
            name: "access".into(),
            value: "selected".into(),
            location: moleapi_core::AuthLocation::Query,
        }));
        let snippet = generate_request(&w, "r", "shell", "curl", true).unwrap();
        assert!(!snippet.code.contains("stale") && !snippet.code.contains("old"));
        assert!(snippet.code.contains("access=selected") && snippet.code.contains("keep=yes"));
        assert!(snippet.code.contains("{{base_url}}"));
        let private = generate_request(&w, "r", "shell", "curl", false).unwrap();
        assert!(!private.code.contains("selected"));
    }
    #[test]
    fn form_private_fields_and_copies_are_screened_after_decoding() {
        let w = workspace(
            "form",
            "password=private%2B%2F+secret&copy=private%2B%2F+secret&x=one&x=two",
        );
        let snippet = generate_request(&w, "r", "shell", "curl", false).unwrap();
        assert!(!snippet.code.contains("private"));
        assert!(snippet.code.contains("one") && snippet.code.contains("two"));
        let full = generate_request(&w, "r", "shell", "curl", true).unwrap();
        assert!(full.code.contains("private"));
    }
    #[test]
    fn templated_url_queries_and_combined_basic_copies_do_not_leak() {
        let mut w = workspace("json", "{\"copy\":\"private-query-seed\"}");
        let request = &mut w.data.collections[0].requests[0];
        request.url = "{{base_url}}/echo?token=private-query-seed".into();
        moleapi_core::validate_workspace(&w.data).unwrap();
        let snippet = generate_request(&w, "r", "node", "fetch", false).unwrap();
        assert!(
            !snippet.code.contains("private-query-seed"),
            "{}",
            snippet.code
        );
        let request = &mut w.data.collections[0].requests[0];
        request.url = "https://example.com/echo".into();
        request.auth.kind = "basic".into();
        request.auth.username = "loginabcde".into();
        request.auth.password = "super+secret".into();
        let encoded = STANDARD.encode("loginabcde:super+secret");
        request.body = json!({"copy":encoded}).to_string();
        moleapi_core::validate_workspace(&w.data).unwrap();
        let snippet = generate_request(&w, "r", "node", "fetch", false).unwrap();
        assert!(!snippet.code.contains(&encoded), "{}", snippet.code);
    }
    #[test]
    fn unsupported_repeated_fields_fail_instead_of_losing_values() {
        let mut w = workspace("form", "x=one&x=two");
        for (target, client) in [("js", "fetch"), ("node", "fetch"), ("go", "native")] {
            assert!(
                generate_request(&w, "r", target, client, true)
                    .unwrap_err()
                    .to_string()
                    .contains("Repeated form fields")
            );
        }
        w.data.collections[0].requests[0].body_kind = "none".into();
        let mut row = moleapi_core::Pair {
            id: "header".into(),
            key: "X-Test".into(),
            value: "one".into(),
            enabled: true,
            secret: None,
            local_value: None,
        };
        w.data.collections[0].requests[0].headers.push(row.clone());
        row.id = "second".into();
        row.key = "x-test".into();
        row.value = "two".into();
        w.data.collections[0].requests[0].headers.push(row);
        assert!(
            generate_request(&w, "r", "node", "fetch", true)
                .unwrap_err()
                .to_string()
                .contains("Repeated headers")
        );
    }
    #[test]
    fn selected_auth_replaces_manual_header_and_transport_headers_are_omitted() {
        let mut w = workspace("json", "{\"hello\":\"world\"}");
        let request = &mut w.data.collections[0].requests[0];
        request.auth.kind = "bearer".into();
        request.auth.token = "selected-token".into();
        for (id, key, value) in [
            ("auth", "authorization", "manual-token"),
            ("host", "Host", "wrong.invalid"),
            ("length", "Content-Length", "999"),
        ] {
            request.headers.push(moleapi_core::Pair {
                id: id.into(),
                key: key.into(),
                value: value.into(),
                enabled: true,
                secret: None,
                local_value: None,
            });
        }
        let code = generate_request(&w, "r", "shell", "curl", true)
            .unwrap()
            .code;
        assert!(code.contains("selected-token"));
        assert!(!code.contains("manual-token"));
        assert!(!code.contains("wrong.invalid"));
        assert!(!code.contains("Content-Length"));
    }
    #[test]
    fn ordinary_credentials_cannot_redact_structural_request_keys() {
        let mut w = workspace("json", "{\"hello\":\"world\"}");
        w.data.collections[0].requests[0].auth.kind = "basic".into();
        w.data.collections[0].requests[0].auth.username = "user".into();
        w.data.collections[0].requests[0].auth.password = "password".into();
        moleapi_core::validate_workspace(&w.data).unwrap();
        let encoded = STANDARD.encode("old-username:password");
        w.data.collections[0].requests[0].body = json!({"copy":encoded}).to_string();
        let snippet = generate_request(&w, "r", "node", "fetch", false).unwrap();
        assert!(!snippet.code.contains(&encoded));
        w.data.collections[0].requests[0].url = format!("https://example.com/echo?copy={encoded}");
        w.data.collections[0].requests[0].body_kind = "form".into();
        w.data.collections[0].requests[0].body = format!("copy={encoded}");
        let snippet = generate_request(&w, "r", "node", "fetch", false).unwrap();
        assert!(!snippet.code.contains(&encoded));
    }
    #[test]
    fn parsed_url_adapters_do_not_invent_a_host_for_base_templates() {
        let mut w = workspace("none", "");
        w.data.collections[0].requests[0].url = "{{base_url}}/echo".into();
        assert!(
            generate_request(&w, "r", "python", "python3", false)
                .unwrap_err()
                .to_string()
                .contains("concrete absolute HTTP URL")
        );
        assert!(
            generate_request(&w, "r", "node", "fetch", false)
                .unwrap()
                .code
                .contains("{{base_url}}")
        );
    }
    #[test]
    fn auth_templates_are_preserved_and_unsupported_protocols_fail() {
        let mut w = workspace("json", "{\"hello\":\"world\"}");
        let request = &mut w.data.collections[0].requests[0];
        request.auth.kind = "bearer".into();
        request.auth.token = "{{selected_token}}".into();
        let snippet = generate_request(&w, "r", "node", "fetch", false).unwrap();
        assert!(snippet.code.contains("{{selected_token}}"));
        w.data.collections[0].requests[0].protocol = Protocol::Sse;
        assert!(generate_request(&w, "r", "node", "fetch", false).is_err());
    }
}
