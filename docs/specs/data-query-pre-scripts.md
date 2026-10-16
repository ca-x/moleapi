# Data per-query preparation

Authority: `data-client.md`, `data-query-completion.md`, remaining-work milestone 3, and the shared `moleapi-pm/1` script and variable contracts.

## Objective

The Data workbench currently runs the inherited pre-request script once when it opens a connection. Subsequent SQL submissions use the editor text directly. Add a separate saved **query pre script** so each explicit Data query can prepare scoped variables, edit its SQL and report its own tests/logs before the SQL operation starts. Connection preparation retains its current behavior; an existing saved Data request with no query script behaves exactly as before.

## User and API contract

- `DataConfig.query_pre_script` is an optional string, defaulting to empty, limited to the same 256 KiB script budget as other request scripts. Existing imports and saved requests without the field continue to load. The Data editor shows it in a dedicated tab while disconnected and connected; edits are saved with the request but a running session uses the connection-time snapshot until reconnect.
- Each accepted `data_query` carries a query ID, selected SQL, and read-only flag. The server presents the selected SQL to the isolated script worker as `pm.request.body.raw` in text mode. The script can set variables through existing scope APIs and replace SQL with `pm.request.body.update(...)`. It can log and test. `pm.response` is absent. The output SQL is resolved against the updated scoped values and then checked against the existing 64 KiB, single-statement, dialect, read-only/write and execution budgets. The script cannot change connection URL/auth, headers, method, protocol/source, file selection or read-only flag. Reject an attempted unsupported mutation before dispatching SQL.
- Only one query preparation or execution is active per Data session. The next pre script waits for the previous query's response postprocessing to finish so it sees the prior script/extraction variable updates. An explicit cancel during preparation stops the preparation and emits a query-scoped cancelled/preparation error outcome without dispatching SQL. Failed/oversized/invalid pre script or SQL resolution emits `data_preprocess_error` with query ID and masked details, and must not run or retry SQL. `pm.skipRequest()` means no SQL and an explicit skipped result. No history record is created for a query that never reached the Data worker.
- Successful preparation emits a `data_prepared` event with the query ID, safe SQL preview or change indicator, tests/logs and non-private variable update metadata. The Data workbench displays preparation status before live rows and keeps it attached to the correct query; it does not silently edit the saved SQL draft. The actual resolved SQL source is passed to the protocol worker and retained by the existing completion/history path.

## Ownership, privacy and limits

- Recheck session, account, workspace/request identity and owner admission generation before running the script and before dispatch. Logout, session deletion, workspace deletion or a changed owner generation prevents SQL dispatch and later variable updates. The script worker has its existing 250 ms deadline, 64 MiB heap, disabled network/filesystem and 8 MiB output cap. The prepared SQL is revalidated by the protocol runtime; neither the script nor the API bypasses its SQL AST policy.
- Script output and pre error details are masked using the captured private values. New private values from the script are added to the session scope before later postprocessing and history screening. A script failure with incomplete privacy metadata withholds diagnostic text. The bounded Data session event budget remains authoritative.
- Busy or full Data sessions reject before script side effects. Each accepted query ID produces at most one SQL dispatch and one terminal preparation result. SQL writes are never retried or called rolled back on cancellation.

## Evidence

Use actual local file and hosted PostgreSQL/MySQL workers to prove variable substitution, SQL mutation, one pre run per query, ordering with post updates, skip/failure/cancel with zero SQL dispatches, read-only policy, privacy masking, owner/logout/delete fencing, script budget, and history SQL source. Verify the bilingual editor and preparation status at 390px. Run Rust and frontend suites, strict Clippy, formatting, production/embedded build, then branch CI and platform gates. Saved datasets, extra Data sources and EDAS remain separate capabilities.
