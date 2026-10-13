# Runner Credentials Implementation Plan

> **For agentic workers:** Implement each task against `docs/specs/runner-credentials.md`, with failing fixtures before code. Keep all changes in the existing isolated application worktree; no merge, push or release.

**Goal:** Replace account-wide agent bearer use with revocable single-runner/single-workspace credentials.

**Architecture:** Reuse the owner-scoped document store and account-row serialization; authenticate by an indexed digest key, then reauthorize inside runner transactions. Management remains login-session-only. Preserve the existing queue/lease engine and CLI transport.

**Tech Stack:** Rust/Axum/SeaORM/SQLite fixtures, existing React/Radix settings patterns, CLI/Clap.

**Spec:** `docs/specs/runner-credentials.md`.

## Global constraints and review focus

- No raw bearer/digest in storage payload, metadata, logs or error text; return raw bearer once on create.
- Lock order: account before workspace and task; no transaction held during remote execution.
- Verify exact method/path authorization and handler-side scope; path-prefix matching alone is insufficient.
- Recheck revocation and expiry under the account lock; independent instances must observe the same durable state.
- Review skipped foreign-workspace queue entries, old persisted tasks with no credential ID, same-ID workspace recreation, and duplicate/late completion after revocation.

## Task 1 — Issuance and lookup

**Files:** Add `crates/server/src/runner_credentials.rs`; update `crates/server/src/lib.rs`, `crates/server/src/auth.rs`; add `crates/server/tests/runner_credentials.rs`.

- [ ] Write failing hosted tests for session-only create/list/revoke, owner/runner/workspace gates, one-time bearer, indexed digest-only row, 100-entry quota, expiry, malformed token and workspace deletion cascade.
- [ ] Implement metadata/document storage and management routes with existing account lock and `SessionAuthentication`. Add indexed bearer lookup to auth guard; reject scoped bearer on every non-agent protected route.
- [ ] Run focused tests and rustfmt; inspect DB rows and JSON output for secret leakage.

## Task 2 — Atomic agent authorization

**Files:** Update `crates/server/src/runners/agent.rs`, `crates/server/src/runners/storage.rs`, `crates/server/src/runner_credentials.rs`; extend `crates/server/tests/runner_credentials.rs` and existing runner/schedule fixtures.

- [ ] Write failing two-router tests for foreign workspace/runner, claim-vs-revoke, active lease cancellation, heartbeat/completion denial, unrelated credentials, expiry and schedule reconciliation.
- [ ] Require `AgentScope` in agent handlers, validate it under account lock and bind leased task to credential UUID with serde default for old task rows. Scope queued-task scan and owner/runner/workspace checks.
- [ ] Migrate existing direct agent-route tests from owner bearers to scoped credentials, without weakening ownership assertions. Run runner/schedule suites.

## Task 3 — CLI and hosted UI

**Files:** Update `crates/cli/src/args.rs`, `crates/cli/src/runners.rs`, `crates/cli/tests/runners.rs`; extend `web/src/features/runners/RunnersPanel.tsx` and tests, shared types and bilingual catalogs; update `docs/SELF-HOSTED-RUNNERS.md`.

- [ ] Write a failing CLI process fixture using a scoped bearer, plus no-broad-bearer and no-owner-API assertions. Add owner-session credential management commands; use existing token file/env for agent.
- [ ] Add account-fenced hosted issue/list/confirm-revoke UI with one-time copy and native exclusion. Write frontend tests for late responses and secret clearing.
- [ ] Run CLI, frontend and embedded server build/targeted browser QA; document private token deployment and rotation.

## Task 4 — Integration gate

**Files:** Update `docs/SELF-HOSTED-RUNNER-COVERAGE.md`, `docs/IMPLEMENTATION-STATUS.md`, `tasks/implementation/remaining-work-plan.md`.

- [ ] Run focused Rust/CLI/frontend suites, strict relevant lint and formatting, production frontend/server builds, and inspect output/status. Record any unrelated baseline lint failures precisely.
- [ ] Review owner/agent route matrices and DB races against the spec. Mark only verified target/platform combinations complete; leave DB/platform/release gates open.
