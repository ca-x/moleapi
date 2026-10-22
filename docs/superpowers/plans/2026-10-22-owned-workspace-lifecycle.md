# Owned workspace lifecycle completion

Continue development after the native shared-workspace plan. This slice closes the owned-workspace lifecycle gaps without changing release state.

## Contract

- A session in `closed` or `error` state cannot return to `open` or `connecting` when an older poll completes. Final events remain readable and automatic polling stops.
- Explicitly installing/restoring a workspace replaces its transient request-editor state and closes its live sessions, even when workspace/request IDs stay the same. Ordinary edits and successful saves preserve editor input and connections.
- Request/account switches and unmount close the session captured by their cleanup; a cleanup cannot close a newer session.

## Tasks and evidence

1. Add failing stale-poll and same-ID workspace replacement regressions; preserve existing authentication and pending-creation guards.
2. Make terminal session state monotonic and stop its polling.
3. Track explicit workspace replacement independently of server revision; use it to reset owned editors and session identity, and reject old HTTP responses/errors/private updates.
4. Check captured-session cleanup and ordinary-save behavior, and inspect the shared-workspace equivalent for the same race.
5. Run focused/full frontend tests, TypeScript, production build, and update coverage with the exact completed scope.

## Ledger

- Baseline `362a8f6`: focused protocol lifecycle, workspace controllers and version history pass (24 tests in 3 files).
- SharedLiveSessionPanel already rejects terminal-to-live state regressions; the owned hook lacks the same guard.
- Release operations are out of scope by the user's explicit instruction.

- Task 1 complete: five regressions failed against the baseline for their intended behavior (terminal `closed`/`error` reverting to live, same-ID session replacement, transient request draft retention, late HTTP private updates). Captured-session cleanup and ordinary-save preservation already passed.
- Task 2 complete: terminal states reject stale live summaries, final events remain visible, and the terminal session stops its polling interval. All four terminal/live-state combinations pass.
- Task 3 complete: explicit installations advance a local generation separately from persisted revision. Editors/body scopes and live-session identity follow that generation. A synchronous generation reference fences late HTTP results, errors and private updates, including responses resolved before the replacement rerender.
- Task 4 complete: captured-session cleanup on request change/unmount, pending creation during replacement, and ordinary-save input/connection preservation pass. SharedLiveSessionPanel already has the terminal adoption guard; no shared-path changes were needed.
- Task 5 complete: focused 36 tests pass; all 808 frontend tests in 148 files pass (12 added); TypeScript and production build pass; 38 frontend chunks pass cycle validation; `git diff --check` passes. No backend or installed-native GUI claim is made for this frontend slice.

## Remaining parent-plan boundaries

- Installed native GUI/file-chooser acceptance and real macOS/Windows service registration still need platform execution; existing build or mocked-manager evidence does not close them.
- Workspace versioning currently covers snapshot history/restore. Branch/merge/Git workflows remain outside the completed slice, as recorded in WORKSPACE-VERSION-COVERAGE.md.
- The 395-item feature matrix is a requirements catalog rather than a completed implementation checklist. This plan closes the owned-resource lifecycle gap only.
