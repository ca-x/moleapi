# Runner Label Targets Implementation Plan

**Goal:** Route manual and scheduled jobs to an enabled member of a label-defined runner group, with a concrete durable binding and no local fallback.

**Spec:** `docs/specs/runner-label-targets.md`.

**Order:**

1. Extend the shared task summary and server queue contract with optional `target_label` / `runner_label`. Resolve an owned enabled member by pending-task count and runner ID inside the account transaction. Reuse the existing direct target path and quotas. Cancel queued jobs when their runner is disabled.
2. Extend persisted schedule definitions, create/update validation and remote dispatch/reconciliation. Old definitions remain readable. A missing label member at occurrence time records a failed remote run.
3. Expose direct/label selection in hosted runner and schedule controls, CLI queue JSON, task metadata and bilingual text. Keep native offline paths local.
4. Write server two-router, schedule, CLI process and frontend tests for the spec, then run the complete feature and build verification as the last phase of this milestone. Record exact evidence and open platform gaps in coverage/status.

The user's 2026-10-13 direction puts test execution after functionality and test authoring; the implementation order above takes precedence over the usual test-first steps in engineering skills.
