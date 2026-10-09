use crate::{Response, VariableUpdate};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Extraction {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub target: String,
    pub scope: String,
    pub key: String,
    #[serde(default = "enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub required: bool,
}
fn enabled() -> bool {
    true
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub id: String,
    pub name: String,
    pub required: bool,
    pub update: Option<VariableUpdate>,
    pub error: Option<String>,
}
pub fn validate_extractions(rules: &[Extraction]) -> anyhow::Result<()> {
    use anyhow::ensure;
    let mut ids = std::collections::BTreeSet::new();
    ensure!(rules.len() <= 100, "Request exceeds 100 extraction rules");
    for rule in rules {
        ensure!(
            !rule.id.is_empty()
                && rule.id.len() <= 128
                && ids.insert(&rule.id)
                && rule.name.len() <= 256,
            "Invalid extraction ID/name"
        );
        ensure!(
            matches!(
                rule.kind.as_str(),
                "body" | "header" | "json" | "jsonpath" | "xpath" | "regex"
            ),
            "Unsupported extraction kind"
        );
        ensure!(
            matches!(
                rule.scope.as_str(),
                "temporary" | "environment" | "collection" | "project"
            ),
            "Unsupported extraction variable scope"
        );
        ensure!(
            !rule.key.is_empty() && rule.key.len() <= 1024 && rule.target.len() <= 4096,
            "Invalid extraction variable key or selector"
        );
    }
    Ok(())
}
pub(crate) fn evaluate_extractions(
    rules: &[Extraction],
    response: &Response,
) -> Vec<ExtractionResult> {
    let mut total = 0usize;
    rules
        .iter()
        .filter(|rule| rule.enabled)
        .map(|rule| {
            let result = select(rule, response).and_then(|value| {
                let value = match value {
                    Value::String(value) => value,
                    value => value.to_string(),
                };
                total = total.saturating_add(value.len());
                if value.len() > 64 * 1024 || total > crate::MAX_VARIABLE_BYTES {
                    Err("extraction value limit exceeded")
                } else {
                    Ok(value)
                }
            });
            let (update, error) = match result {
                Ok(value) => (
                    Some(VariableUpdate {
                        scope: rule.scope.clone(),
                        key: rule.key.clone(),
                        value: Some(value),
                    }),
                    None,
                ),
                Err(error) => (None, Some(error.into())),
            };
            ExtractionResult {
                id: rule.id.clone(),
                name: rule.name.clone(),
                required: rule.required,
                update,
                error,
            }
        })
        .collect()
}
fn select(rule: &Extraction, response: &Response) -> Result<Value, &'static str> {
    if rule.kind == "header" {
        return response
            .headers
            .iter()
            .find(|header| header.key.eq_ignore_ascii_case(&rule.target))
            .map(|header| Value::String(header.value.clone()))
            .ok_or("response header missing");
    }
    if response.truncated || response.body_base64.is_some() {
        return Err("extraction requires a complete UTF-8 response");
    }
    match rule.kind.as_str() {
        "body" => Ok(Value::String(response.body.clone())),
        "json" | "jsonpath" => {
            let document: Value =
                serde_json::from_str(&response.body).map_err(|_| "invalid JSON response")?;
            crate::assertions::structured_budget(&document)?;
            if rule.kind == "json" {
                return document
                    .pointer(&rule.target)
                    .cloned()
                    .ok_or("JSON pointer missing");
            }
            use jsonpath_rust::JsonPath;
            let values = document
                .query(&rule.target)
                .map_err(|_| "invalid JSONPath query")?;
            if values.is_empty() {
                Err("JSONPath match missing")
            } else if values.len() == 1 {
                Ok(values[0].clone())
            } else {
                Ok(Value::Array(values.into_iter().cloned().collect()))
            }
        }
        "regex" => {
            let regex = fancy_regex::RegexBuilder::new(&rule.target)
                .backtrack_limit(100_000)
                .build()
                .map_err(|_| "invalid extraction regex")?;
            let captures = regex
                .captures(&response.body)
                .map_err(|_| "regex extraction limit exceeded")?;
            captures
                .and_then(|capture| capture.get(1).or_else(|| capture.get(0)))
                .map(|capture| Value::String(capture.as_str().into()))
                .ok_or("regex extraction match missing")
        }
        "xpath" => {
            let xml = roxmltree::Document::parse(&response.body)
                .map_err(|_| "invalid XML or forbidden DTD")?;
            if xml.descendants().count() > 50000 {
                return Err("XML extraction complexity limit exceeded");
            }
            let package =
                sxd_document::parser::parse(&response.body).map_err(|_| "invalid XML response")?;
            let xpath = sxd_xpath::Factory::new()
                .build(&rule.target)
                .map_err(|_| "invalid XPath query")?
                .ok_or("empty XPath query")?;
            let mut context = sxd_xpath::Context::new();
            for ns in xml.root_element().namespaces() {
                if let Some(prefix) = ns.name() {
                    context.set_namespace(prefix, ns.uri());
                }
            }
            let value = xpath
                .evaluate(&context, package.as_document().root())
                .map_err(|_| "XPath extraction failed")?;
            match value {
                sxd_xpath::Value::Nodeset(ref nodes) if nodes.size() == 0 => {
                    Err("XPath match missing")
                }
                sxd_xpath::Value::Boolean(value) => Ok(Value::Bool(value)),
                sxd_xpath::Value::Number(value) => serde_json::Number::from_f64(value)
                    .map(Value::Number)
                    .ok_or("XPath result is not finite"),
                value => Ok(Value::String(value.string())),
            }
        }
        _ => Err("unsupported extraction kind"),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn rule(kind: &str, target: &str) -> Extraction {
        Extraction {
            id: "r".into(),
            name: "Value".into(),
            kind: kind.into(),
            target: target.into(),
            scope: "temporary".into(),
            key: "value".into(),
            enabled: true,
            required: true,
        }
    }
    fn response(body: &str) -> Response {
        serde_json::from_value(json!({"status":200,"status_text":"OK","headers":[{"id":"h","key":"X-Token","value":"header-value","enabled":true}],"body":body,"elapsed_ms":1,"size_bytes":body.len(),"truncated":false,"url":"https://example.test","tests":[]})).unwrap()
    }
    #[test]
    fn selectors_preserve_values_types_and_missing_disabled_rules() {
        let json = response(r#"{"items":[1,2],"value":"id=42"}"#);
        let checks = [
            rule("json", "/items"),
            rule("jsonpath", "$.items[*]"),
            rule("regex", "id=(\\d+)"),
            rule("header", "x-token"),
        ];
        let result = evaluate_extractions(&checks, &json);
        assert_eq!(
            result
                .iter()
                .map(|value| value.update.as_ref().unwrap().value.as_deref().unwrap())
                .collect::<Vec<_>>(),
            vec!["[1,2]", "[1,2]", "42", "header-value"]
        );
        let xml = response(r#"<s:root xmlns:s="urn:test"><s:id>17</s:id></s:root>"#);
        assert_eq!(
            evaluate_extractions(&[rule("xpath", "string(/s:root/s:id)")], &xml)[0]
                .update
                .as_ref()
                .unwrap()
                .value
                .as_deref(),
            Some("17")
        );
        let mut missing = rule("json", "/missing");
        assert!(
            evaluate_extractions(std::slice::from_ref(&missing), &json)[0]
                .error
                .is_some()
        );
        missing.enabled = false;
        assert!(evaluate_extractions(&[missing], &json).is_empty());
    }
    #[test]
    fn oversized_values_are_not_truncated_into_different_variables() {
        let response = response(&"x".repeat(70 * 1024));
        let result = evaluate_extractions(&[rule("body", "")], &response);
        assert!(result[0].update.is_none());
        assert!(result[0].error.as_ref().unwrap().contains("limit"));
    }
}
