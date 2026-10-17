# Owned Mock call logs

Authority: `mock-008` in `docs/FEATURE-MATRIX.md`, existing owner-authenticated direct-example and routed Mock endpoints, and the shared document storage contract. Capture useful Mock activity without persisting incoming secrets or response bodies.

## Contract

- Every owner-authenticated Mock call that reaches a saved workspace records one outcome: selected example and returned status, unmatched 404, or a generic failure class. Both explicit saved-example and routed endpoints participate. The durable record contains a generated ID, workspace/request/example IDs when selected, method, timestamp, elapsed milliseconds, response status and outcome class. No path or URL is stored because even an authored path might contain a private value. No incoming headers, query values, body, bearer token, script error text, dataset row or response body is stored.
- Store at most 200 calls per owner/workspace, ordered by a database-assigned sequence. Insertion and pruning share one transaction and serialize through the same owner/workspace no-op row lock pattern used by scheduling plus an atomically incremented sequence document, so two hosted instances cannot silently exceed the cap or resurrect a log after workspace deletion. Record only after the response has been built, before serving it. A write failure fails closed; no successful response is returned without its required call record. Calls rejected before workspace ownership is established are not logged. A workspace deletion or revision change during response construction also fails closed and may leave no log because the old revision can no longer be recorded safely.
- `GET /api/workspaces/{id}/mock-calls` lists the newest 200 metadata rows only for the owner. `DELETE` clears current rows; a concurrently admitted later call may appear afterward. Workspace deletion cascades logs and sequence state. The endpoints never return unredacted request/response content and are not included in default or full workspace backup/sync.
- The bilingual example editor exposes a compact recent-activity view filtered to the selected saved request with status, time, example and outcome. It may resolve the current saved request's path in memory but does not persist that path with the call. A clear action is owner-scoped. Existing direct preview and routed response bytes remain unchanged.

## Evidence

- SQLite/PostgreSQL/MySQL real router fixtures: direct/routed success, no-match/error, owner isolation, concurrent bounded append, delete cascade and clear/read races.
- Explicit private inputs (Authorization, Cookie, query token, body, private dataset row and script error) do not occur in API reads or stored payloads. Default/full backup excludes logs. A storage failure cannot serve a successful Mock response.
- Browser English/Chinese 390px call, reload, activity and clear flow; focused Rust/frontend tests, strict Clippy/format/build, configured three-database CI and current-head native package checks.

## Remaining Mock scope

Schema-driven response data, public Mock permissions and simulated delay/failure settings remain separate matrix rows. This log does not claim full traffic capture.
