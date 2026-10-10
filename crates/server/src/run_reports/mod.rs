mod projection;
use crate::{ApiError, AppState, auth::Identity, entities::document, storage, workspaces::owned};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use moleapi_core::{RunReportSummary, SavedRunReport};
pub(crate) use projection::Source;
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    TransactionTrait, sea_query::Expr,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
const KIND: &str = "run-report";
const BRIEF: &str = "run-brief";
fn brief_key(id: &str) -> String {
    format!("run-brief-{id}")
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct List {
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: Option<u64>,
}
#[derive(Serialize)]
pub struct Page {
    items: Vec<RunReportSummary>,
    next_cursor: Option<String>,
}
pub async fn list(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(workspace): Path<String>,
    Query(query): Query<List>,
) -> Result<Json<Page>, ApiError> {
    owned(&state, &owner.0, &workspace).await?;
    let limit = query.limit.unwrap_or(20);
    if !(1..=50).contains(&limit) {
        return Err(ApiError::bad("Report page requires 1 to 50 items"));
    }
    let mut request = document::Entity::find()
        .filter(document::Column::Owner.eq(&owner.0))
        .filter(document::Column::Kind.eq(BRIEF))
        .filter(document::Column::RefId.eq(&workspace));
    if let Some(cursor) = query.cursor {
        if cursor.len() > 512 {
            return Err(ApiError::bad("Invalid report cursor"));
        }
        let bytes = URL_SAFE_NO_PAD
            .decode(cursor)
            .map_err(|_| ApiError::bad("Invalid report cursor"))?;
        let (timestamp, id): (String, String) =
            serde_json::from_slice(&bytes).map_err(|_| ApiError::bad("Invalid report cursor"))?;
        if timestamp.len() > 40
            || id.len() > 128
            || chrono::DateTime::parse_from_rfc3339(&timestamp).is_err()
        {
            return Err(ApiError::bad("Invalid report cursor"));
        }
        request = request.filter(
            Condition::any()
                .add(document::Column::UpdatedAt.lt(&timestamp))
                .add(
                    Condition::all()
                        .add(document::Column::UpdatedAt.eq(timestamp))
                        .add(document::Column::Id.lt(id)),
                ),
        );
    }
    let mut rows = request
        .order_by_desc(document::Column::UpdatedAt)
        .order_by_desc(document::Column::Id)
        .limit(limit + 1)
        .all(&state.db)
        .await?;
    let next_cursor = if rows.len() > limit as usize {
        rows.truncate(limit as usize);
        rows.last()
            .map(|row| {
                serde_json::to_vec(&(&row.updated_at, &row.id))
                    .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
            })
            .transpose()
            .map_err(|_| ApiError::internal())?
    } else {
        None
    };
    let items = rows
        .into_iter()
        .map(|row| {
            serde_json::from_str::<RunReportSummary>(&row.payload).map_err(|_| ApiError::internal())
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(Page { items, next_cursor }))
}
async fn lookup(
    state: &AppState,
    owner: &str,
    workspace: &str,
    id: &str,
) -> Result<SavedRunReport, ApiError> {
    owned(state, owner, workspace).await?;
    let row = document::Entity::find_by_id(id)
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(KIND))
        .filter(document::Column::RefId.eq(workspace))
        .one(&state.db)
        .await?
        .ok_or_else(ApiError::not_found)?;
    serde_json::from_str(&row.payload).map_err(|_| ApiError::internal())
}
pub async fn get(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path((workspace, id)): Path<(String, String)>,
) -> Result<Json<SavedRunReport>, ApiError> {
    Ok(Json(lookup(&state, &owner.0, &workspace, &id).await?))
}
pub async fn response(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path((workspace, id, position)): Path<(String, String, usize)>,
) -> Result<Json<moleapi_core::HistoryEntry>, ApiError> {
    let report = lookup(&state, &owner.0, &workspace, &id).await?;
    let history_id = report
        .results
        .iter()
        .find(|step| step.position == position)
        .and_then(|step| step.history_id.as_ref())
        .ok_or_else(ApiError::not_found)?;
    let row = document::Entity::find_by_id(history_id)
        .filter(document::Column::Owner.eq(owner.0))
        .filter(document::Column::Kind.eq("history"))
        .filter(document::Column::RefId.eq(workspace))
        .one(&state.db)
        .await?
        .ok_or_else(ApiError::not_found)?;
    Ok(Json(
        serde_json::from_str(&row.payload).map_err(|_| ApiError::internal())?,
    ))
}
pub async fn remove(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path((workspace, id)): Path<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    lookup(&state, &owner.0, &workspace, &id).await?;
    document::Entity::delete_many()
        .filter(document::Column::Id.is_in([id.clone(), brief_key(&id)]))
        .filter(document::Column::Owner.eq(owner.0))
        .filter(document::Column::Kind.is_in([KIND, BRIEF]))
        .filter(document::Column::RefId.eq(workspace))
        .exec(&state.db)
        .await?;
    Ok(Json(json!({"deleted":true})))
}
pub async fn clear(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(workspace): Path<String>,
) -> Result<Json<Value>, ApiError> {
    owned(&state, &owner.0, &workspace).await?;
    document::Entity::delete_many()
        .filter(document::Column::Owner.eq(owner.0))
        .filter(document::Column::Kind.is_in([KIND, BRIEF]))
        .filter(document::Column::RefId.eq(workspace))
        .exec(&state.db)
        .await?;
    Ok(Json(json!({"deleted":true})))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Export {
    format: String,
    #[serde(default = "english")]
    language: String,
}
fn english() -> String {
    "en".into()
}
pub async fn export(
    State(state): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path((workspace, id)): Path<(String, String)>,
    Query(options): Query<Export>,
) -> Result<Json<moleapi_formats::ExportResult>, ApiError> {
    let report = lookup(&state, &owner.0, &workspace, &id).await?;
    Ok(Json(
        moleapi_formats::export_run_report(&report, &options.format, &options.language)
            .map_err(|error| ApiError::bad(error.to_string()))?,
    ))
}
pub(crate) async fn record(
    state: &AppState,
    owner: &str,
    source: &Source<'_>,
    live: &Value,
) -> Result<Option<String>, ApiError> {
    let tx = state.db.begin().await?;
    let result = record_in(&tx, owner, source, live).await?;
    tx.commit().await?;
    Ok(result)
}
pub(crate) async fn record_in<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    source: &Source<'_>,
    live: &Value,
) -> Result<Option<String>, ApiError> {
    let report = projection::project(source, live)?;
    // A no-op workspace update acquires the database's write/row lock without changing its revision or data.
    document::Entity::update_many()
        .col_expr(
            document::Column::Revision,
            Expr::col(document::Column::Revision).into(),
        )
        .filter(document::Column::Id.eq(storage::workspace_key(owner, &source.workspace.id)))
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq("workspace"))
        .exec(db)
        .await?;
    if !source_exists(db, owner, source.workspace).await? {
        return Ok(None);
    }

    storage::insert_doc(
        db,
        report.id.clone(),
        owner,
        KIND,
        &source.workspace.id,
        0,
        &report,
    )
    .await?;
    storage::insert_doc(
        db,
        brief_key(&report.id),
        owner,
        BRIEF,
        &source.workspace.id,
        0,
        &report.brief(),
    )
    .await?;
    let old = document::Entity::find()
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(BRIEF))
        .filter(document::Column::RefId.eq(&source.workspace.id))
        .order_by_desc(document::Column::UpdatedAt)
        .order_by_desc(document::Column::Id)
        .offset(100)
        .limit(1000)
        .all(db)
        .await?;
    if !old.is_empty() {
        document::Entity::delete_many()
            .filter(document::Column::Owner.eq(owner))
            .filter(document::Column::Kind.is_in([KIND, BRIEF]))
            .filter(document::Column::Id.is_in(old.into_iter().flat_map(|row| {
                let id = row
                    .id
                    .strip_prefix("run-brief-")
                    .unwrap_or(&row.id)
                    .to_string();
                [id, row.id]
            })))
            .exec(db)
            .await?;
    }
    crate::notifications::enqueue_run(db, owner, source, &report).await?;
    Ok(Some(report.id))
}

