use crate::{ApiError, AppState, entities::document, mock_logs, storage};
use moleapi_core::{Example, Workspace};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbBackend, DbErr, EntityTrait, QueryFilter, Statement,
    TransactionTrait, sea_query::Expr,
};
use serde::{Deserialize, Serialize};

const KIND: &str = "mock-rate";
const WINDOW_MS: i64 = 60_000;

#[derive(Serialize, Deserialize)]
struct RateState {
    workspace_revision: i64,
    request_id: String,
    example_id: String,
    samples_ms: Vec<i64>,
}

pub(crate) enum Admission {
    Allowed,
    Limited { retry_after_seconds: u64 },
}

fn state_id(owner: &str, workspace: &str, request: &str, example: &str) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "mr_{}",
        hex::encode(Sha256::digest(
            format!("{owner}\0{workspace}\0{request}\0{example}").as_bytes()
        ))
    )
}

fn admit_sample(samples: &mut Vec<i64>, limit: usize, now_ms: i64) -> Option<u64> {
    samples.retain(|sample| *sample > now_ms - WINDOW_MS);
    if samples.len() >= limit {
        let remaining_ms = (samples[0] + WINDOW_MS - now_ms).max(1);
        Some(((remaining_ms + 999) / 1000) as u64)
    } else {
        samples.push(now_ms);
        None
    }
}

async fn database_now_ms<C: ConnectionTrait>(db: &C) -> Result<i64, ApiError> {
    let backend = db.get_database_backend();
    let sql = match backend {
        DbBackend::Sqlite => {
            "SELECT CAST(strftime('%s','now') AS INTEGER) * 1000 + CAST(substr(strftime('%f','now'), 4, 3) AS INTEGER) AS now_ms"
        }
        DbBackend::Postgres => {
            "SELECT CAST(EXTRACT(EPOCH FROM clock_timestamp()) * 1000 AS BIGINT) AS now_ms"
        }
        DbBackend::MySql => {
            "SELECT CAST(UNIX_TIMESTAMP(CURRENT_TIMESTAMP(3)) * 1000 AS SIGNED) AS now_ms"
        }
    };
    let row = db
        .query_one(Statement::from_string(backend, sql))
        .await?
        .ok_or_else(ApiError::internal)?;
    let now_ms: i64 = row.try_get("", "now_ms")?;
    if now_ms <= 0 {
        return Err(ApiError::internal());
    }
    Ok(now_ms)
}

pub(crate) async fn admit(
    app: &AppState,
    owner: &str,
    workspace: &Workspace,
    request_id: &str,
    example: &Example,
) -> Result<Admission, ApiError> {
    let limit = usize::from(example.rate_limit_per_minute);
    if limit == 0 {
        return Ok(Admission::Allowed);
    }
    if limit > 1_000 {
        return Err(ApiError::bad("Invalid saved Mock rate limit"));
    }
    let tx = app.db.begin().await?;
    let current = mock_logs::lock_workspace(&tx, owner, &workspace.id).await?;
    if current.revision != workspace.revision {
        return Err(ApiError::conflict(
            "Workspace changed before Mock rate admission",
        ));
    }
    let now_ms = database_now_ms(&tx).await?;
    let id = state_id(owner, &workspace.id, request_id, &example.id);
    let existing = document::Entity::find_by_id(&id)
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(KIND))
        .filter(document::Column::RefId.eq(&workspace.id))
        .one(&tx)
        .await?;
    let mut state = if let Some(row) = &existing {
        let state: RateState =
            serde_json::from_str(&row.payload).map_err(|_| ApiError::internal())?;
        if state.request_id != request_id
            || state.example_id != example.id
            || state.samples_ms.len() > 1_000
            || state.samples_ms.windows(2).any(|pair| pair[0] > pair[1])
            || state.samples_ms.iter().any(|sample| *sample > now_ms)
        {
            return Err(ApiError::internal());
        }
        state
    } else {
        RateState {
            workspace_revision: workspace.revision,
            request_id: request_id.into(),
            example_id: example.id.clone(),
            samples_ms: Vec::new(),
        }
    };
    if state.workspace_revision != workspace.revision {
        state.workspace_revision = workspace.revision;
        state.samples_ms.clear();
    }
    if let Some(retry_after_seconds) = admit_sample(&mut state.samples_ms, limit, now_ms) {
        tx.commit().await?;
        return Ok(Admission::Limited {
            retry_after_seconds,
        });
    }
    if let Some(row) = existing {
        if row.revision == i64::MAX {
            return Err(ApiError::internal());
        }
        let changed = document::Entity::update_many()
            .col_expr(
                document::Column::Payload,
                Expr::value(serde_json::to_string(&state).map_err(|_| ApiError::internal())?),
            )
            .col_expr(document::Column::Revision, Expr::value(row.revision + 1))
            .col_expr(document::Column::UpdatedAt, Expr::value(storage::now()))
            .filter(document::Column::Id.eq(&id))
            .filter(document::Column::Owner.eq(owner))
            .filter(document::Column::Kind.eq(KIND))
            .filter(document::Column::RefId.eq(&workspace.id))
            .filter(document::Column::Revision.eq(row.revision))
            .exec(&tx)
            .await?;
        if changed.rows_affected != 1 {
            return Err(ApiError::internal());
        }
    } else {
        storage::insert_doc(&tx, id, owner, KIND, &workspace.id, 1, &state).await?;
    }
    tx.commit().await?;
    Ok(Admission::Allowed)
}

pub(crate) async fn clear_workspace<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    workspace: &str,
) -> Result<(), DbErr> {
    document::Entity::delete_many()
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(KIND))
        .filter(document::Column::RefId.eq(workspace))
        .exec(db)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::admit_sample;

    #[test]
    fn rolling_window_expires_each_admission_after_sixty_seconds() {
        let mut samples = Vec::new();
        assert_eq!(admit_sample(&mut samples, 2, 1_000_000), None);
        assert_eq!(admit_sample(&mut samples, 2, 1_030_000), None);
        assert_eq!(admit_sample(&mut samples, 2, 1_059_999), Some(1));
        assert_eq!(admit_sample(&mut samples, 2, 1_060_000), None);
        assert_eq!(admit_sample(&mut samples, 2, 1_089_999), Some(1));
        assert_eq!(admit_sample(&mut samples, 2, 1_090_000), None);
    }
}
