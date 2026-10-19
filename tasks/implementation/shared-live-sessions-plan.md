# Shared live protocol sessions slice

**Parent:** `tasks/implementation/workspace-collaboration-plan.md`. **Authorities:** `docs/specs/workspace-collaboration.md`, `docs/PROTOCOL-COVERAGE.md`, and `collaboration-003/004` plus `protocols-001/002` in `docs/FEATURE-MATRIX.md`.

## Contract

- Editors/admins may open a canonical saved SSE or WebSocket request through explicit `/api/owners/{owner}/workspaces/{workspace}/sessions` routes. The create input contains a saved request ID, optional canonical environment ID and expected workspace revision, never an arbitrary request draft or member-owned local overrides. Viewers and members using owner-only `/api/sessions` routes remain denied.
- The session uses the owner's workspace, variables, auth and network policy, but is bound to the actual member actor. Session IDs are random and additionally checked against that actor on every summary/event/send/close/delete operation. Actor-labelled start audit has no request body, secret or URL. A member cannot use another member's session ID even when both share the workspace.
- Admission uses the current membership generation/role and canonical revision. A cross-instance role change, removal/rejoin, workspace revision change, workspace deletion or creator credential revocation closes the underlying connection and withholds subsequent private events. Each route checks membership before and after reading or mutating the in-process session; an outbound send already delivered before revocation cannot be rolled back. Existing owned session behavior stays intact.
- The shared web view can connect, inspect bounded events, send WebSocket text/binary/Ping, close, and clear its view on account/workspace/request/role change. A dirty request must be saved through canonical CAS before connect. SSE is receive-only. The existing request/protocol editors remain the source of protocol settings.
- Other live protocols require separate audited policies, including Data postprocessing/history and MCP STDIO authority. A server instance owns its live connection; another instance may check membership in the shared database but cannot resume the in-memory connection after failover. This slice must not imply durable or cross-node session migration.

## Acceptance

- Hosted real SSE/WS loopback fixtures prove canonical owner variables/auth and member actor audit, event polling and WS send/echo/close. Viewer, wrong owner, wrong request/protocol, stale revision, another member's session ID and owner-only URL all reject before private data or outbound work.
- Two hosted routers over one database prove role downgrade/removal/rejoin and workspace revision change close or withhold a live session; logout/revoked credential also closes it. Event polling cannot release a previously captured batch after a downgrade.
- Frontend regressions cover role-aware connect/send/close, dirty CAS gate, account/request switch and late response cancellation. Real 390 px bilingual browser journey, full affected suites, strict Clippy/formatting and production/embedded builds pass. Independent review and fixed-head three-database CI precede any completion claim.

## Implementation order

1. Refactor the existing owner session creation path to accept a distinct credential actor while retaining one protocol preparation/registration implementation. Add member admission checks immediately before and after registration/start, and clean up failed or aborted sessions.
2. Add explicit member routes and an actor-bound in-process session registry with bounded lifetime. Recheck shared DB membership/revision on all operations, watch live sessions for cross-instance changes and creator credential revocation, and record start audit.
3. Add the shared SSE/WS client panel using existing typed session messages and event presentation; preserve the shared view's account/version fences.
4. Run the acceptance gates, update route inventory and collaboration/protocol coverage, review, and push the verified draft head.
