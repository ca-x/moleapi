use anyhow::{Result, ensure};
use moleapi_core::{DataSource, MAX_SQL_SOURCE};
use sqlparser::{
    ast::{Statement, Visit, Visitor},
    dialect::{Dialect, GenericDialect, MySqlDialect, PostgreSqlDialect},
    parser::Parser,
};
use std::ops::ControlFlow;
struct Complexity {
    nodes: usize,
}
impl Visitor for Complexity {
    type Break = ();
    fn pre_visit_expr(&mut self, expression: &sqlparser::ast::Expr) -> ControlFlow<()> {
        if let sqlparser::ast::Expr::Function(function) = expression
            && function
                .name
                .0
                .last()
                .and_then(|part| part.as_ident())
                .is_some_and(|name| {
                    [
                        "set_config",
                        "pg_advisory_lock",
                        "pg_advisory_lock_shared",
                        "pg_try_advisory_lock",
                        "pg_try_advisory_lock_shared",
                        "pg_advisory_unlock",
                        "pg_advisory_unlock_shared",
                        "pg_advisory_unlock_all",
                    ]
                    .iter()
                    .any(|blocked| name.value.eq_ignore_ascii_case(blocked))
                })
        {
            return ControlFlow::Break(());
        }
        self.nodes += 1;
        if self.nodes > 4096 {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }
    fn pre_visit_statement(&mut self, statement: &Statement) -> ControlFlow<()> {
        if matches!(
            statement,
            Statement::StartTransaction { .. }
                | Statement::Commit { .. }
                | Statement::Rollback { .. }
                | Statement::LISTEN { .. }
                | Statement::UNLISTEN { .. }
                | Statement::AlterSession { .. }
                | Statement::Set(_)
                | Statement::Reset(_)
                | Statement::Use(_)
                | Statement::Savepoint { .. }
                | Statement::ReleaseSavepoint { .. }
                | Statement::Prepare { .. }
                | Statement::Execute { .. }
                | Statement::Deallocate { .. }
                | Statement::Discard { .. }
                | Statement::LockTables { .. }
                | Statement::UnlockTables
                | Statement::Kill { .. }
                | Statement::Declare { .. }
                | Statement::Fetch { .. }
                | Statement::Close { .. }
                | Statement::AttachDatabase { .. }
                | Statement::AttachDuckDBDatabase { .. }
                | Statement::DetachDuckDBDatabase { .. }
        ) {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }
}
pub fn statement(sql: &str, source: DataSource, read_only: bool) -> Result<Statement> {
    ensure!(
        !sql.trim().is_empty() && sql.len() <= MAX_SQL_SOURCE,
        "Query SQL exceeds limits or is empty"
    );
    let dialect: Box<dyn Dialect> = match source {
        DataSource::Postgresql => Box::new(PostgreSqlDialect {}),
        DataSource::Mysql => Box::new(MySqlDialect {}),
        _ => Box::new(GenericDialect {}),
    };
    let mut statements = Parser::parse_sql(dialect.as_ref(), sql)?;
    ensure!(statements.len() == 1, "Run only the selected SQL statement");
    let parsed = statements.remove(0);
    ensure!(
        parsed.visit(&mut Complexity { nodes: 0 }).is_continue(),
        "SQL complexity/session-control limits exceeded"
    );
    if read_only || matches!(source, DataSource::LocalFile | DataSource::RemoteFile) {
        ensure!(
            matches!(&parsed, Statement::Query(_)),
            "Read-only/file requests accept queries only"
        );
    }
    Ok(parsed)
}
pub fn quoted(name: &str, source: DataSource) -> String {
    sqlparser::ast::Ident::with_quote(
        if source == DataSource::Mysql {
            '`'
        } else {
            '"'
        },
        name,
    )
    .to_string()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn managed_connections_reject_session_controls_even_in_write_mode() {
        for (source, sql) in [
            (DataSource::Postgresql, "SET bytea_output = 'escape'"),
            (DataSource::Postgresql, "RESET ALL"),
            (DataSource::Postgresql, "SAVEPOINT user_savepoint"),
            (DataSource::Postgresql, "RELEASE SAVEPOINT user_savepoint"),
            (DataSource::Postgresql, "PREPARE user_query AS SELECT 1"),
            (DataSource::Postgresql, "EXECUTE user_query"),
            (DataSource::Postgresql, "DEALLOCATE user_query"),
            (DataSource::Postgresql, "DISCARD ALL"),
            (DataSource::Postgresql, "LISTEN channel"),
            (DataSource::Postgresql, "UNLISTEN *"),
            (
                DataSource::Postgresql,
                "SELECT pg_catalog.set_config('bytea_output','escape',false)",
            ),
            (DataSource::Postgresql, "SELECT pg_advisory_lock(1)"),
            (DataSource::Mysql, "USE other_database"),
            (DataSource::Mysql, "SET autocommit = 1"),
            (DataSource::Mysql, "/*! SET autocommit = 1 */"),
            (DataSource::Mysql, "LOCK TABLES items WRITE"),
            (DataSource::Mysql, "UNLOCK TABLES"),
            (DataSource::Mysql, "KILL QUERY 123"),
        ] {
            assert!(
                statement(sql, source, false).is_err(),
                "accepted {source:?}: {sql}"
            );
        }
        statement("CREATE TABLE items (id BIGINT)", DataSource::Mysql, false).unwrap();
        statement(
            "INSERT INTO items VALUES (1)",
            DataSource::Postgresql,
            false,
        )
        .unwrap();
    }
    #[test]
    fn uses_sql_ast_for_strings_comments_single_queries_and_write_mode() {
        statement(
            "-- comment\nSELECT ';' AS text",
            DataSource::Postgresql,
            true,
        )
        .unwrap();
        statement("UPDATE items SET n=1", DataSource::Mysql, false).unwrap();
        for sql in [
            "SELECT 1;SELECT 2",
            "/* SELECT */ DELETE FROM items",
            "COMMIT",
            "BEGIN",
        ] {
            assert!(statement(sql, DataSource::Postgresql, true).is_err());
        }
        assert_eq!(quoted("a\"b", DataSource::Postgresql), "\"a\"\"b\"");
    }
}
