# Independent backend fix review — 2026-10-05

## Final focused re-review

### Text-protocol classification and disposable schema sockets addendum

Inspected the later MySQL cell-classification fix and schema connection changes. No definite important issue was identified in this narrow source pass. `Value::Bytes` now uses the SDK's `ColumnType::is_numeric_type()` for integer classification, with DECIMAL/NEWDECIMAL/FLOAT/DOUBLE returned as exact received decimal strings before charset-based binary handling. Numeric columns no longer become Binary merely because charset is 63. Temporal values remain text; binary classification is limited to the listed blob/string/bit/geometry/vector types. The resolved SDK helper also includes YEAR among numeric types. The new test covers actual TextProtocol-style Bytes for BIGINT, DECIMAL, DOUBLE and DATE, plus binary bytes.

MySQL metadata opens a disposable direct `Conn` from the same checked options and aborts it after collection, including an error result. Dropping a canceled open/collect future retains the immediate-drop transport disposal behavior. PostgreSQL metadata connects with the cloned checked endpoint, and the local `PgSource` drop aborts its SDK task after collection/error/cancellation. These side connections preserve pinned addresses, TLS host/config, credentials and destination database, while capped metadata no longer leaves a pending stream on the reusable user query connection. Existing outer schema operation deadlines and permits cover these operations.

The parent's reported 246 passing workspace tests and classification red/green evidence were not independently rerun by this reviewer. Real PostgreSQL/MySQL schema, cap, TLS, cancellation and server-side session disappearance remain pending CI. This addendum supplies source review, not new live database evidence.

The two P2 findings below are resolved in the current source. They remain recorded as the earlier review snapshot, not current open findings.

`Connection::is_closed()` now matches PostgreSQL and MySQL. PostgreSQL `PgSource::is_closed()` combines its explicit capped/aborted flag with the SDK closed state. A capped PostgreSQL query preserves captured rows, marks closed and aborts the SDK connection task; the actor emits the result then uses its closed-connection branch to return a terminal/reconnect reason. MySQL continues to abort its transport synchronously. No definite remaining important connection-reuse or permit-lifetime failure was identified in this focused source pass.

The visitor now rejects `Statement::LISTEN`, `Statement::UNLISTEN`, and `Statement::AlterSession`, as well as the prior session/transaction controls. The uppercase variant names are real SQLparser 0.63 variants: resolved `src/ast/mod.rs:4879,4889`; the parser has dedicated LISTEN/UNLISTEN parsing, so they were accepted commands before this change, not already-rejected unsupported syntax. The `Expr::Function` check examines the mature AST's final function-name identifier and blocks `set_config` and session advisory-lock/unlock builtins case-insensitively, including the schema-qualified `pg_catalog.set_config` spelling. Transaction-scoped advisory functions remain permitted. This addresses the concrete persistent-state reproduction reported below.

This is not a database-function sandbox: privileges and behavior of user-defined functions/procedures remain governed by the actual database. Source inspection does not prove server-side connection disappearance or cancel/commit timing. Real database cap, bytea-after-rejection, cancellation, and session-ID disappearance fixtures remain pending Actions. No tests were executed by this reviewer during the final focused pass.

Read-only implementation review against `docs/specs/data-client.md`; only this review artifact was written. Reviewed the current four-finding fixes, including the later scoped `serde_json::RawValue` implementation. No live database verification, new test run, commit, or parity claim.

## Definite remaining findings

### P2 — PostgreSQL fatal transport failure leaves the protocol session open

Location: `crates/data/src/connection.rs:151-153`; `crates/protocols/src/data.rs:328-338`.

`Connection::is_closed()` only matches MySQL. `PgSource::is_closed()` already exposes the driver's closed state, but is never consulted. A PostgreSQL peer disconnect, exceeded lifetime wire budget, or oversized frame causes a query error; the actor emits DataError, checks this incomplete closed predicate, and continues its command loop. The UI/session remains Open even though every later query uses the dead client. This also permits schema refresh on a permanently dead transport.

Reproduction fixture: connect PostgreSQL; make the driver reject a peer frame larger than 1 MiB (or terminate the backend during a query); after the DataError, inspect session state and submit another query. Expected: a terminal/reconnect state. Current source path: Open and repeated dead-client errors. The driver fatal-close path and adapter predicate are established from source; no live peer fixture was run in this review.

Correction: include the PostgreSQL client's closed state in the backend predicate and verify the error-to-terminal event transition.

### P2 — The expanded AST fence still permits persistent PostgreSQL session controls

Location: `crates/data/src/query.rs:22-49`; `crates/data/src/postgres.rs:108-138,229-233`.

The explicit `SET`/`RESET`/`USE`/transaction fence fixes the previously reported ordinary commands, but `Statement::LISTEN` and `Statement::UNLISTEN` are absent. They execute in write mode and persist beyond the adapter's COMMIT, contrary to the spec's managed session-control fence.

There is also a concrete SQL-query spelling of persistent SET: `SELECT pg_catalog.set_config('bytea_output', 'escape', false)` is a normal Query AST and passes in either mode. PostgreSQL permits this session setting in a read-only transaction; `false` requests session rather than transaction-local scope. After COMMIT, `SELECT decode('00ff41','hex')` returns escape-format bytea, while `pg_cell` insists on the hex `\\x` prefix and fails. This is persistent session-state mutation, not a demonstrated database-write/read-only bypass.

Correction: cover the remaining explicit session-control variants and establish how mutable built-in session functions are constrained or reset between managed operations. Add the two-query bytea fixture and LISTEN/UNLISTEN parser cases. User-defined functions may have their own side effects; this finding does not claim that blocking one built-in provides complete function sandboxing.

## Fix assessment

- MySQL direct-connection Drop now calls synchronous `abort()` under the opt-in feature. `abort()` marks disconnected and takes/drops the stream. The ordinary struct-field drop also disposes a transport if a partially canceled SDK routine already marked the connection disconnected. Pooled Drop retains its upstream branch; the Data adapter uses direct connections. No definite partial-handshake/cancellation socket-retention problem remains in this patch on source inspection.
- MySQL capped/error collection drops the transaction wrapper then aborts the transport. Captured capped rows survive, the actor emits result events then closes the session, and reuse is prevented through the MySQL closed predicate. A canceled/timed-out query retains the operation permit through the bounded cancellation attempt and connection disposal. The expected abrupt-close/uncertain-write behavior is stated honestly.
- JSON decimals and mixed numeric columns now preserve their raw numeric lexemes as text before Arrow; homogeneous top-level signed i64 columns remain integers. Nested numeric lexemes are preserved as text, with depth 32 enforced. The global arbitrary_precision feature is gone; `raw_value` is scoped to this crate dependency. No definite remaining precision regression was identified in the scoped conversion on source inspection.
- The expanded visitor rejects the originally reported SET/USE/savepoint/prepared-command escapes without forbidding ordinary INSERT/CREATE TABLE. The remaining session-state forms above need coverage.

Evidence boundary: parent reports six Data tests and two real nonreplying-TCP transport tests passing. Their source was inspected, but this reviewer did not rerun them and does not claim independent execution evidence. Real PostgreSQL/MySQL cancellation, cap, commit/rollback, TLS and partial-handshake fixtures remain pending CI. Full application/native regressions remain separate from this fix review.
