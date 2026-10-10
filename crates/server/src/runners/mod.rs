mod agent;
mod registry;
mod selection;
mod storage;
mod tasks;
use self::storage::Task;
use crate::{ApiError, AppState};
pub(crate) use agent::{claim, complete, heartbeat};
use moleapi_core::RunnerTaskStatus as Status;
pub(crate) use registry::{list, register, update};
pub(crate) use tasks::{cancel, queue, tasks};
fn hosted(state: &AppState) -> Result<(), ApiError> {
    if state.local {
        return Err(ApiError::bad("Execution runners require a hosted service"));
    }
    Ok(())
}
fn terminal(task: &mut Task, status: Status) {
    task.summary.status = status;
    task.summary.finished_at = Some(crate::storage::now());
    task.snapshot = None;
}
