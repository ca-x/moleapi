use crate::entities::{account, session, setting};
use sea_orm::{ActiveModelTrait, DbBackend, Set};
use sea_orm_migration::prelude::*;
pub struct Migrator;
#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(Initial),
            Box::new(ScheduledDueIndex),
            Box::new(AccessTokens),
            Box::new(WorkspaceCollaboration),
        ]
    }
}
#[derive(DeriveMigrationName)]
struct Initial;
#[async_trait::async_trait]
impl MigrationTrait for Initial {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let schema = sea_orm::Schema::new(manager.get_database_backend());
        let tables = [
            schema.create_table_from_entity(account::Entity),
            schema.create_table_from_entity(session::Entity),
            schema.create_table_from_entity(setting::Entity),
        ];
        for mut table in tables {
            table.if_not_exists();
            if manager.get_database_backend() == DbBackend::MySql {
                table.extra("ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin");
            }
            manager.create_table(table).await?;
        }
        let mut documents = Table::create();
        documents
            .table(Alias::new("documents"))
            .if_not_exists()
            .col(
                ColumnDef::new(Alias::new("id"))
                    .string_len(128)
                    .not_null()
                    .primary_key(),
            )
            .col(
                ColumnDef::new(Alias::new("owner"))
                    .string_len(64)
                    .not_null(),
            )
            .col(ColumnDef::new(Alias::new("kind")).string_len(16).not_null())
            .col(
                ColumnDef::new(Alias::new("ref_id"))
                    .string_len(128)
                    .not_null(),
            )
            .col(
                ColumnDef::new(Alias::new("revision"))
                    .big_integer()
                    .not_null(),
            )
            .col(
                ColumnDef::new(Alias::new("updated_at"))
                    .string_len(40)
                    .not_null(),
            );
        let mut payload = ColumnDef::new(Alias::new("payload"));
        payload.not_null();
        if manager.get_database_backend() == DbBackend::MySql {
            payload.custom(Alias::new("LONGTEXT"));
            documents.extra("ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin");
        } else {
            payload.text();
        }
        documents.col(&mut payload);
        manager.create_table(documents).await?;
        setting::ActiveModel {
            id: Set("registration".into()),
            revision: Set(0),
        }
        .insert(manager.get_connection())
        .await?;
        manager
            .create_index(
                Index::create()
                    .name("documents_owner_kind_ref")
                    .table(Alias::new("documents"))
                    .col(Alias::new("owner"))
                    .col(Alias::new("kind"))
                    .col(Alias::new("ref_id"))
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for name in ["documents", "sessions", "accounts", "settings"] {
            manager
                .drop_table(Table::drop().table(Alias::new(name)).to_owned())
                .await?;
        }
        Ok(())
    }
}

struct ScheduledDueIndex;
impl MigrationName for ScheduledDueIndex {
    fn name(&self) -> &str {
        "m20261010_schedule_due_index"
    }
}
#[async_trait::async_trait]
impl MigrationTrait for ScheduledDueIndex {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .name("documents_kind_due")
                    .table(Alias::new("documents"))
                    .col(Alias::new("kind"))
                    .col(Alias::new("revision"))
                    .to_owned(),
            )
            .await
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("documents_kind_due")
                    .table(Alias::new("documents"))
                    .to_owned(),
            )
            .await
    }
}

struct AccessTokens;
impl MigrationName for AccessTokens {
    fn name(&self) -> &str {
        "m20261010_personal_access_tokens"
    }
}

