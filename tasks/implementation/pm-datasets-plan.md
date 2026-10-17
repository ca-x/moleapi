# Script dataset queries implementation plan

**Goal:** Implement Postman-shaped `pm.datasets` SQL queries and saved views in bounded pre/post scripts, then extend the same contract to live sources and Mock scripts.

**Architecture:** Keep the existing QuickJS process isolation. Add a versioned async script evaluator, then pass owner-checked immutable literal dataset snapshots into the worker and execute parameterized read-only SQL in memory. Persist named views on the existing saved dataset model. Live query-backed sources and Mock execution follow as independently testable capabilities.

**Tech Stack:** Rust/rquickjs, mature in-memory SQLite library, existing Axum/SeaORM workspace and runner paths, React/TypeScript/Vitest.

**Spec:** `docs/specs/pm-datasets.md`

## Constraints

- Old `moleapi-pm/1` scripts retain Promise rejection; v2 is explicit.
- No worker filesystem/network access. SQL is prepared, read-only, one statement, bounded by CPU/wall-clock/row/byte budgets.
- Dataset IDs, owner, workspace revision, cancellation and privacy follow existing shared contracts.
- Query-backed sources and Mock execution remain separate gates, not implicit support.

## Tasks

### 1. Versioned bounded asynchronous script phases

- [x] Add an explicit workspace-level v2 script mode with a backward-compatible v1 default and validation.
- [x] Add a failing worker regression for top-level await, settled/rejected/never-settling Promise, detached mutation and private failure output. Keep the existing v1 tests unchanged.
- [x] Implement bounded QuickJS promise evaluation/job draining under the existing process watchdog, with one final exported state only after settlement. Verify worker cancellation/reap, strict Clippy and format; commit.

### 2. Read-only dataset query substrate

- [x] Add validated stable table names to saved datasets and host snapshot admission for literal CSV/JSON sources. No silent omissions or unowned references.
- [x] Add a pinned mature SQLite adapter with bound string parameters, one read-only statement, output/CPU limits and exact value conversion. Test actual CSV/JSON queries, injection, multi-statement/write rejection and bounds.
- [ ] Wire `pm.datasets(id).executeQuery` into v2 and verify `for await...of result.rows`, pre/post phase errors and privacy in real hosted/native runs; commit.

### 3. Saved views and user flow

- [ ] Add bounded view ID/name/SQL metadata with old-workspace compatibility, CAS/owner validation and default/full backup projection.
- [ ] Implement `executeView(viewId, params)` on the same query adapter. Add bilingual table/view editor controls with scope fences.
- [ ] Verify real CLI, scheduled and self-hosted agent runs plus 390px English/Chinese save/reload/query UI; commit.

### 4. Live source and Mock completion

- [ ] Design and implement a checked on-demand host bridge for query-backed dataset IDs under the script phase deadline, without worker network access or eager refresh of unrelated sources.
- [ ] Add a Mock implementation script contract and execute the same dataset handle from a bounded owner-scoped mock request.
- [ ] Run real owner/revocation/cancellation/privacy tests, full relevant suites, three-database CI, native packages and independent review. Update matrix/coverage claims only with exact evidence.

**Checkpoint 2026-10-17:** commits `89c6d52` and `1e13d54` add the explicit v2 phase worker, settings control, stable table names and isolated SQLite adapter. The adapter passes direct tests, but workspace snapshot admission, `pm.datasets` binding, views and live/Mock bridges remain open; no Data matrix row is closed by these commits.

**Query continuation:** Literal dataset snapshots now come from the owned workspace before each v2 script phase. `pm.datasets(id).executeQuery` returns async iterable rows; local and hosted real HTTP fixtures cover pre/post scripts, bound parameters, owner isolation, missing sources and history privacy. Mixed boolean/nonboolean source columns reject before execution because SQLite cannot faithfully recover both original types from one dynamic column. Current-head native UI/CLI/schedule/agent evidence and version-change races remain open, so the public matrix row remains open.

## Review focus

- A Promise settling after a phase returns must not mutate its saved output.
- A SQL parameter that contains quotes/semicolons must remain data, never alter the query.
- Private row values copied into logs/errors/history must be screened even if the script catches an error.
- An old server/client that drops new view metadata must not report sync success.
- An agent losing its lease during a live-source dataset query must stop before sending the request.
