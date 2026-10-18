use crate::{ApiError, AppState, auth::Identity, entities::document, storage, workspaces::owned};
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use sea_orm::{
    ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, TransactionTrait,
    sea_query::Expr,
};
use serde::{Deserialize, Serialize};

const KIND: &str = "mock-call";
const SEQUENCE_KIND: &str = "mock-call-seq";
const _: () = assert!(KIND.len() <= 16 && SEQUENCE_KIND.len() <= 16);
const RETAIN: i64 = 200;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MockCall {
    pub id: String,
    pub workspace_id: String,
    pub request_id: Option<String>,
    pub example_id: Option<String>,
    pub sequence: i64,
    pub method: String,
    pub response_status: u16,
    pub outcome: String,
    pub at: String,
    pub elapsed_ms: u64,
}

pub(crate) struct CallMeta<'a> {
    pub method: &'a str,
    pub request_id: Option<&'a str>,
    pub example_id: Option<&'a str>,
    pub status: StatusCode,
    pub outcome: &'a str,
    pub elapsed_ms: u64,
}

fn sequence_id(owner: &str, workspace: &str) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "mc_{}",
        hex::encode(Sha256::digest(format!("{owner}\0{workspace}").as_bytes()))
    )
}

pub(crate) async fn lock_workspace<C: sea_orm::ConnectionTrait>(
    db: &C,
    owner: &str,
    workspace: &str,
) -> Result<moleapi_core::Workspace, ApiError> {
    document::Entity::update_many()
        .col_expr(
            document::Column::Revision,
            Expr::col(document::Column::Revision).into(),
        )
        .filter(document::Column::Id.eq(storage::workspace_key(owner, workspace)))
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq("workspace"))
        .exec(db)
        .await?;
    storage::get(db, owner, workspace)
        .await?
        .ok_or_else(ApiError::not_found)
}

pub(crate) async fn record(
    s: &AppState,
    owner: &str,
    workspace: &moleapi_core::Workspace,
    meta: CallMeta<'_>,
) -> Result<(), ApiError> {
    let tx = s.db.begin().await?;
    let current = lock_workspace(&tx, owner, &workspace.id).await?;
    if current.revision != workspace.revision {
        return Err(ApiError::conflict("Workspace changed during Mock call"));
    }
    let state_id = sequence_id(owner, &workspace.id);
    let state = document::Entity::find_by_id(&state_id)
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(SEQUENCE_KIND))
        .filter(document::Column::RefId.eq(&workspace.id))
        .one(&tx)
        .await?;
    if state.is_none() {
        storage::insert_doc(
            &tx,
            state_id.clone(),
            owner,
            SEQUENCE_KIND,
            &workspace.id,
            0,
            &serde_json::json!({}),
        )
        .await?;
    }
    let changed = document::Entity::update_many()
        .col_expr(
            document::Column::Revision,
            Expr::col(document::Column::Revision).add(1),
        )
        .filter(document::Column::Id.eq(&state_id))
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(SEQUENCE_KIND))
        .filter(document::Column::Revision.lt(i64::MAX))
        .exec(&tx)
        .await?;
    if changed.rows_affected != 1 {
        return Err(ApiError::internal());
    }
    let state = document::Entity::find_by_id(&state_id)
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(SEQUENCE_KIND))
        .one(&tx)
        .await?
        .ok_or_else(ApiError::internal)?;
    let sequence = state.revision;
    let call = MockCall {
        id: format!("{state_id}_{sequence}"),
        workspace_id: workspace.id.clone(),
        request_id: meta.request_id.map(String::from),
        example_id: meta.example_id.map(String::from),
        sequence,
        method: match meta.method {
            "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS" => meta.method,
            _ => "OTHER",
        }
        .into(),
        response_status: meta.status.as_u16(),
        outcome: meta.outcome.into(),
        at: storage::now(),
        elapsed_ms: meta.elapsed_ms,
    };
    storage::insert_doc(
        &tx,
        call.id.clone(),
        owner,
        KIND,
        &workspace.id,
        sequence,
        &call,
    )
    .await?;
    if sequence > RETAIN {
        document::Entity::delete_many()
            .filter(document::Column::Owner.eq(owner))
            .filter(document::Column::Kind.eq(KIND))
            .filter(document::Column::RefId.eq(&workspace.id))
            .filter(document::Column::Revision.lte(sequence - RETAIN))
            .exec(&tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn list(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
) -> Result<Json<Vec<MockCall>>, ApiError> {
    owned(&s, &owner.0, &id).await?;
    let rows = document::Entity::find()
        .filter(document::Column::Owner.eq(&owner.0))
        .filter(document::Column::Kind.eq(KIND))
        .filter(document::Column::RefId.eq(&id))
        .order_by_desc(document::Column::Revision)
        .limit(RETAIN as u64)
        .all(&s.db)
        .await?;
    let calls = rows
        .into_iter()
        .map(|row| {
            let call: MockCall =
                serde_json::from_str(&row.payload).map_err(|_| ApiError::internal())?;
            if call.id != row.id || call.workspace_id != id || call.sequence != row.revision {
                return Err(ApiError::internal());
            }
            Ok(call)
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    Ok(Json(calls))
}

pub async fn clear(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let tx = s.db.begin().await?;
    lock_workspace(&tx, &owner.0, &id).await?;
    document::Entity::delete_many()
        .filter(document::Column::Owner.eq(&owner.0))
        .filter(document::Column::Kind.eq(KIND))
        .filter(document::Column::RefId.eq(&id))
        .exec(&tx)
        .await?;
    tx.commit().await?;
    Ok(Json(serde_json::json!({"ok":true})))
}
