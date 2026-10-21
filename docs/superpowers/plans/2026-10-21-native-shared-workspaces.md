# Native hosted shared workspaces implementation plan

> **For agentic workers:** Execute inline with executing-plans and verification-before-completion. One independent final review; no implementer agents.

**Goal:** Native hosted connections expose the complete existing role-aware shared workbench through canonical remote operations without copying member data into native owned storage.
**Architecture:** Add an explicit allowlisted, session-generation-fenced local JSON bridge using existing secure hosted transport. Adapt shared API dispatch and member guards to a public native connection marker, then integrate the existing picker/settings/shared view while preserving local owned sync and file APIs.
**Tech Stack:** Existing Axum/SeaORM/reqwest/http/url/percent-encoding, React Query/Radix/native invoke; no new dependency/schema.
**Spec:** docs/specs/native-shared-workspaces.md and docs/specs/workspace-collaboration.md.

## Task1: local hosted member bridge

Files: new sync/collaboration.rs and tests/sync_collaboration.rs; sync.rs/lib.rs/route policy.
- [x] RED absent endpoint; real hosted owner/editor/viewer and native same-ID collision, invitation acceptance/list, canonical CAS and stale conflict, execute and raw Mock status isolation.
- [x] Deny unknown JSON fields; explicit method/path/mode/query allowlist; fixed-origin secure client; bounded streamed request/reply and nested status/value response. Public session generation required, no bearer/header overrides.
- [x] Do not hold sync_lock during HTTP. Initial/poll/final generation fences cancel delayed headers/body and permit concurrent Stop/disconnect/reconnect. RED tests account/server/session changes and unknown write outcome; existing owned sync stays green.

## Task2: native member API and connection boundary

Files: new shared/nativeMemberSession.ts and tests, shared/api.ts/tests, sync/useSync.ts/tests; member credential imports in shared workbench controls.
- [x] RED actual native invoke/member-envelope dispatch, JSON statuses/errors and raw Mock target401 vs auth401. Hosted token never returned to JS; owned APIs retain local invocation.
- [x] Dedicated member credential marker, request generation capture, stale-response/401 fence, current remote401 disconnects member UI only. Protect status/connect/disconnect races and remove generation-scoped caches. Shared guards use marker without changing local auth token semantics.
- [x] Native remote capability/metadata/sessions/Mock/report calls all use the same member adapter; native pick/save retain mature controls with current-context checks.

## Task3: native picker and settings

Files: useWorkbenchController.ts/tests, SettingsPanel/SharedWorkspacesPanel/tests, App props if required.
- [x] RED connected native listing/role labels/selection/editor/viewer/admin/invitation flow and local ownership separation; disconnected no remote query.
- [x] Existing SharedWorkspaceView receives connection-scoped identity. Account/server/reconnect/expiry clears selection/draft/caches and fences retained Save/run/live actions; native local workspace and unsaved local changes preserved.
- [x] Explicit online remote state and no offline replay; reconnect reloads current role/revision. All shared controls available under existing role policy; no native local invite or owned push/pull of remote member data.

## Task4: verify and deliver

- [x] Full frontend/type/build and affected backend; sequential embedded build, strict scoped Clippy/fmt/diff; one independent final review and regressions for material findings.
- [x] Actual final-built native-bridge/hosted wire plus en/zh390/1280 member Save/execute/Mock/data/Stop/viewer/admin/invite/reconnect/current-scope QA and a11y/screenshots; document installed-platform validation gaps explicitly.
- [x] Coverage/capability/route/status/spec/ledger, explicit commit/push/preserved PR and exact-head CI. Full395-capability/native distribution/EDAS/platform release boundaries remain open.

## Review focus

- Local/remote same-ID workspaces, different remote accounts/servers/sessions must never share writes, tokens, query caches or callback results.
- Malformed URI encodings, path separators/dot segments, query credentials and unsupported methods cannot turn this into an unrestricted authenticated proxy.
- Save completion may be unknown after disconnect; never retry it automatically or silently replay an offline draft.
- Stop/reconnect must work during long requests; no global sync lock around network I/O, bounded headers/body watcher and final session fence.
- Mock401/HTML/Set-Cookie and remote401 have different semantics; preserve inert response and current-connection expiry without logging out the native local app.

## Ledger

- Prior turn progress: shared Mock debugger7237c4c committed/pushed/PR41 updated, final frontend772/backend80/review/browser gates pass. Worktree clean before this plan. Exact-head Docker39519294120 (worker/source passed; images running) and Application39519294214 (source passed; check/database running) verified live; no timeout-driven restart.
- Native gap confirmed in controller enabled:!native, native local-only api invoke, null native token guards and local-only sync bases. Existing secure hosted runner gateway provides fixed-origin/DNS/no-redirect utilities but holds sync_lock during HTTP and accepts only management paths; reuse security primitives with independent member operation fence.
- Ruling: connect native shared view online to canonical hosted APIs with scoped memory caches, rather than copy member documents into native owned workspaces. Current spec allows scoped caches and forbids silently accepted offline edits. Cost if wrong: explicit remote-member UI scope; owned sync remains independent and future offline reconciliation can use its own contract.

- Final source verification and real runtime/a11y evidence are recorded in NATIVE-SHARED-WORKSPACES-COVERAGE.md. Independent final reviewer rechecked all5 findings and final one-level JSON-text bearer guard clean. Full frontend796/type/build passed. Installed native GUI/platform and release-specific gates remain open. Prior Mock7237c4c Application39519294214 and Docker39519294120 both completed success.
