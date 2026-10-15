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
            || mode.as_deref() == Some("java")
                && ["form", "text", "binary", "binary-mime", "multipart"]
                    .contains(&profile.as_str()),
        "Unknown snippet fixture profile"
    );
    let body_profile = if mode.as_deref() == Some("java") {
        profile.as_str()
    } else {
        mode.as_deref().unwrap_or("core")
    };
    if body_profile == "form" {
        let request = &mut workspace.data.collections[0].requests[0];
        request.body_kind = "form".into();
        request.body = "label=%E9%BC%B9%E9%BC%A0+%22quotes%22+%26+%2B&label=second&empty=".into();
    }
    if body_profile == "text" {
        let request = &mut workspace.data.collections[0].requests[0];
        request.body_kind = "text".into();
        request.body = "鼹鼠 \"quotes\" & +\r\nline\0\u{b}end".into();
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
    let targets = if mode.as_deref() == Some("java") {
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
    for (target, client, filename) in targets {
        let code = generate_request(&workspace, "r", target, client, true)?.code;
        hashes.insert(
            filename.into(),
            format!("{:x}", Sha256::digest(code.as_bytes())).into(),
        );
        std::fs::write(std::path::Path::new(&output).join(filename), code)?;
    }
    if mode.as_deref() == Some("java") {
        let source = serde_json::to_vec(&workspace)?;
        std::fs::write(
            std::path::Path::new(&output).join("fixture-request.json"),
            &source,
        )?;
        std::fs::write(
            std::path::Path::new(&output).join("fixture-manifest.json"),
            serde_json::to_vec_pretty(
                &json!({"engine":moleapi_generation::ENGINE,"target":"java","profile":profile,"source_sha256":format!("{:x}",Sha256::digest(source)),"files":hashes}),
            )?,
        )?;
    }
    Ok(())
}
