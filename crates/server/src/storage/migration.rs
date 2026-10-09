use crate::entities::{account, session, setting};
use sea_orm::{ActiveModelTrait, DbBackend, Set};
use sea_orm_migration::prelude::*;
pub struct Migrator;
#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(Initial), Box::new(ScheduledDueIndex)]
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
