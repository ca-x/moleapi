# Scheduled runner targets implementation report

Worktree: `/home/czyt/code/rust/moleapi/.worktrees/application`, branch `wip/oauth2`, implementation baseline `60db3ce`. Changes are not committed yet. Existing untracked draft artifacts were preserved. No local Docker commands were run.

## Implemented

- Schedule definitions accept an optional `runner_id`; omitted values keep the current server/local execution path.
- Runner-targeted scheduled runs atomically enqueue work into the existing self-hosted runner task queue and persist the active `runner_task_id` in schedule running state.
- Runner-targeted schedules do not consume server execution slots and do not fall back to local/server execution when the runner lease expires or is unavailable.
- Restart reconciliation, lease expiry, cancel/edit/delete, and runner task completion now reconcile the schedule state, history, and linked execution report.
- Runner queue helpers are reusable from scheduling code through `crates/server/src/runners/schedules.rs` and the exported task queue entry point.
- The Testing page includes a hosted runner management panel for registration, rename, disable confirmation, refresh, ad-hoc saved-source dispatch, task cancellation, and report navigation.
- The Schedules panel can target either the current server or an owned enabled runner, with bilingual English and Chinese catalog entries.
- Runner documentation, coverage notes, implementation status, and the feature spec/plan were updated for this slice.

## Public behavior

- Creating or editing a schedule with no runner selected preserves the existing current-server behavior.
- Selecting an owned enabled runner queues future scheduled executions as runner tasks.
- A disabled/missing/non-owned runner is rejected instead of silently falling back.
- Cancelling, editing, or deleting a running runner-targeted schedule cancels its queued runner task.
- A completed runner task updates the schedule's last run, appends history, and links the produced report.

## Actual verification

- `cargo test -p moleapi-server --no-default-features --test scheduled_runners --test scheduled_runs --test runners`: passed, 14 tests total.
- `npm --prefix web test -- src/features/runners/RunnersPanel.test.tsx src/features/monitoring/SchedulesPanel.test.tsx src/features/testing/TestingPage.flow.test.tsx src/features/testing/TestingPage.scenarios.test.tsx src/shared/i18n/catalogs.test.ts`: passed, 13 tests across 5 files.
- `npm --prefix web run typecheck`: passed.
- `npm --prefix web run build`: passed with existing large chunk warnings only.
- `cargo build -p moleapi-server`: passed after rebuilding the embedded web bundle.
- `cargo build -p moleapi-server -p moleapi-cli`: passed.
- `cargo clippy -p moleapi-server --no-default-features --all-targets -- -D warnings`: passed; unrelated vendored dependency warnings did not fail the crate.
- `git diff --check`: passed after focused formatting.

## Browser QA

- Server ran on `127.0.0.1:19077`; Vite ran on `127.0.0.1:19078`.
- Account `runnerqa`, workspace `Runner QA`, and runner `LAN QA` were created for the manual fixture.
- Ordinary runner dispatch completed with report `64028c59-6c5d-43e6-ad40-c7f9dabc4b50`.
- Scheduled runner-target dispatch completed with report `b7c193fe-e155-481b-8441-ce1446b4470c`.
- Schedule `last_run` showed `passed`, `passed: 1`, and the linked report opened from the UI.
- Mobile viewport `390x844` had document width `390`, no horizontal overflow, and axe returned `{}`.
- Screenshots: `/tmp/moleapi-runner-resume-desktop.png` and `/tmp/moleapi-runner-resume-mobile.png`.

## Known remaining gaps

- Runner labels/group scheduling, workspace-scoped runner credentials, service installation flows, response-history upload, native hosted management, and broader database/platform verification are still future runner work.
- Full SDK/server generation, additional target languages, Dubbo, enterprise collaboration/versioning/governance, AI, and the wider 395-feature matrix remain outside this slice.
- CI, Docker, package validation, final formal review, merge to `main`, branch cleanup, and release validation remain deferred until functional coverage is complete.
