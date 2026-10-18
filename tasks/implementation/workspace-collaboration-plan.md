# Hosted workspace collaboration implementation plan

**Goal:** Give invited members useful access to one canonical hosted workspace under owner/admin/editor/viewer permissions.

**Spec:** `docs/specs/workspace-collaboration.md`; capability map: `docs/COLLABORATION-CAPABILITY-MAP.md`. Authority: `collaboration-002/003/004` in `docs/FEATURE-MATRIX.md`.

## 1. Route and data contract

- [ ] Inventory all hosted routes and classify minimum role, returned private data, actor attribution and cancellation need. Unknown member routes deny by default. Keep the audited table executable in tests so new routes cannot silently inherit editor access.
  - Current checkpoint: `docs/COLLABORATION-ROUTE-POLICY.md` records all 143 route declarations now present in `lib.rs`, with 16 explicit member entry points and a drift test. The remaining owner-only/resource routes and module-composed routers still need per-method privacy, actor and cancellation review before member access is enabled.
- [ ] Specify indexed membership/invitation entities and migrations for SQLite/PostgreSQL/MySQL, including uniqueness, token digest, expiry and workspace deletion cascade. Add migration/recreation tests before changing handlers.
- [ ] Add a resolved workspace principal carrying actor ID, canonical owner ID, workspace ID, role and membership revision. Use explicit owner/workspace pair routes for members because bare workspace IDs are owner-scoped, and keep the actor identity for audit.

## 2. Invitation and authorization

- [ ] Implement single-use invitation create/list/revoke/accept, member list/change-role/remove and their hosted-session admission. Test cross-instance acceptance races, idempotent same-account retry, expiry, owner protection, limits and generic unknown-token errors.
- [ ] Enforce role checks for workspace GET/PUT, resource reads/writes, request execution, Mock, scenarios, history, backup, public sharing, runner credentials and outside-`/workspaces/` entry points. The route inventory determines exact policy rather than method-only guesses.
- [ ] Serve a privacy-safe viewer projection, block viewer execution/full export and recheck membership after asynchronous operations before returning private responses. Revoke/removal fences active sessions/jobs across instances.
  - Current checkpoint: shared workspace GET/PUT and history/report/Mock-call reads recheck membership before releasing private responses; successful invite/member/workspace writes append actor-labelled audit events atomically. Shared collection, canonical saved-request and saved Mock preview executions have actor audit, concurrent cross-instance membership polling and final response fencing; ad hoc execution and other long-running resource routes remain closed until equivalent fences are integrated.

## 3. Client and acceptance

- [ ] Show owned/shared workspaces and role labels in the picker; add bilingual invite/member management and read-only viewer states. Account/logout/workspace changes invalidate drafts, caches and pending mutations.
  - Current checkpoint: hosted settings create/revoke one-time fragment invitation links, update/remove members with revision checks, and accept links under the current session. The picker shows owner/role labels; a dedicated shared view offers viewer-safe browsing and CAS editing for saved HTTP request settings/basic Mock examples, collection run, saved-request/Mock preview, history, report and Mock-call actions. Dedicated non-HTTP and advanced Mock/resource editor parity, cache/account fencing audit and native hosted sync remain open.
- [ ] Test real owner/admin/editor/viewer journeys at 390px in English and Chinese, including save conflict, execution denial, membership revocation and safe viewer responses. Verify offline workspaces remain local and shared hosted sync never uploads a stale member draft.
- [ ] Run focused/full Rust and frontend suites, strict Clippy, production frontend/embedded server builds, three-database fixed-head CI, Docker and native installed-package checks. Obtain independent review, fix Important findings and merge only after the integrated gate.
