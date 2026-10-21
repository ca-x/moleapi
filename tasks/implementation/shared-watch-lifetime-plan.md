# Shared operation watcher lifetime diagnosis and fix

> Use systematic-debugging, executing-plans and verification-before-completion.

**Goal:** Remove the confirmed single-connection pool starvation mechanism in member operation monitors while preserving role/revision fences and cancellation.
**Authority:** Full goal remains the feature matrix; this resolves the known intermittent Mock500 gate.
**Evidence:** Prior captured DB diagnostic is ConnectionAcquire(Timeout), with background reads timing out. Shared wrappers borrow pinned operation/watch futures in select and then await final DB checks while the losing watcher is still alive.

## Task 1

- [x] Reproduce resource lifetime deterministically with one real SQLite pooled connection and a parked watcher, using the same select/final-check boundary.
- [x] Introduce a private operation/watch/final-check boundary and release both raced futures before any final DB read; apply to all seven shared wrappers and the scheduled-run watchdog with this same pattern.
- [x] Cover operation success/error, watcher win/cancellation, priority and final access/revision denial; retain creator Stop side effect.

## Task 2

- [x] Run targeted/full affected server regressions and bounded repeat of original cross-instance test, strict Clippy/format/build.
- [x] Independent review, scoped documentation, commit/push/PR and CI.

## Review focus

All watched wrappers must call the same ownership boundary. Revocation has priority when both ready. A denied watcher still cancels the exact shared run job. Final checks run after both futures drop, including when operation returned error. Sessions/background loops do not inherit unrelated changes. No pool enlargement, artificial retries or dropped audit logs.

Task 1: Ruling: read-only scan found the same borrowed watchdog retained across scheduling::finish database writes — include its local execution scope in this fix, preserving shutdown/2-second cancellation grace semantics — cost if wrong: scheduling result/cancel timing could regress; run its existing wire/queue/cancel/restart/remote-runner coverage.

## Evidence ledger

Task1: controlled one-connection SQLite regression RED ConnectionAcquire(Timeout) after operation completion; GREEN after moving both pinned futures into inner scope. Seven shared wrappers use the common boundary; scheduling adds equivalent lexical ownership scope before finish. Success/error, watcher-win/drop, simultaneous readiness and final denial covered.
Task2: full server lib22 and affected member/scheduler/notification integration41 pass; original cross-instance Mock regression10/10 passes; strict Clippy/package formatting/diff/embedded build pass. First combined command accidentally filtered integration names and ran0 there; excluded it from counts and reran independent unfiltered binaries.
Final review: shared_report_export_review independently inspected seven shared wrappers, then the newly included scheduling scope. No concrete finding; watcher priority/cancel side effects/final checks and scheduler shutdown/grace semantics preserved.
Final: Ruling: real online PostgreSQL/MySQL timing was not reproduced locally — use deterministic real SQLite pool ownership plus generic Rust resource scopes and existing wire/cancel tests; leave current-head multi-database CI as a separate gate — cost if wrong: a database-specific failure outside this invariant remains to diagnose.
Durable learning saved via nowledge-mem as learning ef96948f-1834-442c-8b16-4acf1844b0df; records this project-specific failure pattern, proof and boundaries.
