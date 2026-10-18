# Request-level version history and restore

Status: implementation specification. Matrix target: the request-resource part of `versioning-006`. This builds on `docs/specs/workspace-version-restore.md` and existing whole-workspace snapshots. It does not introduce a second authoritative storage model.

## Objective and assumptions

An owner can see how one saved request changed across workspace revisions, inspect an earlier request state, and restore only that request while preserving unrelated current workspace edits. A deleted request can be restored from an archived state. The current personal-owner boundary applies in hosted mode; the offline client uses its existing local identity. A saved workspace revision is the history source of truth, so no background index or migration is needed.

## Contract

1. `GET /api/workspaces/{workspace}/requests/{request}/versions?before=<revision>` returns a page of changes for one request. It checks the live workspace owner first. Each entry contains archived workspace revision, revision timestamp, request name/method, original collection ID and whether the request existed at that revision. It does not return request body, credentials or headers. Unknown workspace and foreign owner are indistinguishable 404 responses.
2. The server scans at most 32 archived workspace revisions per page, one row at a time. It compares each request state and collection ID with the immediately newer state, starting with the live workspace for the first page. Unchanged request states do not produce entries. A page returns at most 20 entries and an exclusive `next_before` cursor when older revisions remain. A later page loads the cursor revision as its comparator; no client-provided digest controls filtering. A request absent both before and after a revision is skipped. Rows are ordered by revision descending.
3. `GET /api/workspaces/{workspace}/requests/{request}/versions/{revision}` returns that one archived request state, its collection ID and revision timestamp. `request: null` represents a historical state in which the request did not exist. The path revision must belong to the same owner and live workspace; missing revisions return 404. No full workspace snapshot is returned to the browser.
4. `POST /api/workspaces/{workspace}/requests/{request}/versions/{revision}/restore` accepts `{ "expected_revision": currentRevision, "destination_collection_id": optionalId }`. It requires a live workspace and an archived revision. If the target state contains a request, replace the current request in place when it exists. If currently deleted, insert it at the end of its historical collection when that collection still exists, otherwise require an explicit existing destination collection. If the target state is absent, remove the current request. A no-op target returns conflict without writing.
5. Restore validates the complete resulting workspace with the current validator, scrubs local-only values on hosted servers, writes through the existing `storage::replace` CAS and reconciles protocol sessions. A successful restore returns the new `Workspace`, archives the prior current version exactly once, and runs existing removed-request Mock share revocation. Stale expected revisions return 409. SQLite lock races use the same bounded retry as whole-workspace restore; persistent database failure remains an error.
6. The request editor offers a history dialog showing a paged change list, safe text preview of the selected request and a confirmation that names the revision. It shows request deletion explicitly before confirmation. Unsaved drafts use the existing save/discard guard; conflicts use the existing server-version dialog. After success, the current request selection and response are reconciled, history is invalidated, and a switched account cannot receive a prior account's response or cached history.

## Source, style and commands

- Backend: `crates/server/src/workspaces.rs` or a focused `request_versions.rs`, routes in `crates/server/src/lib.rs`, existing document entity and `storage::replace`; real three-database API cases in `crates/server/tests/database.rs` or a dedicated suite.
- Frontend: request editor action, a focused dialog under `web/src/features/requests/`, existing workbench context/guard, bilingual catalogs and focused React tests.
- Existing Axum/SeaORM handler style applies; respond with `ApiError::not_found()` only after the owner check and `ApiError::conflict("Workspace revision changed")` for CAS failures. Do not log request payloads.
- Verify with `cargo test -p moleapi-server --test database`, strict server all-target Clippy, scoped rustfmt, `npm --prefix web test`, `npm --prefix web run build`, configured SQLite/PostgreSQL/MySQL CI, and actual 390px Chinese/English browser restore/reload.

## Verification and boundaries

- Test unchanged workspace saves being omitted, changes across page boundaries, deletion/recreation, missing historical collection with and without destination, unrelated current edits preserved, invalid linked specification rejected without write, cross-owner denial, concurrent CAS, and public Mock share revocation on historical deletion.
- Test that a large archived workspace cannot cause an unbounded list response or scan, that list entries omit request secrets, and that only the selected revision returns request details to its owner.
- Keep branch/merge review, Git synchronization, resource diff rendering and a retention policy as separate open matrix scope. No new schema or dependency is required for this slice.
