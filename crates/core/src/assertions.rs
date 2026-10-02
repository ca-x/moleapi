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
                _ => (false, "unsupported assertion".into()),
            };
            TestResult {
                id: a.id.clone(),
                name: a.name.clone(),
                passed,
                actual,
                expected: a.expected.clone(),
            }
        })
        .collect()
}
