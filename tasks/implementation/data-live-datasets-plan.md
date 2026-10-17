# Query-backed Data datasets implementation plan

**Goal:** Refresh a saved Data request into a bounded private dataset once per run, across all existing runner entry points.

**Architecture:** Add a reference variant to the existing saved dataset model. A shared asynchronous resolver verifies the owned saved workspace and Data request, runs read-only SQL through the existing checked Data engine, converts typed rows to the existing JSON dataset parser, and gives each run one immutable set of rows. The existing execution, reports and privacy gates remain authoritative.

**Tech Stack:** Rust core/data/server/protocols, React/TypeScript, Vitest, existing PostgreSQL/MySQL/file fixtures.

**Spec:** `docs/specs/data-live-datasets.md`

## Constraints

- A saved dataset has a literal source, a query reference, or neither after redacted import; never both.
- Query references are same-workspace saved read-only Data requests; query-backed datasets are private.
- Refresh once per run before requests, with no stale fallback and no partial result.
- 1–100 rows, 1–100 unique nonempty columns, 1 MiB JSON, exact integer/decimal text.
- Existing owner, revision, deadline, cancellation, network and privacy controls apply to every entry point.

## Tasks

### 1. Persisted model and privacy

- [x] Extend `SavedDataset` with a bounded query request ID and validate exclusivity, privacy and same-workspace read-only Data request references in `crates/core/src/datasets.rs` and `validation.rs`.
- [x] Add core compatibility/invalid-reference tests and default/full backup tests in `crates/formats/tests/saved_datasets.rs`. Default export withholds the link; full export restores it.
- [x] Verify core/formats tests and formatting; commit.

### 2. Typed Data result conversion and resolver

- [x] Extract one shared Rust conversion of `QueryResult` into validated `DatasetSource`, rejecting truncation, binary, duplicate/empty columns, over-limit rows/bytes and preserving exact numeric strings.
- [ ] Add an async resolver for a saved Data request using existing checked endpoint, `Connection`, read-only query preparation, environment and cancellation. Local-file success/failure/cancellation fixtures pass; PostgreSQL/MySQL fixtures are added but await configured CI endpoints.
- [ ] Verify focused data/server tests, strict Clippy and format; commit.

### 3. Wire every run entry point

- [x] Interactive/server runs resolve once before selecting iterations or dispatching a collection step; report refresh failures with zero collection executions. CLI process evidence remains in Task 3's verification step.
- [x] Scheduled and CI preset validation accept query-backed datasets without trying to parse a nonexistent literal source. Resolve under the run's owner/revision/deadline.
- [x] Self-hosted runner task leases keep the existing authorized workspace snapshot; the agent's local `/run` resolves one bounded dataset per attempt without durable row persistence or a hosted duplicate refresh. Real CLI process fixtures cover success, credential revocation and source-workspace revision changes during a pending refresh.
- [x] Verify actual process/API fixtures for hosted/native runner, CLI agent, schedule and CI preset paths, including local cancellation and persisted-row privacy; commit. Current PostgreSQL/MySQL CI evidence remains a separate gate in Task 4.

### 4. Editor, end-to-end evidence and merge

- [x] Add a bilingual Data query source picker to the existing saved dataset editor. Recheck scope after async saves and show refresh failure without stale-row fallback.
- [ ] Verify 390px English/Chinese save, reload, run and changed-result refresh with a real Data fixture. Save/reload/two-iteration run, no overflow and zero automated WCAG A/AA violations passed; changed-result browser flow remains.
- [ ] Run focused/full relevant suites, strict Clippy, frontend/embedded builds and three-database CI. Update coverage/status and seek independent review before merge.

## Review focus

- Missing referenced request after a workspace edit must stop execution.
- A query returning 101 rows must fail instead of using the first 100.
- Revocation during a slow refresh must cancel the Data connection and run.
- A default backup must not preserve a runnable private query link.
- Agent retries must refresh once per attempt and never reuse a stale lease snapshot.

## Progress, 2026-10-17

The persisted model, same-workspace Data reference validation and default/full backup projection are implemented. Core dataset tests (4), shared format tests (74 total) and TypeScript typecheck pass. The pure typed Data result converter is implemented and its exact large-integer/decimal, boolean, duplicate-name, truncated/binary and row-shape fixture passes. This branch does not yet execute a query-backed dataset during a run; refresh integration and entry-point validation remain open.

Runtime continuation: the shared `/run` route now refreshes read-only Data queries under the existing job cancellation/deadline and validates the result before any collection request. A real saved local CSV query ran two iterations, refreshed changed bytes on the next run and yielded a durable zero-step failed report on invalid SQL or a dynamic 100-row × 11-step execution limit. A delayed remote CSV fetch cancelled without sending collection HTTP; private row values were absent from the persisted report. A real CLI agent completed a query-backed task. Separate process fixtures proved that hosted credential revocation and workspace edits close an in-flight agent Data source before any collection request, and hosted heartbeat rejects the old revision. Existing saved-dataset, runner, scheduling, CI-preset and database SQLite suites pass; the configured PostgreSQL/MySQL fixture has not run locally. Frontend full suite passed 336 tests and production build; 390px English/Chinese hosted browser saved, reloaded and ran a two-row dataset without horizontal overflow. Final Chinese editor WCAG A/AA automation reported zero violations and one incomplete contrast check where elements were obscured. Current branch CI, native packages and review rerun remain.

Final local recheck after the independent review fixes: 18 focused server integration tests, all 5 CLI runner process tests, 336 frontend tests, strict all-target Clippy for CLI/server, scoped format check and a production frontend plus embedded server build passed on the current worktree. The downloaded original platform package and remote database gates remain separate from these local results. Reviewer recheck and branch CI remain open.
