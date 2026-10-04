use crate::ApiError;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard, Semaphore};
use tokio_util::sync::CancellationToken;
type WorkspaceGates = HashMap<(String, String), Weak<AsyncMutex<()>>>;
#[derive(Default)]
pub struct Gates(Mutex<WorkspaceGates>);
impl Gates {
    pub async fn lock(&self, owner: &str, workspace: &str) -> OwnedMutexGuard<()> {
        let gate = {
            let mut gates = self.0.lock().unwrap();
            gates.retain(|_, value| value.strong_count() > 0);
            let key = (owner.to_owned(), workspace.to_owned());
            match gates.get(&key).and_then(Weak::upgrade) {
                Some(value) => value,
                None => {
                    let value = Arc::new(AsyncMutex::new(()));
                    gates.insert(key, Arc::downgrade(&value));
                    value
                }
            }
        };
        gate.lock_owned().await
    }
}
pub struct Bridge {
    pub origin: String,
    pub bind: String,
    pub stop: CancellationToken,
    pub task: tokio::task::JoinHandle<()>,
}
pub struct ReplayTask {
    pub workspace: String,
    pub inbox: String,
    pub stop: CancellationToken,
}
pub struct Hub {
    pub gates: Arc<Gates>,
    pub admission: AsyncMutex<()>,
    pub intake: Arc<Semaphore>,
    pub replay: Arc<Semaphore>,
    pub bridge: AsyncMutex<Option<Bridge>>,
    pub replay_tasks: Mutex<HashMap<(String, String), ReplayTask>>,
}
impl Default for Hub {
    fn default() -> Self {
        Self {
            gates: Arc::new(Gates::default()),
            admission: AsyncMutex::new(()),
            intake: Arc::new(Semaphore::new(32)),
            replay: Arc::new(Semaphore::new(4)),
            bridge: AsyncMutex::new(None),
            replay_tasks: Mutex::new(HashMap::new()),
        }
    }
}
impl Drop for Hub {
    fn drop(&mut self) {
        if let Some(bridge) = self.bridge.get_mut().take() {
            bridge.stop.cancel();
            bridge.task.abort();
        }
        for task in self.replay_tasks.get_mut().unwrap().values() {
            task.stop.cancel();
        }
    }
}
pub fn capacity(message: &str) -> ApiError {
    ApiError {
        status: axum::http::StatusCode::TOO_MANY_REQUESTS,
        message: message.into(),
    }
}

impl Hub {
    pub fn cancel_scope(&self, owner: &str, workspace: Option<&str>, inbox: Option<&str>) {
        for ((task_owner, _), task) in self.replay_tasks.lock().unwrap().iter() {
            if task_owner == owner
                && workspace.is_none_or(|w| w == task.workspace)
                && inbox.is_none_or(|i| i == task.inbox)
            {
                task.stop.cancel();
            }
        }
    }
}
