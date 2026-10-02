# Storage and database requirement

User confirmed 2026-10-02: SQLite is default; hosted server also supports PostgreSQL and MySQL. This is the application backing store, separate from Data client/SQL test target database connections.

Use SeaORM + sea-orm-migration, as Raindrop does, with sqlx-sqlite/sqlx-postgres/sqlx-mysql drivers and Tokio/rustls. No hand-written ORM or dialect substitution layer. Config `MOLEAPI_DATABASE_URL` / `--database-url`: sqlite://path?mode=rwc (default ./data/moleapi.db), postgresql://..., mysql://.... Passwords/URLs never appear in error or debug logs. Native offline client constructs its own SQLite URL in app-data.

All three use the same entities, migration sequence and API/storage semantics. Use portable UUID string IDs, UTC timestamps, JSON text where portable JSON behavior is required; avoid SQLite-only SQL, last_insert_rowid and INSERT OR REPLACE. SeaQuery migration builders render each dialect. MySQL utf8mb4 and transactional InnoDB; PostgreSQL UTC; SQLite foreign_keys + WAL + busy timeout and serialized writer pool.

A stale revision must be rejected atomically by `UPDATE ... WHERE id AND revision` and affected_rows==1; snapshot and revision changes occur in one transaction. First-admin creation and unique names must use database constraints, not read-then-write races. Tenant/membership boundaries apply on all engines. Switching URL does not migrate existing user data automatically; explicit full export/import preserves IDs and references.

Verification: real schema migration, account/session/workspace CRUD, revision contention and backups on SQLite; GitHub Actions service containers PostgreSQL 16 and MySQL 8.4 run identical integration tests via env URLs. Migrating twice must be idempotent. Cross-dialect SQL generation isn't a substitute for running all three engines.

Deleting a workspace removes its payload/history/base and retains only an ID/revision tombstone. Recreating the same ID continues the revision sequence; revision values are fencing tokens, never reset, so deleting/recreating between a sync read and CAS write cannot admit a stale write. Tombstone kind fits the same portable schema.
