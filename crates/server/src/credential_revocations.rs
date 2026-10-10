//! Durable owner revisions propagate explicit credential revocation between hosted instances.
use crate::{
    ApiError, AppState,
    entities::{account, setting},
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set, sea_query::Expr,
};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;
const PREFIX: &str = "credential-revocation:";

pub(crate) async fn revisions<C: ConnectionTrait>(
    db: &C,
) -> Result<HashMap<String, i64>, sea_orm::DbErr> {
    Ok(setting::Entity::find()
        .filter(setting::Column::Id.starts_with(PREFIX))
        .all(db)
        .await?
        .into_iter()
        .map(|row| (row.id[PREFIX.len()..].to_owned(), row.revision))
        .collect())
}

pub(crate) async fn current<C: ConnectionTrait>(
    db: &C,
    owner: &str,
) -> Result<i64, sea_orm::DbErr> {
    Ok(setting::Entity::find_by_id(format!("{PREFIX}{owner}"))
        .one(db)
        .await?
        .map_or(0, |row| row.revision))
}

/// Call inside the credential-deletion transaction. The account row serializes publishers.
pub(crate) async fn publish<C: ConnectionTrait>(db: &C, owner: &str) -> Result<i64, ApiError> {
    account::Entity::update_many()
        .col_expr(
            account::Column::Username,
            Expr::col(account::Column::Username).into(),
        )
        .filter(account::Column::Id.eq(owner))
        .exec(db)
        .await?;
    if account::Entity::find_by_id(owner).one(db).await?.is_none() {
        return Err(ApiError::unauthorized());
    }
    let id = format!("{PREFIX}{owner}");
    let next = match setting::Entity::find_by_id(&id).one(db).await? {
        Some(previous) => {
            let revision = previous
                .revision
                .checked_add(1)
                .ok_or_else(ApiError::internal)?;
            setting::Entity::update_many()
                .col_expr(setting::Column::Revision, Expr::value(revision))
                .filter(setting::Column::Id.eq(&id))
                .exec(db)
                .await?;
            revision
        }
        None => {
            setting::ActiveModel {
                id: Set(id),
                revision: Set(1),
            }
            .insert(db)
            .await?;
            1
        }
    };
    Ok(next)
}

/// The caller holds the owner admission gate during deletion or remote event application.
pub(crate) async fn apply_locked(
    state: &AppState,
    owner: &str,
    revision: i64,
    generation: &mut u64,
) -> Result<(), ApiError> {
    if state
        .credential_revisions
        .lock()
        .await
        .get(owner)
        .is_some_and(|seen| *seen >= revision)
    {
        return Ok(());
    }
    *generation = generation.checked_add(1).ok_or_else(ApiError::internal)?;
    crate::auth::stop_owner(state, owner).await;
    state
        .credential_revisions
        .lock()
        .await
        .insert(owner.into(), revision);
    Ok(())
}

pub(crate) struct Lifetime {
    state: AppState,
    stop: CancellationToken,
}
impl Drop for Lifetime {
    fn drop(&mut self) {
        self.stop.cancel();
    }
}
pub(crate) fn start(state: AppState) -> Arc<Lifetime> {
    let guard = Arc::new(Lifetime {
        state,
        stop: CancellationToken::new(),
    });
    let weak = Arc::downgrade(&guard);
    let stop = guard.stop.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! { _ = stop.cancelled() => break, _ = tokio::time::sleep(Duration::from_secs(1)) => {} }
            let Some(state) = weak.upgrade().map(|guard| guard.state.clone()) else {
                break;
            };
            tokio::select! {
                _ = stop.cancelled() => break,
                result = tick(&state) => {
                    if result.is_err() { eprintln!("credential revocation scan failed; retrying"); }
                }
            }
        }
    });
    guard
}
async fn tick(state: &AppState) -> Result<(), ApiError> {
    for (owner, revision) in revisions(&state.db).await? {
        if state
            .credential_revisions
            .lock()
            .await
            .get(&owner)
            .is_some_and(|seen| *seen >= revision)
        {
            continue;
        }
        let gate = state.protocol_admission.owner(&owner)?;
        let mut generation = gate.lock().await;
        apply_locked(state, &owner, revision, &mut generation).await?;
    }
    Ok(())
}
