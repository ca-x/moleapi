# Independent Data backend review — 2026-10-05

Reviewed uncommitted backend against `docs/specs/data-client.md` at HEAD `bb5b2eea1ec674efe530cc9d730e13171c1bfbf7`. Report only; no implementation changes, commits, or delegated reviews. Full product goal remains active. This is not parity or release approval.

## Important findings

### P1 — MySQL drop starts unbounded detached cleanup instead of closing the connection

Locations: `crates/data/src/mysql.rs:31-34,52-74`; `crates/protocols/src/data.rs:288-313`.

`MysqlSource` owns an ordinary `mysql_async::Conn`, sets no driver read deadline, and relies on dropping that value after timeout/cancellation to close it. The resolved SDK explicitly does something else: `mysql_async-0.37.1/src/conn/pool/mod.rs:363-383` calls `conn::disconnect(self.take())` for a direct connection. `src/conn/mod.rs:63-83` spawns a detached task which first awaits `cleanup_for_pool()`. That cleanup (`:1363-1382`) drains pending rows and rolls back before disconnecting. Neither this task nor its socket remains subject to the protocol operation timeout, session cancellation token, or execution permit. The adapter releases the permit before its one-second cancellation attempt and eventual drop.

Reproduction: begin a streaming MySQL query; have the endpoint keep the result open, and make the second cancellation connection fail or stall. Cancel/timeout the session. Its visible operation finishes, but SDK cleanup keeps reading the original connection indefinitely. Repeat to create sockets/tasks beyond the four execution slots. A stalled cancellation connection can itself enter this detached cleanup on drop. A finite query also continues draining after the advertised timeout when cancellation fails.

Required correction: provide a bounded abort/close path that actually drops the transport without pending-result cleanup, or use a verified bounded driver cleanup/read timeout and retain resource admission until it finishes. Verify Stop, owner logout/delete, timeout, failed KILL, and stalled peer behavior with socket/task lifetime assertions.

### P1 — JSON imports silently round decimals and mixed numeric columns

Location: `crates/data/src/files.rs:214-244`.

JSON numeric values are parsed into `serde_json::Value`, inferred with Arrow's default JSON schema inference, and rewritten into JSON lines. Default Arrow inference maps noninteger numbers to `Float64` and promotes mixed `Int64`/`Float64` columns to `Float64` (`arrow-json-59.3.0/src/reader/schema.rs:86-99,384`). This destroys original precision before SQL execution and before `Cell::Decimal` converts the already-rounded float to a string. Merely enabling arbitrary precision in serde_json would not fix the Arrow conversion.

Reproduction: import `[{"amount":1.23000000000000000001}]` and run `SELECT amount FROM data`: the numeric literal becomes approximately `1.23`, unlike the deliberately exact CSV path. Import `[{"n":9007199254740993},{"n":1.5}]`: schema promotion makes the first integer inexact. A later DECIMAL cast cannot restore the lost digits. This violates the spec's exact precision requirement.

Required correction: preserve JSON numeric lexemes in an exact representation and choose deliberate exact decimal/integer or text columns before Arrow decoding. Add decimal and mixed-number fixtures in addition to the existing large-integer-only fixture.

### P2 — Session and transaction controls remain executable in write mode

Location: `crates/data/src/query.rs:22-32,48-57`.

The visitor rejects only `StartTransaction`, `Commit`, and `Rollback`. With `read_only=false`, `Statement::Set`, `Use`, savepoints, and other session controls pass unchanged to the driver, despite the managed-transaction/session-control fence in the authority spec. For example PostgreSQL `SET bytea_output = 'escape'` persists after the adapter's COMMIT; subsequent bytea queries fail because `pg_cell` only accepts hex `\\x` output (`crates/data/src/postgres.rs:229-233`). MySQL `USE other_database` changes the database after endpoint checking, while ConnectionInfo still describes the configured database; subsequent metadata uses `DATABASE()` and silently follows the changed target.

Required correction: reject session/transaction-control AST variants consistently in both modes, while retaining ordinary authorized DML/DDL. Include SET, USE, savepoint, prepared/executed session statements and dialect-specific forms in fixture coverage. The findings above concern write mode; this review did not demonstrate an ordinary PostgreSQL/MySQL read-only transaction bypass.

### P2 — MySQL cancellation during result capping can discard the bounded result

Location: `crates/data/src/mysql.rs:85-96,151-164`.

After collecting a capped result, the adapter drops the pending query stream, sends KILL QUERY, then uses `transaction.rollback().await?` before returning the captured rows. SDK rollback sends a command through `clean_dirty()`, which first drains the old pending result (`mysql_async/src/queryable/mod.rs:105-111`, `conn/mod.rs:968-978`). If KILL interrupted an actively producing query, this drain returns the server's interruption error; rollback propagates it before sending ROLLBACK, and the adapter returns `DataError` instead of the already-produced rows and truncation reason. If KILL has no effect because server execution already finished, rollback instead drains all buffered rows and can hit the parent timeout, again losing the bounded result.

Reproduction fixture: a query producing more than 1,000 rows slowly enough that it is still active at the cap, then inspect the expected partial rows/truncated event versus the interruption error. This control-flow failure is established from the adapter and SDK source; the exact timing fixture was not run locally because no native database fixture is configured.

Required correction: explicitly distinguish expected cancellation/drain errors from actual rollback failure, retain the capped result, and close or safely repair the connection under a bounded deadline. Do not claim rollback succeeded if cleanup failed. Verify both active-server and already-buffered caps.

## Covered surfaces and evidence limits

Read Data adapters/query validation/schema/file workers, endpoint checking/TLS, protocol command control/permits/events/retention, server dispatch/integration, core Data validation/interpolation, formats privacy, and the vendored PostgreSQL decoder patch. Inspected resolved mysql_async, SQLparser, and Arrow sources where their behavior determined a finding. No new tests or live database fixtures were run by this reviewer; earlier parent test results are not independent evidence from this review. Existing CI must supply real PostgreSQL/MySQL verification; local Docker was not used.

Direct skeptic checks removed several hypotheses: SQLparser 0.63 MySqlDialect expands ordinary MySQL `/*! ... */` executable comments, so that is not a proven AST bypass; MySQL SDK errors when TLS is requested but the server lacks SSL support; remote file URL userinfo is rejected; PostgreSQL sockets/cancellation retain checked IPs and original TLS names. The PostgreSQL lifetime wire budget and SDK frame limit are real bounded controls. Global allocator limits are enabled only by the worker sentinel; file workers clear environment, use a parent deadline, and request kill on drop. These observations do not prove hostile endpoint coverage or native worker cleanup across all platforms.

Default export explicitly clears local file bytes and applies known-private-value screening to SQL, including quote/backslash escaped forms. Explicit private export uses the original workspace. No definite new export leak was established in this read-through; arbitrary obfuscations of copied secrets are outside the demonstrated screening coverage.

Remaining verification: remote database cancellation/cap/transaction/session fixtures; JSON exact decimal fixtures; worker timeout/cancel/heap behavior on real application executables; complete application regressions and native platform evidence. Frontend review belongs to the separate assigned surface.
