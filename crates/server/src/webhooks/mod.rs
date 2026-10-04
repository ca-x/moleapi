mod api;
mod inspect;
mod listener;
mod manager;
mod models;
mod receiver;
mod replay;
mod storage;
pub use api::*;
pub use inspect::{export, get as capture_get, list as captures};
pub use listener::{start as listener_start, status as listener_status, stop as listener_stop};
pub(crate) use manager::Hub;
pub use receiver::{ReceiverState, ingest};
pub use replay::{cancel as replay_cancel, run as replay};
pub(crate) async fn cascade<C: sea_orm::ConnectionTrait>(
    db: &C,
    owner: &str,
    workspace: &str,
) -> Result<(), crate::ApiError> {
    let ids: Vec<_> = storage::inboxes_for_workspace(db, owner, workspace)
        .await?
        .into_iter()
        .map(|v| v.id)
        .collect();
    storage::delete_captures(db, owner, &ids).await
}
