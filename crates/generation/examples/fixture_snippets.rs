//! Generate trusted fixture snippets for explicit compile/run checks, never a server action.
use anyhow::Result;
use moleapi_generation::generate_request;
use serde_json::json;
fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let output = args.next().expect("output directory");
    let url = args.next().expect("fixture URL");
    std::fs::create_dir_all(&output)?;
    let workspace: moleapi_core::Workspace = serde_json::from_value(
        json!({"id":"w","name":"Fixture","revision":1,"updated_at":"now","data":{"schema_version":1,"collections":[{"id":"c","name":"Fixture","description":"","requests":[{"id":"r","name":"Fixture","method":"POST","url":format!("{url}/echo?existing=one"),"description":"","query":[{"id":"q","key":"q","value":"space & +","enabled":true}],"headers":[{"id":"h","key":"X-Test","value":"moleapi-fixture","enabled":true},{"id":"auth","key":"Authorization","value":"Bearer ignored-manual-token","enabled":true},{"id":"host","key":"Host","value":"wrong.invalid","enabled":true}],"body_kind":"json","body":"{\"message\":\"鼹鼠 \\\"quotes\\\" & +\"}","auth":{"kind":"bearer","token":"fixture-token","username":"","password":""},"timeout_ms":1000,"follow_redirects":false,"verify_tls":true,"assertions":[],"examples":[]}]}],"environments":[],"active_environment_id":null}}),
    )?;
    let mode = args.next();
    let targets = if mode.as_deref() == Some("csharp") {
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
    for (target, client, filename) in targets {
        let code = generate_request(&workspace, "r", target, client, true)?.code;
        std::fs::write(std::path::Path::new(&output).join(filename), code)?;
    }
    Ok(())
}
