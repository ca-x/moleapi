use crate::{Assertion, Response, TestResult};
pub fn assertions(checks: &[Assertion], response: &Response) -> Vec<TestResult> {
    checks
        .iter()
        .map(|a| {
            let (passed, actual) = match a.kind.as_str() {
                "status" => (
                    a.expected.parse::<u16>().ok() == Some(response.status),
                    response.status.to_string(),
                ),
                "duration" => (
                    a.expected
                        .parse::<u64>()
                        .is_ok_and(|max| response.elapsed_ms <= max),
                    response.elapsed_ms.to_string(),
                ),
                "contains" => (
                    response.body.contains(&a.expected),
                    if response.body.contains(&a.expected) {
                        "found"
                    } else {
                        "not found"
                    }
                    .into(),
                ),
                "json" => {
                    let value = serde_json::from_str::<serde_json::Value>(&response.body)
                        .ok()
                        .and_then(|v| v.pointer(&a.target).cloned());
                    let expected = serde_json::from_str::<serde_json::Value>(&a.expected).ok();
                    (
                        value.is_some() && value == expected,
                        value
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "missing or invalid JSON".into()),
                    )
                }
                "header" => {
                    let values = response
                        .headers
                        .iter()
                        .filter(|header| header.key.eq_ignore_ascii_case(&a.target))
                        .map(|header| header.value.as_str())
                        .collect::<Vec<_>>();
                    (
                        values.iter().any(|value| *value == a.expected),
                        values.join("; "),
                    )
                }
                "regex" | "jsonpath" | "xpath" | "schema" => match advanced(a, response) {
                    Ok(value) => value,
                    Err(message) => (false, message.to_string()),
                },
                _ => (false, "unsupported assertion".into()),
            };
            TestResult {
                id: a.id.clone(),
                name: a.name.clone(),
                passed,
                actual: if actual.len() <= 4096 {
                    actual
                } else {
                    "[assertion detail omitted: size limit]".into()
                },
                expected: if a.kind == "schema" {
                    "valid JSON Schema".into()
                } else {
                    if a.expected.len() <= 4096 {
                        a.expected.clone()
                    } else {
                        "[expected detail omitted: size limit]".into()
                    }
                },
            }
        })
        .collect()
}

pub(crate) fn structured_budget(value: &serde_json::Value) -> Result<(), &'static str> {
    let mut pending = vec![(value, 0usize)];
    let mut nodes = 0;
    while let Some((value, depth)) = pending.pop() {
        nodes += 1;
        if nodes > 50000 || depth > 64 {
            return Err("assertion data complexity limit exceeded");
        }
        match value {
            serde_json::Value::Object(values) => {
                pending.extend(values.values().map(|value| (value, depth + 1)))
            }
            serde_json::Value::Array(values) => {
                pending.extend(values.iter().map(|value| (value, depth + 1)))
            }
            _ => {}
        }
    }
    Ok(())
}
struct NoResources;
impl jsonschema::Retrieve for NoResources {
    fn retrieve(
        &self,
        _: &jsonschema::Uri<String>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        Err("External assertion schema resources are disabled".into())
    }
}
fn advanced(check: &Assertion, response: &Response) -> Result<(bool, String), &'static str> {
    if response.body_base64.is_some()
        && matches!(check.kind.as_str(), "jsonpath" | "schema" | "xpath")
    {
        return Err("structured assertion requires a UTF-8 response");
    }
    if response.truncated {
        return Err("response is truncated");
    }
    if check.target.len() > 4096
        || check.expected.len() > 64 * 1024
        || response.body.len() > crate::MAX_BODY
    {
        return Err("assertion input limit exceeded");
    }
    match check.kind.as_str() {
        "regex" => {
            let regex = fancy_regex::RegexBuilder::new(&check.target)
                .backtrack_limit(100_000)
                .build()
                .map_err(|_| "invalid regular expression")?;
            let capture = regex
                .captures(&response.body)
                .map_err(|_| "regex evaluation limit exceeded")?;
            let value = capture
                .as_ref()
                .and_then(|capture| capture.get(1).or_else(|| capture.get(0)))
                .map(|value| value.as_str());
            Ok((
                value.is_some_and(|value| check.expected.is_empty() || value == check.expected),
                value.unwrap_or("no match").into(),
            ))
        }
        "jsonpath" => {
            use jsonpath_rust::JsonPath;
            let document: serde_json::Value =
                serde_json::from_str(&response.body).map_err(|_| "invalid JSON response")?;
            structured_budget(&document)?;
            let values = document
                .query(&check.target)
                .map_err(|_| "invalid JSONPath query")?;
            if values.is_empty() {
                return Ok((false, "no JSONPath match".into()));
            }
            let actual = if values.len() == 1 {
                values[0].clone()
            } else {
                serde_json::Value::Array(values.into_iter().cloned().collect())
            };
            let expected: serde_json::Value =
                serde_json::from_str(&check.expected).map_err(|_| "invalid expected JSON value")?;
            Ok((actual == expected, actual.to_string()))
        }
        "schema" => {
            let schema: serde_json::Value =
                serde_json::from_str(&check.expected).map_err(|_| "invalid JSON Schema")?;
            structured_budget(&schema)?;
            let validator = jsonschema::options()
                .with_retriever(NoResources)
                .with_pattern_options(
                    jsonschema::PatternOptions::fancy_regex()
                        .backtrack_limit(100_000)
                        .size_limit(1024 * 1024),
                )
                .should_validate_formats(true)
                .build(&schema)
                .map_err(|_| "invalid or unavailable JSON Schema resource")?;
            let document: serde_json::Value =
                serde_json::from_str(&response.body).map_err(|_| "invalid JSON response")?;
            structured_budget(&document)?;
            let value = if check.target.is_empty() {
                &document
            } else {
                document
                    .pointer(&check.target)
                    .ok_or("schema instance pointer missing")?
            };
            let passed = validator.is_valid(value);
            Ok((
                passed,
                if passed {
                    "schema valid"
                } else {
                    "schema mismatch"
                }
                .into(),
            ))
        }
        "xpath" => {
            let xml = roxmltree::Document::parse(&response.body)
                .map_err(|_| "invalid XML or forbidden DTD")?;
            if xml.descendants().count() > 50000 {
                return Err("XML assertion complexity limit exceeded");
            }
            let package =
                sxd_document::parser::parse(&response.body).map_err(|_| "invalid XML response")?;
            let xpath = sxd_xpath::Factory::new()
                .build(&check.target)
                .map_err(|_| "invalid XPath query")?
                .ok_or("empty XPath query")?;
            let mut context = sxd_xpath::Context::new();
            for namespace in xml.root_element().namespaces() {
                if let Some(prefix) = namespace.name() {
                    context.set_namespace(prefix, namespace.uri());
                }
            }
            let result = xpath
                .evaluate(&context, package.as_document().root())
                .map_err(|_| "XPath evaluation failed")?;
            let (passed, actual) = match result {
                sxd_xpath::Value::Boolean(value) => (
                    check.expected.parse::<bool>().ok() == Some(value),
                    value.to_string(),
                ),
                sxd_xpath::Value::Number(value) => (
                    check.expected.parse::<f64>().ok() == Some(value),
                    value.to_string(),
                ),
                sxd_xpath::Value::Nodeset(ref nodes) if nodes.size() == 0 => {
                    (false, "no XPath match".into())
                }
                value => {
                    let text = value.string();
                    (text == check.expected, text)
                }
            };
            Ok((passed, actual))
        }
        _ => Err("unsupported assertion"),
    }
}
