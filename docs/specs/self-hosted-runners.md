# Self-hosted execution runners

## Objective and scope

Implement owner-bound hosted runner registration, metadata/enablement, durable queued collection/scenario tasks, pull-based claims, leases/heartbeats, cancellation and idempotent redacted report completion. A native CLI agent executes claimed work through the existing offline Rust router on the runner's network. This advances monitoring-006/007, testing-019 and integrations-006; schedule targeting/UI/agent packaging and remaining full matrix are separate outstanding work, not waived requirements.

## Architecture and interfaces

Shared core models define runner metadata, selection, task status and claim envelope. Server runners models/storage/API remain separate from protocol execution. Use existing SeaORM document storage, account ownership and workspace row locks. Short account-row transactions serialize runner operations, followed by workspace/document locks; execution happens outside these transactions. Store task configuration and its dispatched workspace snapshot privately; metadata responses never include snapshot, credentials, lease digest or raw reports. Runner access inherits authenticated account ownership (session/PAT); a runner is explicitly trusted with selected workspace request credentials. Routes are hosted-only. Registration/list/update use /api/runners; workspace queue/list/cancel use /api/workspaces/{id}/runner-tasks; agent claim/heartbeat/complete use /api/runners/{runner}/... . Authentication uses existing credentials and no new transport parser.

## State and invariants

Tasks are queued, leased, completed, failed or cancelled. Queue stores validated collection/scenario/environment/saved-data/request-filter selection and expected workspace revision. Claims snapshot the current revision-bound workspace, generate a random opaque lease token, persist only its SHA256 digest and expire after90seconds. Heartbeats extend leases. Lease tokens/runner IDs/owner/workspace/status are checked atomically. Queued tasks and running cancellation are durable. Explicit cancellation, runner disable, owner credential revocation, workspace deletion/recreation or revision mismatch stop or deny the claim; agents cancel local jobs when heartbeat fails. Revocation generation is captured at queue and rejected after an owner event. Default max_attempts1; explicit1–3 allows lease-expired retry and at-least-once execution, which may repeat side effects. Claimed tasks are not delivered concurrently by two instances. Bound each runner to one active task; horizontal scaling uses separate registered IDs. Cap100 runners/account and100 queued/leased tasks/workspace; retain latest100 terminal task metadata. Single task snapshot/report limits remain bounded.

## Reports and privacy

Completion locks the same durable job and writes a redacted report through the shared report projector/storage/notification path in the same transaction. Repeated completion returns the existing result; stale leases cannot complete a newer attempt. The snapshot supplies source revision, environment and private scopes. Worker-local history IDs are discarded. Raw result payloads are never stored in task metadata or stdout. Agent-private overlays use the existing CLI variables parser; runner output is metadata only. The hosted service trusts registered runner execution results, and retains canonical report privacy rules.

## CLI and operation

CLI runner register/list/set-enabled, task queue/list/cancel and agent --id support authenticated hosted operation only. The agent owns an isolated temporary offline SQLite database per task, imports the dispatched native snapshot, runs existing collection/scenario/data/filters, heartbeats every20seconds while executing, and submits completion. --once handles one claim (or exits empty), otherwise it polls every2seconds. Ctrl-C cancels local work and submits cancelled completion when possible. Local runner variables use --variables-file/--variables-env; values never enter argv/saved remote sources. No local Docker build or arbitrary installation/network address is executed by generation.

## Acceptance and remaining work

Necessary actual hosted tests prove two-instance exclusive claiming, ownership, invalid selections, stale/expired lease rejection and configured retry, cancellation/disable/revocation, private metadata and atomic idempotent report persistence. A real CLI/HTTP fixture proves an agent accesses the local endpoint, applies private variables, returns a redacted hosted report, and preserves canonical workspace values. Keep formal review and full DB/platform verification until functional completion. Continue scheduled-run targeting, bilingual runner UI, workspace-scoped credentials, agent service installation/packaging and full matrix afterward. No claim of complete distributed execution or release readiness from this slice alone.
