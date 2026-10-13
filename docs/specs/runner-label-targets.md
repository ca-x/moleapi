# Runner label targets

## Objective

Allow an owner to target one enabled runner directly or a group of runners sharing one label. A label is the group identifier; this slice does not add a second, separately managed group registry. Both interactive and scheduled jobs resolve the target inside the existing account-locked queue transaction. The selected runner ID is persisted on the task. A task never moves to another runner or to the hosted server after it is queued.

## Contract

- `POST /api/workspaces/{workspace}/runner-tasks` accepts exactly one of `runner_id` or `runner_label`. Older `runner_id` payloads remain valid. A task summary keeps its concrete `runner_id` and an optional `target_label` for audit; old summaries deserialize without the new field.
- A label target must match the existing runner-label syntax and have at least one enabled owned member. Invalid, missing, foreign or empty groups fail without creating a task. The server selects the enabled member with the fewest queued or leased tasks for the owner; ties use ascending runner ID. Account locking makes simultaneous selections across hosted instances deterministic from committed state.
- A schedule definition accepts `runner_label` alongside the existing nullable `runner_id`. Neither means local execution; both is invalid. A label means remote execution even if all members later disappear. Each occurrence resolves current membership and capacity once at queue time. Failure records a failed occurrence and sends existing failure notifications; it never falls back to local execution.
- Direct and label-selected tasks use the existing scoped agent credential, source revision, lease, cancellation, retention and report pipeline. Registry label edits do not silently retarget already queued tasks. Disabling a runner cancels its queued and leased tasks; scheduled jobs reconcile through their existing task link.
- The hosted UI and CLI expose direct and label targets for manual queueing and schedules. The UI shows the chosen concrete runner and requested label in task history. Native offline mode continues to use local scheduling and does not call hosted runner APIs.

## Acceptance and privacy

Write cross-instance fixtures for concurrent group queueing, fair deterministic selection, missing/disabled/foreign groups, conflicting selectors, label changes, disable/cancel/reconciliation and no local fallback. Add CLI process and bilingual UI fixtures for selector submission and saved schedules. No agent bearer, workspace snapshot or secret enters new metadata. Run tests, lint, production builds and rendered QA only after the remaining planned functionality and tests have been written, per the user's current instruction.
