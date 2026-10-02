# Independent backend foundation review

Reviewed the current untracked implementation in `crates/core` and `crates/server` against `docs/API-CONTRACT.md` and `docs/specs/database.md`, plus the backend implementation report. Source inspection only; no production code edits, commits, or target-code executions. Existing test source was reviewed, but its reported passing results were not independently rerun. This review concerns the implemented foundation, not the future full feature matrix.

## Findings

### P1 — Bound interpolation before allocating expanded strings

`crates/core/src/interpolation.rs:14` appends environment values with no output budget; `replace` applies this recursively to every string, and `resolve_request` only calls `validate_request` after all expansion (`:66–75`). The body-size check in `validation.rs:27` consequently runs too late. A valid workspace can contain a 1 MiB environment value; a roughly 50 KiB text body containing 10,000 references to it attempts approximately 10 GiB allocation before the 5 MiB body rejection. The same expansion is reachable through hosted `/api/execute` and collection runs, and through fields without a body cap. `execution.rs:33` resolves synchronously before networking; the core request timeout also starts after resolution. One authenticated account can therefore drive shared server memory consumption far beyond the advertised input/body bounds. No allocation-exhaustion test was attempted.

Use checked incremental output budgets before every append, with a whole-request budget and applicable field limits; include encoded form output in the budget. Reject oversized expansion before allocating it. A regression can use a modest variable and repeat count with a deliberately small internal test budget; do not reproduce resource exhaustion.

### P2 — Detect recreated remote workspaces before acknowledging synchronization

`crates/server/src/sync.rs:21–24` persists only connection identity and two revision numbers. The action calculation at `:285–294` treats unchanged revision numbers as unchanged data even when the earlier content comparison already found different contents. Supported API operations permit deleting and recreating the same workspace ID with revision 1 (`workspaces.rs:50–60`).

Reproduction sequence: sync a new workspace so both bases are 1; delete its hosted counterpart; recreate the same hosted ID at revision 1 with different data; sync again. The code chooses `(false, false) => "noop"`, returns `synced`, and records bases while local and remote still differ. A subsequent local edit chooses push and overwrites the recreated remote without a divergence conflict. This violates the contract that differing states must not be silently acknowledged or overwritten.

Persist a remote content fingerprint and/or immutable incarnation identifier with each base, and incorporate it into change detection. A remote timestamp or content fingerprint can identify this immediate problem without changing ordinary revision CAS. Test recreation both at revision 1 and at a later revision matching an old base; require a pull or explicit conflict, never a false synchronized acknowledgment.

### P2 — Encode workspace IDs as URL path segments

`crates/server/src/sync.rs:268` uses `url::form_urlencoded::byte_serialize` to build a path. That dependency explicitly encodes a space as `+`; Axum path decoding preserves `+`. Workspace creation accepts spaces (`workspaces.rs:51–53`). Thus ID `my workspace` is fetched remotely as `my+workspace`. Initial sync can create the real ID through the JSON POST, but a later sync still cannot fetch it and attempts duplicate creation, producing a conflict. If the plus-spelled ID already exists, the remote-ID check rejects the result instead.

Use a path-segment encoder that represents spaces as `%20`, retaining literal plus as `%2B`. Also reject `.` and `..` as workspace IDs or otherwise prevent URL dot-segment normalization, because they are currently accepted and passed through `Url::join`. Regression coverage should round-trip space, plus, percent, question mark, Unicode, and dot-segment IDs or assert documented rejection.

### P2 — Preserve saved mock error responses through JSON error middleware

`crates/server/src/lib.rs:217–238` rewrites every non-JSON 4xx/5xx response into an API error. It also wraps the saved mock route, whose purpose is returning an example's actual status, headers, and body. A valid example with status 404 and `text/plain` body `missing fixture` is transformed into `{ "error": "Not Found" }`, and its custom headers disappear; a saved 422 additionally becomes 400. This violates the explicit mock endpoint contract.

Mark successful mock-handler responses with an internal response extension and exempt those responses, or normalize framework rejections at a boundary that does not rewrite intentional endpoint responses. Merely exempting all errors by request-path prefix would also exempt real routing/extraction failures. Add saved 404, 422, and 500 examples, checking their exact status/body/custom headers while preserving JSON errors for a missing example and malformed API input. Existing mock coverage only exercises a 201 example.

## Reviewed controls and remaining verification

- Account guards are attached to protected hosted routes; workspace storage keys and document queries include owners. The inspected execution, history, runner, versions, mock, and export entry points check ownership. Local identity is fixed and native synchronization routes are absent from hosted routers.
- First registration is serialized by a transactional singleton-row update; failed setup/disabled-registration checks roll back. Unique usernames have a database constraint. Password hashing and verification are offloaded; session tokens are random, only digests are stored, expiry is checked, and logout deletes the digest.
- Workspace replacement uses a conditional owner/ID/revision update, checks affected rows, and inserts the prior snapshot in the same transaction. Native final workspace/base commit fences local edits and connection identity changes. These controls do not fix the remote recreation finding above.
- Core transport resolves and checks all destination addresses on each redirect, pins them in a no-proxy client, caps redirects and streamed response bytes, rejects downgrade/non-HTTP redirects, and strips authorization/cookie across origins. Native sync requires HTTPS except resolved loopback HTTP and disables redirects for credentials.
- SeaORM entities and migrations are shared across engines; MySQL large document payloads use LONGTEXT and InnoDB/utf8mb4; SQLite uses WAL, foreign keys, and one pooled writer. No additional concrete SQL-dialect defect was established from the reviewed source. Real PostgreSQL 16/MySQL 8.4 migration and CRUD/CAS tests remain necessary: enabled drivers and SQL generation are not verification. The existing env-driven integration suite covers these operations when actual service URLs are supplied.
- Module boundaries separate policy/interpolation/transport from authentication, workspace storage, execution/history, runner, mocks, synchronization, and format adaptation. No style-only refactoring finding is raised.
- Current tests do not cover the four findings. The implementation report's 28 passing tests and SQLite smoke evidence remain implementer-reported, and the absent local PostgreSQL/MySQL runs remain an explicit validation gap.

This is a bounded correctness and security-boundary review, not an exhaustive penetration test or a claim that every future capability is complete.
