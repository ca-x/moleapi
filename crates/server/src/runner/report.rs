use crate::ApiError;
use serde_json::Value;
pub(super) fn append(
    items: &mut Vec<Value>,
    mut item: Value,
    bytes: &mut usize,
    omitted: &mut usize,
) -> Result<(), ApiError> {
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
