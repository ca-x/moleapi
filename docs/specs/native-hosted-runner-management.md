# Native management of hosted runners

## Objective

Let the offline Tauri application manage its connected hosted account's runners and scheduled tests while keeping local workspaces independent. A native user explicitly connects to a self-hosted service through the existing sync login. Runner management uses that saved login session; no hosted bearer is exposed to JavaScript or sent to an arbitrary URL.

## Boundary and contract

- The local Axum router provides an IPC-only `/api/hosted/...` gateway. It is available only in local mode after sync connection. The gateway admits exact method/path shapes for owned workspace metadata, runner registry/credentials, runner tasks, schedules and report summaries/details. It rejects arbitrary paths, query strings and bodies above 1 MiB. It constructs the remote URL from validated path segments and reuses sync's HTTPS/loopback-only, DNS-pinned, redirect-disabled, timed client. Responses are bounded to 20 MiB so a maximum-size workspace remains manageable. Remote auth failures show a reconnect error and never reveal the stored token.
- A local user can see the remote workspace revision and resources before queueing; the gateway never substitutes the local revision. If the selected remote workspace is missing, the native UI asks the user to synchronize before dispatch. Hosted actions use the fetched remote snapshot and leave local data untouched even when local edits differ.
- Native settings show connected hosted runner registration, label editing, enable/disable, workspace-scoped credential issue/list/confirmed revoke, direct/label task dispatch, cancellation and redacted report summaries. Hosted schedule creation and cancellation use remote canonical references and target selectors. Account/connection/workspace changes fence late responses and erase one-time secrets. The existing offline testing UI remains local.
- The native gateway never exposes agent claim/heartbeat/complete, owner access-token management, arbitrary remote HTTP calls or local connection secrets. An agent credential is still the only bearer accepted on hosted agent endpoints.

## Acceptance

Write local-router/remote-host fixtures for exact gateway allowlist, disconnected/expired sessions, URL restrictions, ownership, one-time credential privacy and remote-revision queueing. Write native UI tests for connection/workspace changes, no calls when disconnected, hosted target selection, confirmed revoke and failed sync. Compile source and test code before the final test execution phase requested by the user.
