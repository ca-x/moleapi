# Shared dataset-driven runs implementation plan

> **For agentic workers:** Use executing-plans to implement task by task.

**Goal:** Editors/admins run shared collections and saved scenarios with saved or temporary datasets and 1–100 iterations.
**Architecture:** Resolve workspace-scoped local options against canonical saved datasets. Reuse RunnerDataset/member preview and existing shared runner.
**Spec:** docs/specs/workspace-collaboration.md.
**Tech Stack:** React/TypeScript, Vitest, Rust.

## Constraints

Default no data preserves payload. Saved sources send only dataset_id; query sources refresh server-side. Missing selection/source blocks execution. Temporary data never enters CAS and resets across account/credential/owner/workspace/role boundaries. Lock controls during actual jobs, retain global Stop. Member preview carries expected_revision and rejects stale context.

## Review Focus

Cache notification races; scope isolation; query and missing sources; pending preview/file and active job lifecycle; collection/scenario payload parity.

## Task 1

- [x] Write resolver/view tests and observe RED.
- [x] Implement resolver/UI, scope state, member preview and dispatch validation.
- [x] Run affected tests and typecheck; translate both languages.

## Task 2

- [x] Cover member canonical/temporary/query runs and rejected inputs on backend.
- [x] Run frontend suite, server tests, production build and server gates.
- [x] Bilingual narrow browser acceptance and independent review.
- [x] Update coverage, commit/push and record CI.

## Evidence ledger

Preflight: existing shared runner accepts RunOptions; member preview and canonical revision cache fencing already exist. No new route or engine required.
Task 1: resolver RED missing module; shared view RED missing run dataset control against original source; GREEN 143/143 targeted tests, typecheck passed.
Task 2: backend fixture first rejected incomplete ScenarioStep (missing required ID); corrected fixture to canonical schema. Shared backend 1, collaboration 13, datasets 3, saved datasets 3 passed. Full frontend 605/605, 45 chunks, strict server Clippy, package format and embedded build passed. Bilingual runtime recorded exact eight rows, no CAS mutation/overflow/browser errors.
Final review: independent source review by shared_report_export_review, no concrete findings.
Final: Ruling: reviewer did not exercise native file selection or browser cross-tab races — existing RunnerDataset lifetime tests and shared view scope/Stop/cache race tests plus bilingual runtime cover affected paths; native dialog remains part of current-head installed desktop acceptance — cost if wrong: native-only file import regression escapes this browser pass.
Task 1: Ruling: task-done helper cannot execute its installed non-executable sdd-workspace dependency — preserve the actual command outputs in /tmp/moleapi-shared-run-{unit,all-web,server,build,clippy,server-build}.log and this tracked evidence ledger; no test success inferred from helper — cost if wrong: helper-generated ledger metadata is absent.
