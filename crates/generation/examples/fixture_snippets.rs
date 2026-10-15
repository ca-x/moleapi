//! Generate trusted fixture snippets for explicit compile/run checks, never a server action.
use anyhow::{Result, ensure};
use moleapi_generation::generate_request;
use serde_json::json;
use sha2::{Digest, Sha256};
fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let output = args.next().expect("output directory");
    let url = args.next().expect("fixture URL");
    std::fs::create_dir_all(&output)?;
    let mut workspace: moleapi_core::Workspace = serde_json::from_value(
        json!({"id":"w","name":"Fixture","revision":1,"updated_at":"now","data":{"schema_version":1,"collections":[{"id":"c","name":"Fixture","description":"","requests":[{"id":"r","name":"Fixture","method":"POST","url":format!("{url}/echo?existing=one"),"description":"","query":[{"id":"q","key":"q","value":"space & +","enabled":true}],"headers":[{"id":"h","key":"X-Test","value":"moleapi-fixture","enabled":true},{"id":"auth","key":"Authorization","value":"Bearer ignored-manual-token","enabled":true},{"id":"host","key":"Host","value":"wrong.invalid","enabled":true}],"body_kind":"json","body":"{\"message\":\"鼹鼠 \\\"quotes\\\" & +\"}","auth":{"kind":"bearer","token":"fixture-token","username":"","password":""},"timeout_ms":1000,"follow_redirects":false,"verify_tls":true,"assertions":[],"examples":[]}]}],"environments":[],"active_environment_id":null}}),
    )?;
    let mode = args.next();
    let profile = args.next().unwrap_or_else(|| "core".into());
    ensure!(args.next().is_none(), "Unexpected fixture arguments");
    ensure!(
        profile == "core"
            || matches!(
                mode.as_deref(),
                Some("java" | "kotlin" | "dart" | "ruby" | "python" | "php" | "node")
            ) && ["form", "text", "binary", "binary-mime", "multipart"]
                .contains(&profile.as_str())
            || matches!(
                mode.as_deref(),
                Some("kotlin" | "dart" | "ruby" | "python" | "php" | "node")
            ) && profile == "dollar"
            || mode.as_deref() == Some("ruby") && profile == "interpolation"
            || matches!(mode.as_deref(), Some("python" | "php" | "node"))
                && [
                    "json-values",
                    "json-scalar",
                    "json-null",
                    "json-empty",
                    "get-binary",
                    "head-binary",
                    "delete-json",
                    "text-json",
                    "multipart-meta",
                    "multipart-text",
                    "json-object",
                    "headers-quoted"
                ]
                .contains(&profile.as_str()),
        "Unknown snippet fixture profile"
    );
    let body_profile = if matches!(
        mode.as_deref(),
        Some("java" | "kotlin" | "dart" | "ruby" | "python" | "php" | "node")
    ) {
        if matches!(profile.as_str(), "get-binary" | "head-binary") {
            "binary"
        } else if profile == "delete-json" {
            "core"
        } else {
            profile.as_str()
        }
    } else {
        mode.as_deref().unwrap_or("core")
    };
    if profile == "get-binary" {
        workspace.data.collections[0].requests[0].method = "GET".into();
    }
    if profile == "head-binary" {
        workspace.data.collections[0].requests[0].method = "HEAD".into();
    }
    if profile == "delete-json" {
        workspace.data.collections[0].requests[0].method = "DELETE".into();
    }
    if body_profile == "form" {
        let request = &mut workspace.data.collections[0].requests[0];
        request.body_kind = "form".into();
        request.body = "label=%E9%BC%B9%E9%BC%A0+%22quotes%22+%26+%2B&label=second&empty=".into();
    }
    if body_profile == "text-json" {
        workspace.data.collections[0].requests[0].body_kind = "text".into();
        workspace.data.collections[0].requests[0].body = "{\"b\": 1, \"a\":2}".into();
    }
    if body_profile == "text" {
        let request = &mut workspace.data.collections[0].requests[0];
        request.body_kind = "text".into();
        request.body = "鼹鼠 \"quotes\" & +\r\nline\0\u{b}end".into();
    }
    if body_profile == "dollar" {
        let request = &mut workspace.data.collections[0].requests[0];
        request.body_kind = "text".into();
        request.body = "literal $ and ${1 + 2} 鼹鼠".into();
    }
    if body_profile == "interpolation" {
        let request = &mut workspace.data.collections[0].requests[0];
        request.body_kind = "json".into();
        request.body = json!({"message":"literal #{1 + 2} #@value #$value"}).to_string();
    }
    if body_profile == "json-object" {
        workspace.data.collections[0].requests[0].body = "{}".into();
    }
    if body_profile == "headers-quoted" {
        workspace.data.collections[0].requests[0]
            .headers
            .push(moleapi_core::Pair {
                id: "quoted".into(),
                key: "X-Fixture'Key".into(),
                value: "literal ' \\ $fixture".into(),
                enabled: true,
                secret: None,
                local_value: None,
            });
    }
    if body_profile == "json-values" {
        workspace.data.collections[0].requests[0].body = json!({"yes":true,"no":false,"nil":null,"list":[true,false,null],"strings":["x: true, text","x: false, text","x: null, text","\"true\"", "\\false"]}).to_string();
    }
    if body_profile == "json-scalar" {
        workspace.data.collections[0].requests[0].body = "false".into();
    }
    if body_profile == "json-null" {
        workspace.data.collections[0].requests[0].body = "null".into();
    }
    if body_profile == "json-empty" {
        workspace.data.collections[0].requests[0].body = "\"\"".into();
    }
    if matches!(body_profile, "binary" | "binary-mime") {
        let request = &mut workspace.data.collections[0].requests[0];
        request.body_kind = "binary".into();
        request.body =
            json!({"file_name":"upload.bin","mime":"application/octet-stream","base64":"AP+AClg="})
                .to_string();
        if body_profile == "binary-mime" {
            request.headers.push(moleapi_core::Pair {
                id: "mime".into(),
                key: "Content-Type".into(),
                value: "application/vnd.moleapi.fixture".into(),
                enabled: true,
                secret: None,
                local_value: None,
            });
        }
    }
    if body_profile == "multipart" {
        let request = &mut workspace.data.collections[0].requests[0];
        request.body_kind = "multipart".into();
        request.body=json!({"parts":[{"id":"one","name":"label","value":{"kind":"text","text":"鼹鼠 'quotes' & +","mime":""}},{"id":"two","name":"label","value":{"kind":"text","text":"second","mime":""}},{"id":"upload","name":"upload","value":{"kind":"file","file":{"file_name":"upload.bin","mime":"application/octet-stream","base64":"AP+AClg="}}},{"id":"off","name":"disabled","enabled":false,"value":{"kind":"text","text":"must-not-send","mime":""}}]}).to_string();
    }
    if body_profile == "multipart-meta" {
        let request = &mut workspace.data.collections[0].requests[0];
        request.body_kind = "multipart".into();
        request.body = json!({"parts": [{"id": "text", "name": "metadata", "value": {"kind": "text", "text": "鼹鼠 \"quotes\"\r\nvalue", "mime": "text/x-moleapi-field"}}, {"id": "file", "name": "upload", "value": {"kind": "file", "file": {"file_name": "upload-鼹鼠.bin", "mime": "application/x-moleapi-file", "base64": "AP+AClg="}}}]}).to_string();
    }
    if body_profile == "multipart-text" {
        workspace.data.collections[0].requests[0].body_kind = "multipart".into();
        workspace.data.collections[0].requests[0].body = json!({"parts": [{"id": "one", "name": "label", "value": {"kind": "text", "text": "鼹鼠 \"quotes\"\r\nvalue", "mime": ""}}, {"id": "two", "name": "label", "value": {"kind": "text", "text": "second", "mime": ""}}, {"id": "empty", "name": "empty", "value": {"kind": "text", "text": "", "mime": ""}}, {"id": "disabled", "name": "off", "enabled": false, "value": {"kind": "text", "text": "must-not-send", "mime": ""}}]}).to_string();
    }
    if matches!(mode.as_deref(), Some("php" | "node")) && body_profile == "multipart-meta" {
        let request = &mut workspace.data.collections[0].requests[0];
        request.body = request
            .body
            .replace("upload-鼹鼠.bin", "upload-鼹鼠'quote.bin");
    }
    let targets = if mode.as_deref() == Some("node") {
        vec![
            ("node", "fetch", "fetch.mjs"),
            ("node", "axios", "axios.mjs"),
            ("node", "ofetch", "ofetch.mjs"),
            ("node", "undici", "undici.mjs"),
        ]
    } else if mode.as_deref() == Some("php") {
        vec![
            ("php", "curl", "curl.php"),
            ("php", "guzzle", "guzzle.php"),
            ("php", "laravel", "laravel.php"),
            ("php", "httprequest2", "httprequest2.php"),
            ("php", "pecl_http", "pecl_http.php"),
        ]
    } else if mode.as_deref() == Some("python") {
        vec![
            ("python", "python3", "python3.py"),
            ("python", "aiohttp", "aiohttp.py"),
            ("python", "requests", "requests.py"),
            ("python", "httpx_sync", "httpx_sync.py"),
            ("python", "httpx_async", "httpx_async.py"),
        ]
    } else if mode.as_deref() == Some("ruby") {
        vec![("ruby", "native", "snippet.rb")]
    } else if mode.as_deref() == Some("dart") {
        vec![("dart", "http", "http.dart"), ("dart", "dio", "dio.dart")]
    } else if mode.as_deref() == Some("kotlin") {
        vec![("kotlin", "okhttp", "snippet.kt")]
    } else if mode.as_deref() == Some("java") {
        vec![
            ("java", "nethttp", "nethttp.java"),
            ("java", "okhttp", "okhttp.java"),
            ("java", "asynchttp", "asynchttp.java"),
            ("java", "unirest", "unirest.java"),
        ]
    } else if mode.as_deref() == Some("csharp") {
        vec![
            ("csharp", "httpclient", "httpclient.cs"),
            ("csharp", "restsharp", "restsharp.cs"),
        ]
    } else if mode.as_deref() == Some("parity") {
        vec![
            ("node", "native", "native.cjs"),
            ("node", "request", "request.cjs"),
            ("node", "unirest", "unirest.cjs"),
            ("dart", "dio", "dio.dart"),
            ("php", "httprequest2", "httprequest2.php"),
            ("php", "pecl_http", "pecl_http.php"),
            ("r", "httr", "httr.R"),
            ("r", "rcurl", "rcurl.R"),
            ("shell", "curl_windows", "curl.cmd"),
        ]
    } else if mode.as_deref() == Some("binary") {
        vec![
            ("node", "fetch", "node.mjs"),
            ("go", "native", "main.go"),
            ("shell", "curl", "curl.sh"),
            ("shell", "wget", "wget.sh"),
            ("python", "requests", "python.py"),
            ("csharp", "httpclient", "httpclient.cs"),
            ("csharp", "restsharp", "restsharp.cs"),
            ("rust", "reqwest", "rust.rs"),
            ("kotlin", "okhttp", "kotlin.kt"),
        ]
    } else if mode.as_deref() == Some("multipart") {
        vec![
            ("node", "fetch", "node.mjs"),
            ("node", "native", "native.cjs"),
            ("node", "request", "request.cjs"),
            ("node", "unirest", "unirest.cjs"),
            ("go", "native", "main.go"),
            ("shell", "curl", "curl.sh"),
            ("python", "requests", "python.py"),
            ("csharp", "httpclient", "httpclient.cs"),
            ("csharp", "restsharp", "restsharp.cs"),
        ]
    } else {
        vec![
            ("node", "fetch", "node.mjs"),
            ("js", "fetch", "browser.mjs"),
            ("python", "python3", "python.py"),
            ("go", "native", "main.go"),
            ("shell", "curl", "curl.sh"),
            ("rust", "reqwest", "rust.rs"),
            ("c", "libcurl", "c.c"),
        ]
    };
    let mut hashes = serde_json::Map::new();
    let mut rejections = serde_json::Map::new();
    for (target, client, filename) in targets {
        let result = generate_request(&workspace, "r", target, client, true);
        if mode.as_deref() == Some("node")
            && matches!(profile.as_str(), "get-binary" | "head-binary")
            && matches!(client, "fetch" | "ofetch")
        {
            let error = result.unwrap_err().to_string();
            ensure!(error.contains("cannot send GET/HEAD bodies"));
            rejections.insert(client.into(), error.into());
            continue;
        }
        let code = result?.code;
        hashes.insert(
            filename.into(),
            format!("{:x}", Sha256::digest(code.as_bytes())).into(),
        );
        std::fs::write(std::path::Path::new(&output).join(filename), code)?;
    }
    if matches!(
        mode.as_deref(),
        Some("java" | "kotlin" | "dart" | "ruby" | "python" | "php" | "node")
    ) {
        let source = serde_json::to_vec(&workspace)?;
        std::fs::write(
            std::path::Path::new(&output).join("fixture-request.json"),
            &source,
        )?;
        std::fs::write(
            std::path::Path::new(&output).join("fixture-manifest.json"),
            serde_json::to_vec_pretty(
                &json!({"engine":moleapi_generation::ENGINE,"target":mode,"profile":profile,"source_sha256":format!("{:x}",Sha256::digest(source)),"files":hashes,"rejections":rejections}),
            )?,
        )?;
    }
    Ok(())
}
