mod connection;
mod migration;
use crate::entities::document;
pub use connection::{connect, sqlite_url};
use moleapi_core::Workspace;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait, sea_query::Expr,
};
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
pub fn workspace_key(owner: &str, id: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(format!("{owner}\0{id}")))
}
pub async fn get<C: ConnectionTrait>(
    db: &C,
    owner: &str,
    id: &str,
) -> Result<Option<Workspace>, DbErr> {
    document::Entity::find_by_id(workspace_key(owner, id))
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq("workspace"))
        .one(db)
        .await?
        .map(|m| {
            serde_json::from_str(&m.payload).map_err(|_| DbErr::Custom("Corrupt workspace".into()))
        })
        .transpose()
}
pub async fn insert_doc<C: ConnectionTrait, T: serde::Serialize>(
    db: &C,
    id: String,
    owner: &str,
    kind: &str,
    ref_id: &str,
    revision: i64,
    payload: &T,
) -> Result<(), DbErr> {
    document::ActiveModel {
        id: Set(id),
        owner: Set(owner.into()),
        kind: Set(kind.into()),
        ref_id: Set(ref_id.into()),
        revision: Set(revision),
        payload: Set(serde_json::to_string(payload)
            .map_err(|_| DbErr::Custom("Serialization failed".into()))?),
        updated_at: Set(now()),
    }
    .insert(db)
    .await?;
    Ok(())
}
pub async fn documents<T: serde::de::DeserializeOwned>(
    db: &DatabaseConnection,
    owner: &str,
    kind: &str,
    ref_id: Option<&str>,
    limit: u64,
) -> Result<Vec<T>, DbErr> {
    let mut q = document::Entity::find()
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Kind.eq(kind));
    if let Some(id) = ref_id {
        q = q.filter(document::Column::RefId.eq(id));
    }
    q.order_by_desc(document::Column::UpdatedAt)
        .limit(limit)
        .all(db)
        .await?
        .into_iter()
        .map(|m| {
            serde_json::from_str(&m.payload).map_err(|_| DbErr::Custom("Corrupt document".into()))
        })
        .collect()
}
pub async fn replace(
    db: &DatabaseConnection,
    owner: &str,
    next: Workspace,
    expected: i64,
) -> Result<Option<Workspace>, DbErr> {
    let tx = db.begin().await?;
    let result = replace_in(&tx, owner, next, expected).await?;
    tx.commit().await?;
    Ok(result)
}
pub async fn replace_in<C: ConnectionTrait>(
    tx: &C,
    owner: &str,
    mut next: Workspace,
    expected: i64,
) -> Result<Option<Workspace>, DbErr> {
    let Some(previous) = get(tx, owner, &next.id).await? else {
        return Ok(None);
    };
    if previous.revision != expected {
        return Ok(None);
    }
    next.revision = expected + 1;
    next.updated_at = now();
    let updated = document::Entity::update_many()
        .col_expr(
            document::Column::Payload,
            Expr::value(
                serde_json::to_string(&next)
                    .map_err(|_| DbErr::Custom("Serialization failed".into()))?,
            ),
        )
        .col_expr(document::Column::Revision, Expr::value(next.revision))
        .col_expr(document::Column::UpdatedAt, Expr::value(&next.updated_at))
        .filter(document::Column::Id.eq(workspace_key(owner, &next.id)))
        .filter(document::Column::Owner.eq(owner))
        .filter(document::Column::Revision.eq(expected))
        .exec(tx)
        .await?;
    if updated.rows_affected != 1 {
        return Ok(None);
    }
    insert_doc(
        tx,
        uuid::Uuid::new_v4().to_string(),
        owner,
        "version",
        &next.id,
        previous.revision,
        &previous,
    )
    .await?;
    Ok(Some(next))
}
