# Query-backed reusable Data datasets

Authority: remaining-work milestone 3, `data-client.md`, `data-result-datasets.md`, `SAVED-DATASETS-COVERAGE.md`, and the shared runner/dataset contracts. A saved dataset may explicitly refer to a saved Data request. Every run resolves that request and executes its read-only SQL once before the first iteration, then uses the resulting rows as an immutable run snapshot.

## User flow and persistence

- The testing page offers a **Data query** source beside CSV/JSON. The user selects one saved Data request in the same workspace, names the dataset, and saves it through the existing workspace revision/CAS path. The selected request must use the Data protocol, have a nonempty SQL statement, and be configured read-only. No connection URL, SQL, credential or result row is copied into dataset metadata.
- Add an optional `query` reference to `SavedDataset` containing the request ID. Existing CSV/JSON datasets and old workspaces continue to deserialize. A dataset has either a literal `source` or a `query` reference; a metadata-only dataset with neither is allowed for redacted backup compatibility. Both at once reject. Query-backed datasets are always private.
- The source request is resolved by ID in the same saved workspace revision. Missing, changed-to-non-Data, writable, or invalid requests fail before any collection request executes. No silent fallback to a previous result, a snapshot, another workspace, or the current Data editor draft.
- Default backups retain dataset metadata but withhold the query link. Explicit include-secrets MoleAPI backup retains the link and referenced request. Other export formats keep the existing rejection for saved datasets. Import and sync validate the reference after the complete workspace is assembled.

## Refresh and run behavior

- Refresh occurs once for each collection/scenario/CI/scheduled/runner-agent run, before iteration count and request execution are fixed. The result is reused for all iterations in that run. A later run refreshes again. The referenced SQL is not inserted as a collection step and is not recorded as a separate user response history item.
- Use the existing Data endpoint preparation, checked and pinned network policy, file worker, source/SQL limits, selected environment, private variables, read-only enforcement and query cancellation. A Data query pre-script can prepare the selected SQL through the existing bounded script path; its variable changes are not committed to the collection run unless explicitly specified by a later contract.
- Require a complete successful result with 1–100 rows, 1–100 distinct nonempty columns, no binary or truncated cells and a JSON representation within 1 MiB. Preserve exact integer/decimal text as strings. Run the shared `DatasetSource::parse` validator on the converted JSON. No partial rows or stale data are used after an error or cancellation.
- The refresh inherits the run's owner, workspace revision, environment, deadline, cancellation and credential scope. Source access must stop on logout, workspace removal, job cancellation, timeout or revoked runner credentials. For self-hosted query datasets, the agent checks its hosted lease every second with a two-second request ceiling; loss of credentials or a source-workspace revision change cancels the local run. Any write-capable Data source/request is rejected; refresh never performs DML/DDL.
- If the refreshed row count multiplied by the selected flow steps exceeds the 1,000-execution budget, record a failed zero-step run. Do not reject a query-backed task merely because its as-yet-unknown result could exceed the budget.
- The run report identifies the saved dataset ID and indicates refresh success/failure without persisting private row values, SQL or credentials. Existing history/notification redaction continues to apply. A failed refresh yields a failed run with zero collection requests.

## Entry points and evidence

The same `/run` resolver is used by interactive hosted/native runs, CLI, CI presets, schedules and self-hosted runner tasks. A self-hosted agent receives its already authorized workspace snapshot and calls the local `/run` route; the hosted claim handler does not execute the refresh. Each attempt refreshes on the execution host, and private result rows stay in that run's memory rather than being persisted in the durable task payload.

Tests cover old JSON compatibility, source/query exclusivity, missing/foreign/non-Data/writable request IDs, default/full export, real file and PostgreSQL/MySQL refresh with exact typed rows, new rows on the next run, zero HTTP requests on refresh failure, owner/revocation/cancellation, report privacy, all entry points, and a 390px bilingual save/reload/run UI. Run the focused Rust/frontend suites, strict Clippy and builds, three-database CI and current platform checks. EDAS is unrelated and remains unverified without a sandbox.
