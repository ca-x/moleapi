//! Dedicated database/file SQL adapters; application storage is independent.
mod models;
pub use models::*;
mod query;
pub use query::{quoted, statement};

mod files;
pub use files::FileSource;

mod file_worker;
pub use file_worker::{FileOutput, dispatch_file_worker, run_file_worker};

#[cfg(test)]
mod files_tests;

mod endpoint;
pub use endpoint::Endpoint;
mod io;
mod postgres;
pub use postgres::{PgCancel, PgSource};

mod mysql;
pub use mysql::{MysqlCancel, MysqlSource};
mod schema;
pub use schema::{MYSQL_SCHEMA_SQL, PG_SCHEMA_SQL, schema_tables};

mod connection;
pub use connection::{Cancellation, Connection, ConnectionInfo};
