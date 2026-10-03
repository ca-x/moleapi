use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fmt};

/// Private values survive a failed phase without committing its variable changes.
#[derive(Clone, Serialize, Deserialize)]
pub struct ScriptFailure {
    pub message: String,
    pub private_values: BTreeSet<String>,
    pub privacy_complete: bool,
}
impl fmt::Display for ScriptFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}
impl std::error::Error for ScriptFailure {}

impl fmt::Debug for ScriptFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ScriptFailure")
            .field("privacy_complete", &self.privacy_complete)
            .field("private_value_count", &self.private_values.len())
            .finish_non_exhaustive()
    }
}
