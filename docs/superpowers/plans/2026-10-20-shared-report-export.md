# Shared Report Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a hosted workspace editor or admin download an already saved, privacy-projected test report from the shared workbench.

**Architecture:** Add an explicit owner/workspace export route that delegates to the existing bounded report exporter through the collaboration `editor_read` admission and final membership check. The shared view offers the existing four formats and discards an export if its account, credential, owner, workspace or role changes before saving.

**Tech Stack:** Axum, SeaORM, Rust integration tests, React, TanStack Query, Vitest.

**Spec:** `docs/specs/workspace-collaboration.md` and `docs/COLLABORATION-ROUTE-POLICY.md`.

## Global Constraints

- Keep owner-only report deletion, full workspace export and public sharing unchanged.
- Return only the report already stored by the privacy projection; use the existing `json`, `csv`, `junit`, `html` format and `en`/`zh-CN` language limits.
- Require editor or admin membership before loading the report and recheck the same membership generation before releasing exported content.
- Never persist an export response in shared query caches.

## Review Focus

- A viewer or former member requests the export URL: return 404 without content.
- A member supplies another owner or workspace ID: return 404 without content.
- The report is missing or belongs to a different workspace: return 404.
- An invalid format or language is supplied: retain the existing bounded 400 response.
- A delayed export resolves after the shared view closes or changes account: do not save a file.

---

### Task 1: Scoped report export route

**Files:** `crates/server/src/lib.rs`, `crates/server/src/collaboration.rs`, `crates/server/src/run_reports/mod.rs`, `crates/server/tests/collaboration.rs`, `docs/COLLABORATION-ROUTE-POLICY.md`, `docs/specs/workspace-collaboration.md`.

**Interface:** `GET /api/owners/{owner}/workspaces/{workspace}/reports/{report}/export?format=html&language=en` returns `Json<ExportResult>` to an active editor/admin, using `editor_read` around `run_reports::export`.

- [x] Add integration assertions for editor success, viewer/foreign/revoked/missing denial, and invalid format/language.
- [x] Run `cargo test --locked -p moleapi-server --test collaboration`; confirm the new assertions fail before the route exists.
- [x] Add the explicit route, expose `run_reports::Export` within the crate, and update policy/spec rows.
- [x] Rerun the collaboration suite and strict server Clippy; require zero failures.

### Task 2: Shared report download

**Files:** `web/src/features/workspaces/SharedWorkspaceView.tsx`, `web/src/features/workspaces/SharedWorkspaceView.test.tsx`, `docs/COLLABORATION-COVERAGE.md`.

**Interface:** The shared reports tab offers format choice and a `下载脱敏报告` action for each summary; `saveProjectFile` receives the bounded export only while the original shared scope remains active.

- [x] Test a successful editor download plus a delayed result after closing/changing scope; confirm both fail before the UI change.
- [x] Implement the download with pending state, error display and account/credential/role fences; leave viewers without report controls.
- [x] Run focused and full frontend tests, typecheck and production build; update coverage with the exact result.
- [ ] Check `git diff --check`, commit and push the branch to PR #41; inspect fixed-head CI before merge.

## Execution record

- The editor export assertion failed with 404 before the route existed; the audit-state assertion then failed before its `started` event was implemented. The UI tests failed because the format choice and download action were absent. All passed after implementation.
- Independent review found an await between final membership check and response return. Ruling: record an admitted export as `started` before `editor_read`, so its final membership check remains the last awaited database operation. Re-review found no remaining concrete issue.
- Local checks: 13 collaboration integration tests, 42 focused shared-view tests, full frontend suite, TypeScript, production build with 45 verified chunks, strict server Clippy, Rust format and diff checks passed. Fixed-head CI and native package acceptance remain open.
