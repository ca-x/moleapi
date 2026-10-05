//! Portable selected-file source; reqwest owns the MIME wire format.
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const MAX_BODY_SOURCE: usize = 8 * 1024 * 1024;
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BinaryBody {
    pub file_name: String,
    pub mime: String,
    pub base64: Option<String>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MultipartBody {
    pub parts: Vec<MultipartPart>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MultipartPart {
    pub id: String,
    pub name: String,
    #[serde(default = "enabled")]
    pub enabled: bool,
    pub value: MultipartValue,
}
fn enabled() -> bool {
    true
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MultipartValue {
    Text {
        text: String,
        #[serde(default)]
        mime: String,
    },
    File {
        file: BinaryBody,
    },
}
fn mime_valid(value: &str, templates: bool) -> Result<()> {
    ensure!(value.len() <= 128, "Body MIME type exceeds limit");
    if !value.is_empty() && !(templates && value.contains("{{")) {
        value
            .parse::<mime::Mime>()
            .context("Invalid body MIME type")?;
    }
    Ok(())
}
fn file_valid(file: &BinaryBody, missing_allowed: bool, templates: bool) -> Result<usize> {
    ensure!(
        file.file_name.len() <= 512 && !file.file_name.chars().any(char::is_control),
        "Invalid body filename"
    );
    mime_valid(&file.mime, templates)?;
    match &file.base64 {
        Some(value) => {
            ensure!(
                value.len() <= crate::MAX_BODY.div_ceil(3) * 4,
                "Body file exceeds 5 MiB"
            );
            let bytes = STANDARD.decode(value).context("Invalid body file Base64")?;
            ensure!(bytes.len() <= crate::MAX_BODY, "Body file exceeds 5 MiB");
            Ok(bytes.len())
        }
        None => {
            ensure!(
                missing_allowed,
                "Select the body file again; its bytes are missing"
            );
            Ok(0)
        }
    }
}
pub fn validate_structured_body(request: &crate::RequestSpec, templates: bool) -> Result<()> {
    if !matches!(request.body_kind.as_str(), "binary" | "multipart") {
        return Ok(());
    }
    ensure!(
        request.protocol == crate::Protocol::Http,
        "Structured file bodies require HTTP"
    );
    ensure!(
        request.body.len() <= MAX_BODY_SOURCE,
        "Body source exceeds 8 MiB"
    );
    if request.body_kind == "binary" {
        let file: BinaryBody =
            serde_json::from_str(&request.body).context("Invalid binary body source")?;
        file_valid(&file, templates, templates)?;
    } else {
        let source: MultipartBody =
            serde_json::from_str(&request.body).context("Invalid multipart body source")?;
        ensure!(source.parts.len() <= 64, "Multipart body exceeds 64 parts");
        let mut ids = std::collections::HashSet::new();
        let mut total = 0usize;
        for part in &source.parts {
            ensure!(
                !part.id.is_empty() && part.id.len() <= 128 && ids.insert(&part.id),
                "Multipart IDs must be unique and nonempty"
            );
            ensure!(
                !part.name.is_empty()
                    && part.name.len() <= 512
                    && !part.name.chars().any(char::is_control),
                "Invalid multipart field name"
            );
            total = total
                .checked_add(match &part.value {
                    MultipartValue::Text { text, mime } => {
                        mime_valid(mime, templates || !part.enabled)?;
                        text.len()
                    }
                    MultipartValue::File { file } => {
                        file_valid(file, templates || !part.enabled, templates || !part.enabled)?
                    }
                })
                .context("Body size overflow")?;
            ensure!(
                total <= crate::MAX_BODY,
                "Combined body files/text exceed 5 MiB"
            );
        }
        ensure!(
            !request
                .headers
                .iter()
                .any(|h| h.enabled && h.key.eq_ignore_ascii_case("content-type")),
            "Multipart Content-Type is generated with its boundary; disable the manual header"
        );
    }
    Ok(())
}
fn encode_source<T: Serialize>(value: &T) -> Result<String> {
    struct Limited(Vec<u8>);
    impl std::io::Write for Limited {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.0.len().saturating_add(bytes.len()) > MAX_BODY_SOURCE {
                return Err(std::io::Error::other("Body source expansion exceeds 8 MiB"));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut output = Limited(Vec::new());
    serde_json::to_writer(&mut output, value)?;
    Ok(String::from_utf8(output.0)?)
}
pub(crate) fn resolve_structured_body(
    kind: &str,
    source: &str,
    variables: &HashMap<&str, &str>,
    shared: &mut usize,
) -> Result<String> {
    ensure!(source.len() <= MAX_BODY_SOURCE, "Body source exceeds 8 MiB");
    fn resolve(
        value: &str,
        variables: &HashMap<&str, &str>,
        maximum: usize,
        shared: &mut usize,
    ) -> Result<String> {
        let mut budget = maximum.min(*shared);
        let output = crate::interpolation::interpolate_budget(value, variables, &mut budget)?;
        *shared -= output.len();
        Ok(output)
    }
    if kind == "binary" {
        let mut file: BinaryBody = serde_json::from_str(source)?;
        file_valid(&file, true, true)?;
        file.mime = resolve(&file.mime, variables, 128, shared)?;
        encode_source(&file)
    } else {
        let mut body: MultipartBody = serde_json::from_str(source)?;
        ensure!(body.parts.len() <= 64, "Multipart body exceeds 64 parts");
        let mut remaining = crate::MAX_BODY;
        for part in &body.parts {
            let used = match &part.value {
                MultipartValue::File { file } => file_valid(file, true, true)?,
                MultipartValue::Text { text, .. } if !part.enabled => text.len(),
                _ => 0,
            };
            remaining = remaining
                .checked_sub(used)
                .context("Combined body files/text exceed 5 MiB")?;
        }
        for part in &mut body.parts {
            if !part.enabled {
                continue;
            }
            part.name = resolve(&part.name, variables, 512, shared)?;
            match &mut part.value {
                MultipartValue::Text { text, mime } => {
                    *text = resolve(text, variables, remaining, shared)?;
                    remaining -= text.len();
                    *mime = resolve(mime, variables, 128, shared)?;
                }
                MultipartValue::File { file } => {
                    file.mime = resolve(&file.mime, variables, 128, shared)?
                }
            }
        }
        encode_source(&body)
    }
}
pub(crate) async fn prepare_structured_body(
    request: &crate::RequestSpec,
) -> Result<(Vec<u8>, String)> {
    validate_structured_body(request, false)?;
    if request.body_kind == "binary" {
        let file: BinaryBody = serde_json::from_str(&request.body)?;
        Ok((
            STANDARD.decode(file.base64.context("Body file is missing")?)?,
            if file.mime.is_empty() {
                "application/octet-stream".into()
            } else {
                file.mime
            },
        ))
    } else {
        let source: MultipartBody = serde_json::from_str(&request.body)?;
        let mut form = reqwest::multipart::Form::new();
        for part in source.parts.into_iter().filter(|part| part.enabled) {
            let value = match part.value {
                MultipartValue::Text { text, mime } => {
                    let value = reqwest::multipart::Part::text(text);
                    if mime.is_empty() {
                        value
                    } else {
                        value.mime_str(&mime)?
                    }
                }
                MultipartValue::File { file } => {
                    let value = reqwest::multipart::Part::bytes(
                        STANDARD.decode(file.base64.context("Body file is missing")?)?,
                    )
                    .file_name(file.file_name);
                    if file.mime.is_empty() {
                        value.mime_str("application/octet-stream")?
                    } else {
                        value.mime_str(&file.mime)?
                    }
                }
            };
            form = form.part(part.name, value);
        }
        let content_type = format!("multipart/form-data; boundary={}", form.boundary());
        use futures_util::StreamExt;
        let stream = form.into_stream();
        futures_util::pin_mut!(stream);
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            ensure!(
                bytes.len().saturating_add(chunk.len()) <= crate::MAX_BODY + 256 * 1024,
                "Encoded multipart body exceeds limit"
            );
            bytes.extend_from_slice(&chunk);
        }
        Ok((bytes, content_type))
    }
}
