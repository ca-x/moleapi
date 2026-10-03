//! Owner-scoped admission fencing. Weak gates disappear after pending calls finish.
use crate::ApiError;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};
use tokio::sync::Mutex as AsyncMutex;
#[derive(Default)]
pub(crate) struct AdmissionGates {
    owners: Mutex<HashMap<String, Weak<AsyncMutex<u64>>>>,
}
impl AdmissionGates {
    pub fn owner(&self, owner: &str) -> Result<Arc<AsyncMutex<u64>>, ApiError> {
        let mut owners = self.owners.lock().unwrap();
        owners.retain(|_, gate| gate.strong_count() > 0);
        if let Some(gate) = owners.get(owner).and_then(Weak::upgrade) {
            return Ok(gate);
        }
        if owners.len() >= 1024 {
            return Err(ApiError {
                status: axum::http::StatusCode::TOO_MANY_REQUESTS,
                message: "Protocol preparation capacity reached (1024 owners)".into(),
            });
        }
        let gate = Arc::new(AsyncMutex::new(0));
        owners.insert(owner.into(), Arc::downgrade(&gate));
        Ok(gate)
    }
}
