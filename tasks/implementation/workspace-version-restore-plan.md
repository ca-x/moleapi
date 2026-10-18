# Workspace Version Restore Implementation Plan

**Goal:** Let an owner inspect archived workspace revisions and restore one as a new revision.

**Architecture:** Reuse existing owner-scoped `version` documents and `storage::replace` compare-and-swap. The backend selects a snapshot by owner, workspace and revision; the UI requires explicit confirmation and installs only the returned current workspace.

**Tech Stack:** Axum, SeaORM, React, TypeScript, existing i18n and TanStack Query.

**Spec:** `docs/specs/workspace-version-restore.md`

## Global constraints

- No new tenancy or schema model. Keep personal-owner and offline behavior.
- Validate restored data, scrub hosted local values, and use the existing replacement side effects.
- A stale current revision must conflict without creating history.
- Protect unsaved browser drafts and cross-account state.

## Tasks

### 1. Owner-scoped restore API

- [x] Add real API tests in `crates/server/tests/database.rs` for restore success/new history, stale CAS, missing version and cross-owner denial.
- [x] Implement `POST /workspaces/{id}/versions/{revision}/restore` in `crates/server/src/workspaces.rs` and register it in `lib.rs`.
- [x] Query one archived version by owner/kind/ref/revision, validate and scrub its data, set the live id, then use `storage::replace` and `reconcile_sessions`.
- [ ] Run the database test with SQLite and configured PostgreSQL/MySQL, strict Clippy and formatting.

### 2. Historical preview and confirmed restore

- [x] Add a focused React test for unsaved-draft protection, successful new revision, conflict refresh, and account switch while preview is open.
- [x] Add a workspace history dialog under `web/src/features/workspaces/` with accessible revision list, safe text preview and explicit confirmation.
- [x] Connect it through the existing workspace controller and invalidate the history query after restore; add Chinese/English strings.
- [x] Run frontend tests/build and a real 390px bilingual browser save→restore→reload flow.

### 3. Integration evidence

- [ ] Run current-head Application CI across three databases and Docker smoke, then inspect changes against the spec.
- [ ] Record exact evidence and remaining versioning scope; merge the feature batch only after required checks.
