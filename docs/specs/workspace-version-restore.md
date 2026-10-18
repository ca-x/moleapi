# Workspace version history and restore

Status: implementation specification. Matrix target: `versioning-006` (resource version history and recovery). This slice covers whole-workspace snapshots already written by `storage::replace_in`; per-resource diffs and branch merges remain separate versioning work.

## Objective and assumptions

An owner can inspect saved workspace revisions and restore one without overwriting a newer edit. The restored contents become a new revision, so the previously current workspace remains available in history. Hosted workspaces continue to use their existing owner boundary; local desktop workspaces keep the `local` identity. No team membership is inferred from knowing a workspace ID.

The existing `GET /api/workspaces/{id}/versions` returns prior full `Workspace` snapshots, and `storage::replace_in` archives the current revision on every successful replacement. Restore must use that shared replacement path rather than a separate write model. The application already has an offline workspace draft and save-conflict flow; restoring through the UI must preserve an unsaved draft until the user explicitly confirms the operation.

## Contract

1. List versions only after checking ownership of the current live workspace. Show revision, saved time, name, and a bounded content summary before loading a full snapshot into the UI. Do not show history for a deleted workspace. Unknown workspace and cross-owner requests return the same not-found result.
2. `POST /api/workspaces/{id}/versions/{revision}/restore` accepts `{ "expected_revision": currentRevision }`. The revision path selects one archived snapshot belonging to the authenticated owner and live workspace. `expected_revision` must be a positive current revision; a stale value returns conflict without a write. A missing archived revision returns not found.
3. The saved snapshot supplies workspace name and data; its historical `id`, `revision`, and timestamp cannot override the live workspace identity or revision. Validate the restored data with the current workspace validator, scrub local-only values on the hosted server, and write through `storage::replace` so existing Mock share revocation, rate reset, and new history insertion execute. Reconcile running protocol sessions as on a normal update.
4. A successful restore returns the new `Workspace` at `expected_revision + 1`. The pre-restore live value is archived exactly once. Concurrent restore/edit operations across server instances have one winner through the database compare-and-swap; losers return conflict. Repeated submission of the same request after success is stale and cannot create another revision.
5. The UI presents a revision picker with timestamps and a readable preview of the selected historical workspace. It requires a confirmation that names the target revision. If the current draft has unsaved changes, the existing guard flow must protect them before restoration. On success the returned workspace replaces the local draft, invalidates workspace/history queries and clears the selected preview; on conflict it offers the current server version without discarding the draft.
6. History may contain sensitive request data. Keep it behind the same authenticated owner route, omit it from logs/analytics, never put its contents in URL parameters, and preserve default private-export behavior. The preview must render user text as text, never executable HTML.

## Source and commands

- Rust routes and owner check: `crates/server/src/lib.rs`, `crates/server/src/workspaces.rs`; snapshot lookup and CAS: `crates/server/src/storage/mod.rs`; real API tests: `crates/server/tests/database.rs`.
- React workspace controls, API client and i18n: `web/src/features/workspaces/`, `web/src/shared/`, existing catalogs under `web/src/`.
- Verify with `cargo test -p moleapi-server --test database`, `cargo clippy -p moleapi-server --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `npm --prefix web test -- --run`, `npm --prefix web run build`, then the repository's configured SQLite/PostgreSQL/MySQL CI job. Use a real 390px browser flow for the new UI.

Follow existing Axum handler and SeaORM query conventions. For example, return `ApiError::not_found()` for both unknown and foreign workspace IDs and return `ApiError::conflict("Workspace revision changed")` for failed CAS; do not leak a snapshot before the owner check.

## Verification and boundaries

- Real API tests cover owner success, cross-owner denial, unknown/deleted workspace, invalid and absent revision, stale CAS, current content preserved as a new archived version, request removal revoking a Mock public share, and current session cleanup.
- Database tests run against all three configured engines; a same-revision concurrent test demonstrates one winner. Frontend tests cover unsaved draft guard, confirmation, conflict handling, and safe preview. Browser checks cover desktop and 390px Chinese/English save, restore and reload.
- Keep the current personal-owner model and offline behavior. Adding team membership, branch merge, Git sync, revision retention policy, or a new schema is outside this slice and remains open in the full matrix.
- Never commit secrets or historical snapshot payloads into diagnostics. A schema or permission-model change requires updating this specification and its migration/compatibility tests before implementation.
