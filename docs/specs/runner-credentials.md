# Workspace-scoped runner credentials

## Purpose and boundary

A self-hosted agent currently uses an account session or PAT. That bearer can reach owner APIs unrelated to the job it is trusted to run. Replace agent authentication with a one-time-issued credential bound to exactly one owner, runner ID and workspace ID. The agent still receives the selected workspace snapshot (including request credentials) when it claims a task; this feature limits which snapshot it may receive and which service APIs it may call. It does not sandbox a malicious agent or protect the selected workspace from that agent.

Hosted-only. Offline collection execution remains unchanged. The owner uses a login session to manage agent credentials; a PAT or agent credential cannot mint, list or revoke them. Queue/list/cancel, schedule editing, registry editing and all other owner APIs continue to use existing owner authentication. Agent `claim`, `heartbeat` and `complete` require the scoped credential; broad sessions/PATs are rejected on those three routes after the CLI and tests migrate. No fallback to broad credentials or another runner/workspace when a credential is missing, expired or revoked.

## Threat model

- Stolen agent bearer: may call only the three agent endpoints for its bound runner and workspace; cannot queue work, read arbitrary workspaces/reports, manage tokens, or use another runner. A claim explicitly reveals the queued workspace snapshot and its request secrets.
- Stolen lease token alone: cannot call any route without the agent bearer. A bearer for another runner/workspace cannot reuse the lease token.
- Revoked or expired bearer: fails on the next admission even on another service instance. An already leased task is cancelled at the next heartbeat; its completion is denied. Revocation itself durably cancels the matching active lease under the account lock.
- In-flight race: route admission alone is insufficient. Claim/heartbeat/complete recheck credential identity, owner, runner, workspace and expiry inside the same account-locked transaction that changes task state. Revocation takes that lock before deletion. A request ordered before revocation may finish; a request ordered after it may not.
- Owner credential revocation remains the existing account-wide generation fence. Revoking one agent credential must not invalidate unrelated owner sessions, runners or credentials. Workspace deletion cascades to its scoped credentials and jobs atomically; same-ID recreation cannot revive an old bearer.
- Failed/late responses, metadata, logs, CLI stdout, browser storage and exports never contain the bearer or its digest. Issuance returns the bearer once over the authenticated response; the caller must store it privately. The browser drops it when the one-time dialog closes or account changes.

## Data and API contract

- Bearer format: `moleapi_runner_` plus 64 lowercase hexadecimal characters generated with a CSPRNG. Persist only SHA-256 of the full bearer, not plaintext. Reject malformed prefixes/lengths before DB lookup.
- Store one credential as an owner-scoped `documents` row with kind `runner-credential`, ID `runner-credential:<digest>` (indexed primary key), and `ref_id` equal to the workspace ID so existing workspace deletion cascades remove it. The private payload contains a random public credential UUID, runner ID, workspace ID, display name, created/expires timestamps, but no bearer or digest. This avoids a migration and gives the guard a keyed lookup. Do not return the document ID in metadata.
- Cap 100 credentials per owner. Name: 1–80 non-control characters after exact trim check. Expiry: 1–365 days, default 90. Expiration is checked at every admission and transactional recheck. Rotate by minting a new credential then revoking the old one; no mutable secret endpoint.
- `GET/POST /api/runners/{runner}/credentials`: list metadata / create with `{workspace_id,name,expires_in_days?}`. Creation validates owned enabled runner and owned live workspace. Response adds `token` once. `DELETE /api/runners/{runner}/credentials/{credential}` revokes by public UUID and returns `{revoked:true}`. Missing, foreign or wrong-runner resources return not-found. Management requires `SessionAuthentication`, as PAT management does.
- Global auth guard recognizes the runner prefix and inserts a scoped agent identity. It admits this identity only to exact protected method/path shapes for claim, heartbeat and complete; all other protected routes return forbidden. Owner sessions/PATs do not gain `AgentScope`. Agent handlers require that extension, and management handlers reject it. Do not rely on an untrusted request body or path prefix alone for scope enforcement.
- A scoped claim skips queued tasks for other workspaces and may claim only a task matching both bound IDs. The task stores the public credential UUID when leased (`None` for legacy persisted tasks). Heartbeat/completion require the same UUID and task workspace. A different credential for the same runner/workspace cannot take over an active lease. Queued tasks are owner-authored and may later be claimed by another valid scoped credential for that runner/workspace.
- Revocation uses the existing account-row lock, deletes only the matching credential, and durably cancels a matching active task/lease. Any schedule linked to that task reconciles through the existing schedule path. Do not increment the account-wide credential generation for a single agent revocation. Workspace deletion and expiry also deny active heartbeat/completion; expiry may leave metadata until normal reconciliation.
- CLI `runner agent --id` uses the existing global `--token-file` or `--token-env`, but that value must be the scoped bearer. `runner credentials create/list/revoke` use an owner login session. Never put the generated bearer in argv or config examples; show private file/environment usage. Hosted UI management follows the existing token dialog pattern and clears one-time secret state on close/account change. Native offline mode never calls these hosted routes.

## Acceptance gates

1. Two hosted routers sharing SQLite: issuance, exact route allowlist, foreign owner/runner/workspace denial, no credential material in list/error/task metadata, malformed token denial, expiry, and fresh cross-instance revocation.
2. Race fixtures establish account-lock ordering for claim versus revoke, active lease cancellation and denied heartbeat/completion after revoke; unrelated credentials and queued tasks are unaffected. Workspace deletion and same-ID restore cannot resurrect the bearer.
3. Actual CLI process uses the scoped bearer to claim a saved request against a loopback fixture and submit a redacted report. A broad PAT/session fails on agent routes; an agent bearer fails on owner APIs and on another workspace. Existing one-attempt and cancellation behavior remains unchanged.
4. Hosted UI can issue/copy once, list metadata and confirm revoke with account/credential fences. Native UI makes no hosted calls. Bilingual catalog, frontend tests, typecheck/build and narrow-viewport accessibility checks pass.
5. Rust focused tests, strict relevant Clippy/rustfmt, frontend tests/build and embedded binary build pass. PostgreSQL/MySQL and platform installation evidence remain a later milestone, not implied by SQLite fixtures.

## Deliberate exclusions

Label/group routing, response-detail history upload, service installation, runner identity attestation, OS keychain storage and arbitrary per-request credential projection remain separate work. A scoped bearer is not an authorization boundary for the target APIs inside its selected workspace.
