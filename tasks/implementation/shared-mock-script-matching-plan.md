# Shared Mock scripts and routed matching

Authority: `docs/specs/workspace-collaboration.md`, member resource/editor parity and actor attribution before shared access.

- Extract pure matching-rule and script editors from the owned example editor. Reuse them in the shared draft. Match inputs cover bounded priority, query/header pairs and optional exact body; script editing appears only in an already enabled v2 workspace. Source/schema/data/traffic fields stay intact.
- Add `ANY /api/owners/{owner}/workspaces/{workspace}/mocks/{*path}` for editor/admin members. Reuse the existing bounded owner Mock dispatcher rather than copy matching or script execution. Check membership under the canonical workspace lock, audit the actor without URI/body/credentials, watch membership and workspace revision during execution, and recheck both immediately before returning.
- Preserve existing owner-only Mock paths and public links. Viewer/foreign/removed members cannot invoke the scoped matching route. Reuse URI/body caps, sensitive-header/query filtering, selected-only script execution and Mock call logs. The shared UI displays only the scoped matching path for literal HTTP URLs.
- Tests first fail for the missing route/controls. Cover real matched script/fallback, duplicate query/header/body criteria, sensitive header exclusion, wrong method/owner/viewer, delayed cross-instance role/version changes, source preservation and v1 script gating. Run affected server tests, full frontend, strict Clippy, format and production/embedded builds, then actual bilingual 390 px member save/preview/routed-call acceptance and independent review before push.
- Data-binding editor, new specification-link selection, public share delegation and other protocols remain required separate work; this does not close collaboration or Mock parity.

## Execution record

- RED: missing scoped catch-all route and shared script/matching controls reproduced by new server/client tests.
- GREEN: selected script and static fallback, duplicate query/header/body matching, private input filtering, method/owner/viewer/body-limit denials, actor-only audit and cross-instance role/revision changes. Shared CAS tests retain existing schema/body/traffic settings and v1 scripts remain gated.
- Verification: full frontend 500 cases; server Mock 31, collaboration 13, routed Mock 2, local SQLite database 2. Production frontend verifies 45 chunks, strict server Clippy passes, embedded server rebuild passes, package rustfmt and diff checks pass. `cargo fmt --all` cannot resolve a nested vendor workspace from this worktree, so the changed server package is checked with `cargo fmt -p moleapi-server -- --check`.
- Runtime: actual member UI at 390 px in English and Chinese saved the script and query/header/exact-body rules, then previewed generated status 202. Member-token routed POST returned script-generated method/body/source for matching conditions and static fallback otherwise. Owner read confirmed the static body persisted; owner audit attributed both routed calls to the member with no request content. No horizontal overflow or browser errors. Evidence linked in `docs/COLLABORATION-COVERAGE.md`.
- Independent source review: no concrete privacy/admission/race/compatibility defect found; reviewer did not rerun tests.
- Full collaboration remains incomplete. Data binding, new specification links, other resources/protocols, native hosted sync and current-head CI/package acceptance remain required.
