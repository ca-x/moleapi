use crate::ApiError;
use serde_json::Value;
pub(super) fn append(
    items: &mut Vec<Value>,
    mut item: Value,
    bytes: &mut usize,
    omitted: &mut usize,
) -> Result<(), ApiError> {
    if item.get("outcome").is_none() {
        let skipped = item["condition_skipped"].as_bool() == Some(true)
            || item["response"]["skipped"].as_bool() == Some(true);
        let tests = item["response"]["tests"].as_array();
        let failed = tests.map_or(0, |tests| {
            tests
                .iter()
                .filter(|test| test["passed"].as_bool() != Some(true))
                .count()
        });
        let passed = tests.map_or(0, |tests| tests.len() - failed);
        item["tests_failed"] = failed.into();
        item["tests_passed"] = passed.into();
        item["outcome"] = if item.get("error").is_some() || failed > 0 {
            "failed"
        } else if skipped {
            "skipped"
        } else {
            "passed"
        }
        .into();
        if item.get("size_bytes").is_none() {
            item["size_bytes"] = item["response"]["size_bytes"].clone();
        }
    }
    if let Some(method) = item["response"]["request_updates"]
        .as_array()
        .and_then(|updates| updates.iter().find(|update| update["field"] == "method"))
        .and_then(|update| update["value"].as_str())
        .map(str::to_string)
    {
        item["method"] = method.into();
    }
    let size = serde_json::to_vec(&item)
        .map_err(|_| ApiError::internal())?
        .len();
    if bytes.saturating_add(size) > 8 * 1024 * 1024 {
        item.as_object_mut()
            .ok_or_else(ApiError::internal)?
            .remove("response");
        item["response_omitted"] = true.into();
        *omitted += 1;
    }
    *bytes += serde_json::to_vec(&item)
        .map_err(|_| ApiError::internal())?
        .len();
    items.push(item);
    Ok(())
}