struct WorkspaceCollaboration;
impl MigrationName for WorkspaceCollaboration {
    fn name(&self) -> &str {
        "m20261019_workspace_collaboration"
    }
}
#[async_trait::async_trait]
impl MigrationTrait for WorkspaceCollaboration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let mut members = Table::create();
        members
            .table(Alias::new("workspace_memberships"))
            .col(
                ColumnDef::new(Alias::new("id"))
                    .string_len(128)
                    .not_null()
                    .primary_key(),
            )
            .col(
                ColumnDef::new(Alias::new("owner_id"))
                    .string_len(64)
                    .not_null(),
            )
            .col(
                ColumnDef::new(Alias::new("workspace_id"))
                    .string_len(128)
                    .not_null(),
            )
            .col(
                ColumnDef::new(Alias::new("member_id"))
                    .string_len(64)
                    .not_null(),
            )
            .col(ColumnDef::new(Alias::new("role")).string_len(16).not_null())
            .col(
                ColumnDef::new(Alias::new("generation"))
                    .string_len(36)
                    .not_null(),
            )
            .col(
                ColumnDef::new(Alias::new("revision"))
                    .big_integer()
                    .not_null(),
            )
            .col(
                ColumnDef::new(Alias::new("created_at"))
                    .big_integer()
                    .not_null(),
            )
            .col(
                ColumnDef::new(Alias::new("updated_at"))
                    .big_integer()
                    .not_null(),
            );
        if manager.get_database_backend() == DbBackend::MySql {
            members.extra("ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin");
        }
        manager.create_table(members).await?;
        manager
            .create_index(
                Index::create()
                    .name("workspace_memberships_owner_workspace_member")
                    .table(Alias::new("workspace_memberships"))
                    .col(Alias::new("owner_id"))
                    .col(Alias::new("workspace_id"))
                    .col(Alias::new("member_id"))
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("workspace_memberships_member")
                    .table(Alias::new("workspace_memberships"))
                    .col(Alias::new("member_id"))
                    .to_owned(),
            )
            .await?;

        let mut invites = Table::create();
        invites
            .table(Alias::new("workspace_invitations"))
            .col(
                ColumnDef::new(Alias::new("id"))
                    .string_len(128)
                    .not_null()
                    .primary_key(),
            )
            .col(
                ColumnDef::new(Alias::new("owner_id"))
                    .string_len(64)
                    .not_null(),
            )
            .col(
                ColumnDef::new(Alias::new("workspace_id"))
                    .string_len(128)
                    .not_null(),
            )
            .col(
                ColumnDef::new(Alias::new("token_digest"))
                    .string_len(64)
                    .not_null(),
            )
            .col(ColumnDef::new(Alias::new("role")).string_len(16).not_null())
            .col(
                ColumnDef::new(Alias::new("created_by"))
                    .string_len(64)
                    .not_null(),
            )
            .col(
                ColumnDef::new(Alias::new("created_at"))
                    .big_integer()
                    .not_null(),
            )
            .col(
                ColumnDef::new(Alias::new("expires_at"))
                    .big_integer()
                    .not_null(),
            )
            .col(ColumnDef::new(Alias::new("consumed_by")).string_len(64))
            .col(ColumnDef::new(Alias::new("revoked_at")).big_integer())
            .col(
                ColumnDef::new(Alias::new("revision"))
                    .big_integer()
                    .not_null(),
            );
        if manager.get_database_backend() == DbBackend::MySql {
            invites.extra("ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin");
        }
        manager.create_table(invites).await?;
        manager
            .create_index(
                Index::create()
                    .name("workspace_invitations_token_digest")
                    .table(Alias::new("workspace_invitations"))
                    .col(Alias::new("token_digest"))
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("workspace_invitations_owner_workspace")
                    .table(Alias::new("workspace_invitations"))
                    .col(Alias::new("owner_id"))
                    .col(Alias::new("workspace_id"))
                    .to_owned(),
            )
            .await?;
        let schema = sea_orm::Schema::new(manager.get_database_backend());
        let mut audit =
            schema.create_table_from_entity(crate::entities::workspace_audit_event::Entity);
        if manager.get_database_backend() == DbBackend::MySql {
            audit.extra("ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin");
        }
        manager.create_table(audit).await?;
        manager
            .create_index(
                Index::create()
                    .name("workspace_audit_owner_workspace_time")
                    .table(Alias::new("workspace_audit_events"))
                    .col(Alias::new("owner_id"))
                    .col(Alias::new("workspace_id"))
                    .col(Alias::new("created_at"))
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("workspace_audit_events"))
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("workspace_invitations"))
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("workspace_memberships"))
                    .to_owned(),
            )
            .await
    }
}
#[async_trait::async_trait]
impl MigrationTrait for AccessTokens {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let schema = sea_orm::Schema::new(manager.get_database_backend());
        let mut table = schema.create_table_from_entity(crate::entities::access_token::Entity);
        if manager.get_database_backend() == DbBackend::MySql {
            table.extra("ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin");
        }
        manager.create_table(table).await?;
        manager
            .create_index(
                Index::create()
                    .name("access_tokens_owner_created")
                    .table(Alias::new("access_tokens"))
                    .col(Alias::new("owner"))
                    .col(Alias::new("created_at"))
                    .to_owned(),
            )
            .await
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Alias::new("access_tokens")).to_owned())
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectionTrait, Database, EntityTrait, Statement};
    #[tokio::test]
    async fn access_token_upgrade_preserves_existing_accounts_sessions_and_version_history() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, Some(2)).await.unwrap();
        account::ActiveModel {
            id: Set("existing".into()),
            username: Set("existing-user".into()),
            password_hash: Set("saved-password-digest".into()),
        }
        .insert(&db)
        .await
        .unwrap();
        session::ActiveModel {
            id: Set("saved-session-digest".into()),
            owner: Set("existing".into()),
            expires_at: Set(2000000000),
        }
        .insert(&db)
        .await
        .unwrap();
        Migrator::up(&db, None).await.unwrap();
        assert_eq!(
            account::Entity::find_by_id("existing")
                .one(&db)
                .await
                .unwrap()
                .unwrap()
                .password_hash,
            "saved-password-digest"
        );
        assert_eq!(
            session::Entity::find_by_id("saved-session-digest")
                .one(&db)
                .await
                .unwrap()
                .unwrap()
                .owner,
            "existing"
        );
        assert_eq!(
            Migrator::get_applied_migrations(&db).await.unwrap().len(),
            4
        );
        assert!(
            crate::entities::access_token::Entity::find()
                .all(&db)
                .await
                .unwrap()
                .is_empty()
        );
        Migrator::up(&db, None).await.unwrap();
        assert_eq!(
            Migrator::get_applied_migrations(&db).await.unwrap().len(),
            4
        );
    }

    #[tokio::test]
    async fn membership_upgrade_keeps_canonical_workspace_and_adds_indexed_tables() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, Some(3)).await.unwrap();
        crate::entities::document::ActiveModel {
            id: Set("owner:w".into()),
            owner: Set("owner".into()),
            kind: Set("workspace".into()),
            ref_id: Set("w".into()),
            revision: Set(7),
            payload: Set("{\"id\":\"w\",\"revision\":7}".into()),
            updated_at: Set("now".into()),
        }
        .insert(&db)
        .await
        .unwrap();
        Migrator::up(&db, None).await.unwrap();
        assert_eq!(
            Migrator::get_applied_migrations(&db).await.unwrap().len(),
            4
        );
        assert_eq!(
            crate::entities::document::Entity::find_by_id("owner:w")
                .one(&db)
                .await
                .unwrap()
                .unwrap()
                .payload,
            "{\"id\":\"w\",\"revision\":7}"
        );
        for table in [
            "workspace_memberships",
            "workspace_invitations",
            "workspace_audit_events",
        ] {
            let row = db
                .query_one(Statement::from_string(
                    DbBackend::Sqlite,
                    format!("SELECT COUNT(*) AS count FROM {table}"),
                ))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(row.try_get::<i64>("", "count").unwrap(), 0);
        }
        Migrator::up(&db, None).await.unwrap();
        assert_eq!(
            Migrator::get_applied_migrations(&db).await.unwrap().len(),
            4
        );
    }
}
