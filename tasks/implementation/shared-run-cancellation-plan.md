# Shared collection run cancellation

Authority: `docs/specs/workspace-collaboration.md`, shared collection execution and role-aware workbench parity.

## Behavior

- Editors/admins can stop their own collection run through `POST /api/owners/{owner}/workspaces/{workspace}/run/cancel` with the original `job_id`. Viewers and removed members cannot cancel. Another member's identical client job ID cannot target the creator's run.
- Derive an internal bounded job key from canonical owner/workspace, actor, membership generation/revision and the validated client ID. Pass only that key to the existing process-local job registry; return and audit the client ID. Existing owner-only jobs keep their current key behavior.
- Reuse the registry's bounded early-cancel handling so Stop can arrive before execution registration. Cancellation signals the existing runner token, records the actor without request contents and returns the runner's cancelled result/report.
- The shared web view retains the active client ID, shows the existing bilingual Stop collection run action, prevents concurrent Stop submissions and ignores late responses after scope changes.
- Cancellation reaches the server instance executing the run, as with existing owner runs. Durable cross-instance job scheduling remains a separate capability; database membership changes still fence running shared requests across instances.

## Implementation and evidence

1. Extend hosted collaboration integration tests for real in-flight stop, early stop, malformed IDs, viewer/removal denial, another editor with the same client ID and actor audit. Observe failure before the route exists.
2. Add the internal key derivation, scoped cancellation route and runner key setter. Keep role checks and audit within the workspace-locked admission transaction before signalling cancellation.
3. Add a shared-view deferred-run test proving Stop uses its original client ID and the cancelled result is displayed; add Stop control and scope cleanup.
4. Run collaboration/database suites, focused/full frontend checks, strict server Clippy, format/build checks, then real embedded browser stop. Review the final diff, update route policy/coverage and push the draft PR. Current-head CI/native gates remain required before merge.

## Local execution evidence

- The hosted test first failed because the cancellation route was absent; the UI test first failed because Stop was absent. Both passed after implementation.
- Collaboration 13/13, local SQLite database 2/2, frontend 490/490, production build with 45 chunk checks, strict server Clippy and formatting passed. A Clippy attempt invalidated by a concurrent frontend rebuild was rerun successfully after the bundle stabilized.
- Independent source review found no concrete privacy, actor-isolation, cancellation or compatibility issue. Real English/Chinese 390 px browsers stopped a running 15-second HTTP fixture, displayed cancellation and saved a cancelled report; owner audit identified the member. Both layouts had no horizontal overflow or browser errors. Current-head remote checks remain pending.
