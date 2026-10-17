# Bounded scripts for saved Mock examples

Authority: `mock-006` and `mock-012` in `docs/FEATURE-MATRIX.md`, `docs/specs/pm-datasets.md`, and the existing owner-scoped saved-example route. This slice adds an opt-in dynamic response to one saved example. Broader route matching, schema generation, public sharing and Mock call logs remain separate Mock rows.

## Contract

- A saved example may include additive `mock_script`. Old examples and workspaces keep the exact static status, headers and body. A nonempty Mock script requires workspace script mode `v2`; unsupported external exports reject it rather than silently dropping it.
- `GET /api/mock/{workspace_id}/{request_id}/{example_id}` remains owner authenticated. The script sees a bounded, read-only snapshot of the actual incoming GET path, query and safe headers through `pm.request`. Authorization, cookies and connection headers never enter the script. The saved request definition and example remain unchanged.
- The script runs in the existing isolated worker under its 1-second process watchdog, QuickJS memory/CPU limits and the shared script capacity semaphore. The worker has no filesystem or network. It exposes the same owner-checked `pm.datasets(id).executeQuery/executeView` handle, including on-demand Data refresh through the host bridge.
- `pm.mock.setStatus(status)`, `pm.mock.setHeader(name,value)` and `pm.mock.setBody(value)` edit one response draft seeded from the saved example. A string body is used verbatim; a JSON-compatible nonstring value is serialized. The final status must be 200–599, body at most 5 MiB, and header names/values valid. Hop-by-hop, nominated connection and content-length headers are stripped after the script, using the static Mock route's existing filter. Mock scripts cannot persist variable updates or mutate the incoming request.
- Script errors, missing/redacted datasets, deadline expiry, owner/revision changes and invalid response drafts fail closed without serving the saved body. Failure responses do not reveal private dataset rows, credentials or authored SQL. A default MoleAPI backup screens or omits private Mock scripts; a full backup restores exact text.

## Verification

- Core old-example roundtrip and size/mode validation; full/default backup and external-export behavior.
- Real local and hosted Mock requests: static unchanged, dynamic body/status/header, bound literal and live dataset query, foreign owner rejection, incoming credential exclusion, invalid status/header/body, worker timeout and privacy.
- Browser save/reload/preview in English and Chinese at 390px; inspect actual response and horizontal overflow.
- Focused Rust/frontend tests, strict Clippy, production build, three-database CI and current native package checks before claiming matrix support.

## Implementation

`crates/core/src/models.rs` owns the additive example field; `crates/script-runtime` owns the `pm.mock` API and worker response draft; `crates/server/src/mock.rs` supplies the owner-scoped incoming snapshot and constructs the response; `web/src/features/requests/ExamplesEditor.tsx` owns the editor. Reuse the saved dataset query bridge and existing Mock header filter.
