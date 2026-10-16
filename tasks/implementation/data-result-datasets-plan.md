# Data result dataset snapshot plan

> **For agentic workers:** Execute in order with a failing test for each behavior, then one independent whole-branch review. Use `executing-plans` for inline implementation.

**Goal:** Let users explicitly save bounded live Data query rows as a private reusable workspace dataset.

**Architecture:** Convert the client-side typed `Result` to the existing JSON `DatasetSource`, verify it with the existing server preview, and save a new `SavedDataset` through the workspace CAS path. Keep the conversion pure and bounded; keep the UI form scoped to its account/workspace/request identity. No database migration or new remote connector.

**Tech Stack:** React/TypeScript, Vitest, existing Axum workspace and dataset API, Rust Data file worker, agent-browser for 390px QA.

**Spec:** `docs/specs/data-result-datasets.md`

## Global constraints

- Snapshot only complete, untruncated rows from the active Data query.
- At most 100 rows, 100 columns, 1 MiB source, 20 datasets and 4 MiB aggregate saved source.
- Store integer/decimal values as strings; reject binary/truncated cells and duplicate/empty columns.
- Default private; no SQL, URL or credential metadata in the saved dataset.
- Use existing owner/CAS/preview validation, then check identity after awaits.

### Task 1: Pure typed-result conversion

**Files:** `web/src/features/data/dataset.ts`, `web/src/features/data/dataset.test.ts`.

- [ ] Write failing tests for exact integer/decimal, null/bool/text, duplicate names, binary/truncated cells and row/column/source byte bounds.
- [ ] Implement `resultDatasetSource(result: Result): DatasetSource` with deterministic JSON source. Return user-facing typed errors for unsupported/oversized data.
- [ ] Run focused Vitest and TypeScript checks; commit.

### Task 2: Explicit save flow

**Files:** `web/src/features/data/DataWorkbench.tsx`, a small save dialog component, dictionaries and focused component tests.

- [ ] Add a failing component test: a completed result opens the save dialog, defaults private, previews source, then saves a new dataset with the current workspace revision.
- [ ] Implement the dialog and preview/save using existing APIs and workspace state. Gate async completion to current account/workspace/request and keep a name on errors/conflicts.
- [ ] Add tests that an unfinished/failed/truncated result cannot save, a workspace switch rejects a late preview, and the saved SQL draft/result stay unchanged. Run focused Vitest; commit.

### Task 3: Runtime and release evidence

- [ ] Use the real Data file worker and 390px browser in Chinese/English: query CSV, save dataset, reload, inspect private source and select its ID for a runner.
- [ ] Run `npm --prefix web test`, `npm --prefix web run build`, `cargo fmt -- --check`, `cargo test --locked` and strict all-target Clippy. Use existing database CI for storage/runner parity and record any generator-asset/environment gap.
- [ ] Update `docs/IMPLEMENTATION-STATUS.md`, `docs/SAVED-DATASETS-COVERAGE.md` and the remaining-work plan with exact local/CI/platform evidence. Complete one independent review before merge.
