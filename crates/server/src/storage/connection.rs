use super::migration::Migrator;
use anyhow::{Result, bail};
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;
use std::{path::Path, time::Duration};
pub fn sqlite_url(path: &Path) -> Result<String> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let mut options = std::fs::OpenOptions::new();
    options.create(true).write(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    // URL encoding preserves filenames containing query/fragment characters.
    let absolute = std::fs::canonicalize(path)?;
    let encoded = percent_encoding::percent_encode(
        absolute.as_os_str().as_encoded_bytes(),
        percent_encoding::NON_ALPHANUMERIC,
    );
    Ok(format!("sqlite://{encoded}?mode=rwc"))
}
pub async fn connect(url: &str) -> Result<DatabaseConnection> {
    let sqlite = url.starts_with("sqlite:");
    if !sqlite
        && !url.starts_with("postgres:")
        && !url.starts_with("postgresql:")
        && !url.starts_with("mysql:")
    {
        bail!("Unsupported database URL scheme");
    }
    if sqlite && !url.contains(":memory:") {
        let sqlite_options: sea_orm::sqlx::sqlite::SqliteConnectOptions = url
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid SQLite connection URL"))?;
        let decoded = sqlite_options.get_filename();
        if let Some(parent) = decoded.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
    }
    let mut opts = ConnectOptions::new(url);
    opts.min_connections(1)
        .max_connections(if sqlite { 1 } else { 8 })
        .connect_timeout(Duration::from_secs(10))
        .acquire_timeout(Duration::from_secs(10))
        .sqlx_logging(false);
    if sqlite {
        opts.map_sqlx_sqlite_opts(|o| {
            o.foreign_keys(true)
                .busy_timeout(Duration::from_secs(10))
                .journal_mode(sea_orm::sqlx::sqlite::SqliteJournalMode::Wal)
        });
    } else if url.starts_with("mysql:") {
        opts.map_sqlx_mysql_opts(|o| o.timezone(Some("+00:00".into())));
    } else {
        opts.map_sqlx_postgres_opts(|o| o.options([("timezone", "UTC")]));
    }
    let db = Database::connect(opts)
        .await
        .map_err(|_| anyhow::anyhow!("Database connection failed"))?;
    Migrator::up(&db, None)
        .await
        .map_err(|_| anyhow::anyhow!("Database migration failed"))?;
    Ok(db)
}
