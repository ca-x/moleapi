# Data per-query pre scripts implementation plan

> **For agentic workers:** Execute each numbered task in order with a failing integration test before its implementation. Use `executing-plans` for inline work. Keep the one final independent branch review.

**Goal:** Run a saved isolated script before every accepted Data SQL query, with safe SQL mutation and scoped variables.

**Architecture:** Keep connection-time scripts as they are. Add a bounded query-pre script field to `DataConfig`, and a server-owned per-session preparation context shared by the send handler and Data completion consumer. Split the protocol's Data query admission from queue dispatch, so a script runs only after one query ID has reserved the active slot; cancellation can abort preparation without SQL. The output SQL is resolved and checked again by the existing Data worker.

**Tech Stack:** Rust/Tokio/Axum, `moleapi-pm/1` QuickJS worker, React/TypeScript, Vitest, local DataFusion file worker and hosted PostgreSQL/MySQL.

**Spec:** `docs/specs/data-query-pre-scripts.md`

## Global constraints

- Preserve the old connection-time pre script and empty-field request behavior.
- Keep SQL at 64 KiB, scripts at 256 KiB and the 250 ms isolated worker deadline.
- Never dispatch SQL before owner/session/admission checks or after a failed script. Never retry writes.
- Reuse Data AST/read-only checks and shared variable/privacy helpers.

## Tasks

### Task 1: Saved contract and editor

**Files:** `crates/core/src/data.rs`, `web/src/features/data/types.ts`, `web/src/features/data/DataWorkbench.tsx`, bilingual dictionaries, core and web Data tests.

- [ ] Add a core serialization/validation test: old Data JSON defaults to an empty script, and a script over 256 KiB is rejected before persistence. Run it red.
- [ ] Add `query_pre_script` to `DataConfig` and the frontend type with the empty default and cap; run the test green.
- [ ] Add a Data workbench script tab with a labelled editor that stays editable while connected but documents reconnect-to-apply. Add a saved/reloaded editor test and inspect 390px Chinese/English.
- [ ] Run `cargo test -p moleapi-core data` and the focused Vitest files; commit the contract and editor.

### Task 2: Protocol reservation

**Files:** `crates/protocols/src/data.rs`, `crates/protocols/src/lib.rs`, `crates/protocols/src/models.rs`, `crates/server/tests/data_client.rs`.

- [ ] Add a failing real-worker test where two sends race, one preparation is cancelled, and a rejected query never emits `data_started` or SQL rows.
- [ ] Expose owner-scoped `reserve_data_query` and `dispatch_reserved_data_query` around the current one-active-query, 512-command and 20 MiB limits. Cancellation and failure release a reservation exactly once.
- [ ] Keep the worker's SQL validation after dispatch and its existing terminal completion behavior. Run the new test and all Data client tests; commit.

### Task 3: Server preparation and privacy

**Files:** `crates/server/src/protocols.rs`, `crates/server/src/execution.rs`, `crates/server/src/lib.rs`, Data client and database integration tests.

- [ ] Add a failing local CSV test: per-query script changes SQL from the selected text, runs once on each query, and query N+1 reads N's post-script variable update.
- [ ] Share session scopes with an async lock between the send handler and completion consumer. Capture request/collection/workspace and owner generation at connect, then run the isolated script on a synthetic text-body request.
- [ ] Reject changes to URL/auth, method, headers, protocol/source, file selection and read-only mode; accept only SQL/body and scoped variable updates. Validate/resolve the output SQL before dispatch. Return query-scoped prepared, skipped or masked error events without history for unexecuted SQL.
- [ ] Add failing tests for script deadline/privacy failure, invalid SQL/mutation, owner logout/workspace deletion and cancellation during preparation. Make them pass, then run the Data and database suites; commit.

### Task 4: Workbench result flow

**Files:** `web/src/features/data/useDataEvents.ts`, `web/src/features/data/DataWorkbench.tsx`, `web/src/features/data/types.ts`, focused Vitest files and dictionaries.

- [ ] Add a failing hook test for stale preparation events and cancel/error releasing the run button.
- [ ] Display query preparation status/tests/logs before rows, retain them with the matching result and never rewrite the saved SQL draft.
- [ ] Run focused Vitest and browser QA at 390px in both languages against a real CSV fixture; commit.

### Task 5: Gates and review

- [ ] Run `cargo fmt -- --check`, `cargo test --locked`, `cargo clippy --locked --all-targets -- -D warnings`, `npm --prefix web test`, and `npm --prefix web run build`.
- [ ] Verify hosted PostgreSQL/MySQL SQL mutation and read-only enforcement in Application CI, then track the current platform build and perform one independent whole-branch review.
- [ ] Update `docs/FEATURE-MATRIX.md`, `docs/IMPLEMENTATION-STATUS.md` and the remaining-work plan with exact supported combinations and still-open saved dataset/source/EDAS gates.

## Progress, 2026-10-16

- Saved `query_pre_script` has an empty legacy default, 256 KiB cap, bilingual editor and safe export screening. A real 390px remote CSV query changed selected SQL from `id=1` to `id=2`; English/Chinese status, tests, history link and saved/reloaded script were inspected.
- Protocol reservation prevents concurrent admission and repeated dispatch of a query ID. The server runs the isolated script once per accepted query, uses the previous query's postprocessed variables, validates mutated SQL, and handles skip/error/cancel without running SQL. The worker enforces the saved read-only setting even if a client sends `read_only=false`.
- Real file-worker regressions cover mutation, sequential variable updates, cancellation, skip, illegal request changes, read-only writes, private logs/update keys/database diagnostics and oversized preparation-event rollback. Data client tests passed (21); the full frontend suite passed (320), strict full-workspace Clippy, formatting and production frontend build passed. Independent review found four privacy/state issues and an event-limit rollback issue; each was fixed and rechecked.
- Hosted PostgreSQL/MySQL query-pre mutation, full CI and current platform package gates remain pending. The pinned generator JAR is unavailable in this local checkout, so `cargo test --locked` awaits CI's pinned-asset preparation step. Other Data sources, saved datasets and EDAS are separate remaining capabilities.

## Review focus

- A script changes the Data source, URL/auth, headers or read-only flag: fail before SQL and mask private diagnostics.
- Two simultaneous sends or a cancel while the worker is starting: at most one SQL dispatch, no stranded active query.
- Postprocessing of query N overlaps preparation of N+1: N+1 sees N's scoped updates without leaking them into saved drafts.
- A script worker times out or loses privacy metadata: no SQL dispatch and no unmasked output.
- Logout, workspace deletion or session removal during preparation: no late dispatch, history or cross-owner event.
