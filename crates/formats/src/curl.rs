use crate::{ImportResult, data, pair, request, uid};
use anyhow::{Context, Result, bail};
use moleapi_core::Collection;

pub(super) fn import(content: &str) -> Result<ImportResult> {
    let normalized = content.replace("\\\r\n", " ").replace("\\\n", " ");
    let words = shell_words::split(&normalized).context("cURL 引号或转义无效")?;
    if words.first().map(String::as_str) != Some("curl") {
        bail!("内容必须以 curl 开始");
    }
    let mut result = request("导入 cURL".into(), "GET".into(), String::new())?;
    result.follow_redirects = false;
    let mut method_explicit = false;
    let mut urls = 0;
    let mut i = 1;
    while i < words.len() {
        let word = &words[i];
        let mut next = || -> Result<String> {
            i += 1;
            words.get(i).cloned().context("cURL 参数缺少值")
        };
        match word.as_str() {
            "-X" | "--request" => {
                result.method = next()?.to_ascii_uppercase();
                method_explicit = true;
            }
            "-H" | "--header" => {
                let value = next()?;
                let (key, value) = value.split_once(':').context("Header 缺少冒号")?;
                result.headers.push(pair(key.trim(), value.trim()));
            }
            "-d" | "--data" | "--data-raw" | "--data-binary" => {
                let value = next()?;
                if value.starts_with('@') {
                    bail!("cURL 文件引用需要显式上传，不读取服务器文件");
                }
                result.body = value;
                result.body_kind = "text".into();
                if !method_explicit {
                    result.method = "POST".into();
                }
            }
            "--data-urlencode" => {
                let value = next()?;
                if value.starts_with('@') {
                    bail!("不支持 cURL 隐式文件读取");
                }
                if !result.body.is_empty() {
                    result.body.push('&');
                }
                if let Some((key, value)) = value.split_once('=') {
                    result.body.push_str(
                        &url::form_urlencoded::Serializer::new(String::new())
                            .append_pair(key, value)
                            .finish(),
                    );
                } else {
                    result.body.push_str(&value);
                }
                result.body_kind = "form".into();
                if !method_explicit {
                    result.method = "POST".into();
                }
            }
            "-u" | "--user" => {
                let value = next()?;
                let (user, password) = value.split_once(':').unwrap_or((&value, ""));
                result.auth.kind = "basic".into();
                result.auth.username = user.into();
                result.auth.password = password.into();
            }
            "--url" => {
                result.url = next()?;
                urls += 1;
            }
            "--max-time" | "-m" => {
                let seconds: f64 = next()?.parse().context("超时不是数字")?;
                if !seconds.is_finite() || seconds <= 0.0 || seconds > 120.0 {
                    bail!("超时必须在 0–120 秒内");
                }
                result.timeout_ms = (seconds * 1000.0) as u64;
            }
            "-L" | "--location" => result.follow_redirects = true,
            "-k" | "--insecure" => result.verify_tls = false,
            "-I" | "--head" => {
                result.method = "HEAD".into();
                method_explicit = true;
            }
            "-s" | "--silent" | "-S" | "--show-error" | "--compressed" => {}
            value if value.starts_with('-') => {
                bail!("暂不支持 cURL 选项 {value}，避免静默丢失配置")
            }
            value => {
                result.url = value.into();
                urls += 1;
            }
        }
        i += 1;
    }
    if urls != 1 || result.url.is_empty() {
        bail!("cURL 必须包含且只包含一个 URL");
    }
    if result
        .headers
        .iter()
        .any(|h| h.key.eq_ignore_ascii_case("content-type") && h.value.contains("json"))
    {
        result.body_kind = "json".into();
    }
    Ok(ImportResult {
        name: "cURL 导入".into(),
        data: data(
            vec![Collection {
                variables_enabled: None,
                parent_id: None,
                auth: None,
                variables: vec![],
                pre_request_script: String::new(),
                post_response_script: String::new(),
                id: uid(),
                name: "cURL".into(),
                description: String::new(),
                requests: vec![result],
            }],
            vec![],
        ),
        warnings: vec![],
    })
}