async fn source_exists<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    source: &moleapi_core::Workspace,
) -> Result<bool, ApiError> {
    let Some(current) = storage::get(db, owner, &source.id).await? else {
        return Ok(false);
    };
    if current.revision == source.revision {
        return Ok(true);
    }
    Ok(document::Entity::find()
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq("version"))
        .filter(document::Column::RefId.eq(&source.id))
        .filter(document::Column::Revision.eq(source.revision))
        .one(db)
        .await?
        .is_some())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn source_revision_lineage_allows_edits_but_fences_deleted_and_recreated_workspaces() {
        let temp = tempfile::tempdir().unwrap();
        let db = storage::connect(&storage::sqlite_url(&temp.path().join("lineage.db")).unwrap())
            .await
            .unwrap();
        let original:moleapi_core::Workspace=serde_json::from_value(json!({"id":"w","name":"Original","revision":1,"updated_at":"now","data":{"schema_version":1,"collections":[],"environments":[],"active_environment_id":null}})).unwrap();
        storage::insert_doc(
            &db,
            storage::workspace_key("owner", "w"),
            "owner",
            "workspace",
            "w",
            1,
            &original,
        )
        .await
        .unwrap();
        let mut edited = original.clone();
        edited.name = "Edited".into();
        storage::replace(&db, "owner", edited, 1)
            .await
            .unwrap()
            .unwrap();
        assert!(source_exists(&db, "owner", &original).await.unwrap());
        document::Entity::delete_many()
            .filter(document::Column::Owner.eq("owner"))
            .filter(document::Column::RefId.eq("w"))
            .exec(&db)
            .await
            .unwrap();
        let mut recreated = original.clone();
        recreated.revision = 3;
        recreated.name = "Recreated".into();
        storage::insert_doc(
            &db,
            storage::workspace_key("owner", "w"),
            "owner",
            "workspace",
            "w",
            3,
            &recreated,
        )
        .await
        .unwrap();
        assert!(!source_exists(&db, "owner", &original).await.unwrap());
        assert!(source_exists(&db, "owner", &recreated).await.unwrap());
    }
}
