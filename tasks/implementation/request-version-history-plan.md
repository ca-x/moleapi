# Request Version History Implementation Plan

**Goal:** Inspect and restore an individual request across saved workspace revisions without reverting unrelated resources.

**Architecture:** Read the existing archived workspace snapshots through a bounded row stream for a per-request change timeline. Fetch only one snapshot for detail or restore. Mutate the current workspace through the shared CAS replacement transaction.

**Tech Stack:** Axum, SeaORM, Rust core models, React, TanStack Query, Radix Themes.

**Spec:** `docs/specs/request-version-history.md`

## Global constraints

- Owner check precedes every history lookup; default list omits request payloads and credentials.
- At most 32 archived rows are scanned per list page. Only one archived row is processed at a time; its raw JSON and decoded workspace coexist briefly during parsing, while the live workspace is reduced to the selected request state before the scan.
- Request-only restore retains unrelated current workspace state and existing CAS/revocation side effects.
- Desktop local and hosted modes use the same route and current validator.

## Tasks

### 1. Paged per-request timeline

- [x] Write database API tests for changed, unchanged, deleted and cross-owner states plus page cursor behavior.
- [x] Implement owner-scoped list in `crates/server/src/request_versions.rs` with a 32-row streaming scan and 20-entry response cap.
- [x] Add one-version detail route returning only that request state and historical collection ID.
- [ ] Verify SQLite test, strict Clippy and scoped format.

### 2. Request-only restore

- [x] Add API tests for replace, deleted-request recreation, historical absence removal, missing collection/destination, invalid linked specification, Mock share revocation and concurrent CAS.
- [x] Implement restore using the shared validator, hosted scrub, `storage::replace` and session reconciliation; reuse the bounded SQLite contention handling.
- [ ] Verify SQLite then configured PostgreSQL/MySQL CI.

### 3. Request editor history

- [x] Add a request editor action and a paged history dialog with safe preview and explicit delete/restore confirmation.
- [x] Integrate existing unsaved-draft guard, server conflict flow, owner fencing, request selection and stale-response clearing.
- [x] Add focused frontend tests for account switching, conflicts, deletion and pagination; run full frontend suite/build and a 390px bilingual browser flow.

### 4. Acceptance

- [ ] Record exact coverage and open matrix scope, inspect diff, run fixed-head Application CI and Docker checks, then merge only after the stack parent is merged and required checks pass.
