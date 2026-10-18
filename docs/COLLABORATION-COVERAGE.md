# Hosted workspace collaboration coverage

Status: integrated feature still in progress in `feat/workspace-collaboration`. This file records evidence for the current worktree, not a released or merged capability. Authority: `docs/specs/workspace-collaboration.md` and `docs/COLLABORATION-ROUTE-POLICY.md`.

## Implemented paths

| Capability | Current evidence | Boundary |
| --- | --- | --- |
| Invitations and members | Hosted session create/list/revoke/accept, owner/admin role changes and removal, single-use digest tokens, same-account retry, per-acceptance generation and indexed cursor-paginated membership; cross-instance acceptance/rejoining and deletion/recreation tests | No organization/group ACL or delivery integration |
| Shared workspace data | Explicit owner/workspace GET for viewer-safe projection and editor/admin CAS PUT; owner IDs remain distinct when workspaces share an ID | Full editing parity with every workbench resource is pending |
| Shared execution | Editor/admin collection run and canonical saved-request execution use the owner workspace, audit the real actor, check membership concurrently every 200 ms and withhold results after removal/rejoining | Ad hoc request, protocol/Mock/runner routes remain owner-only; outbound side effects already sent before revocation cannot be rolled back |
| Saved results | Editor/admin history and report summary/detail/step response reads recheck membership before returning | Viewer history/report access and member report mutations/export remain closed |
| Audit | Successful invitation, membership and shared workspace writes append within their transaction; collection run start records the actor; owner/admin session lists latest 100 | Other resource actions need actor attribution before member access opens |
| Hosted client | Owner/admin invitation/member controls, one-time URL fragment link, current-session acceptance, distinct owned/shared picker keys with owner and role, viewer read-only view, basic editor CAS save/collection run/saved-request send/history/report view, pending-save edit retention, unsaved-change guard and shared query cleanup on workspace/account exit | Request editor parity, remaining account switch/cache stress tests, native hosted sync and package validation remain open |

## Current verification

- Local SQLite: `cargo test -p moleapi-server --test collaboration --test database --no-default-features` passed 8 collaboration and 2 database tests, including same-role rejoining during an in-flight run and cursor pagination. The valid large viewer projection and migration upgrade unit tests passed. Re-run on a fixed branch head before publication.
- Strict server Clippy, Rust formatting, diff checks and embedded server build passed for the current backend changes. The latest full frontend run passed 452 cases and the production build verified 46 chunks, including pending-save edit retention, paginated shared listing, owned/shared ID collision and delayed execution-result isolation tests. Re-run on a fixed branch head before publication.
- Real Chromium at 390 px, English: owner created a viewer invitation, another account accepted, opened the shared picker, saw read-only content, became editor, saved a CAS update, resolved a concurrent owner edit, then lost the open view after owner removal.
- Real Chromium at 390 px, Chinese: after a fragment-navigation fix, an already open member session accepted a second invitation and opened the shared editor. An unsaved editor draft triggered a leave guard; confirming discarded the draft and cleared the view. Captures: [English viewer](images/workspace-collaboration-viewer-en-390.png) and [Chinese editor](images/workspace-collaboration-editor-zh-CN-390.png).

## Still required before claiming completion

Finish member routes for ad hoc execution, protocol/Mock and other resource operations with privacy and cancellation fences; finish role-aware editor parity and account/logout cache tests; run full frontend/embedded server checks, three-database fixed-head CI, Docker and native installed-package smoke, independent review, and the broader matrix/release audit. EDAS real integration is still awaiting a sandbox.

Migration note: the collaboration migration has not been published on main. It upgrades main's existing three-migration databases. A temporary database created with the earlier unmerged `b427b0f` collaboration migration lacks the later membership generation column and must be reset before reuse; an additive upgrade is required if that checkpoint is ever distributed.
