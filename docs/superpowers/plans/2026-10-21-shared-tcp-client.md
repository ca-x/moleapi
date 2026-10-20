# Shared TCP client implementation plan

> **For agentic workers:** Use executing-plans inline and verification-before-completion. One final independent review covers this whole change.

**Goal:** Shared editors/admins create/save/configure TCP requests, explicitly connect/send/half-close, inspect bounded events and close with canonical member fences.
**Architecture:** Admit saved TCP through existing member session routes and only TCP commands through their send policy. Extract controlled message/connection fields from the owned workbench; reuse them for shared saved configuration and a session-local message editor. Existing codec, TLS, DNS, quota, privacy and creator lifecycle remain authoritative.
**Tech Stack:** Existing Rust/Tokio protocol engine, React/Radix/CodeMirror, Vitest.
**Spec:** docs/specs/tcp-client.md and docs/specs/workspace-collaboration.md shared TCP subsection.

## Constraints and review focus

- Raw/line/length-LE/BE framing, text/Hex/Base64, secret flag, max frame/idle/Nagle/timeout/TLS/network fields preserve existing bounds/semantics and source.
- TCP is GET/no body and explicit tcp(s)://port; HTTP auth/header/query controls and HTTP handshake are absent. Pre-scripts and canonical scoped variables remain supported.
- Canonical saved configuration changes only through explicit shared CAS; active-session messages are local and never automatically mutate or save that source.
- Create/send/half-close/event/close inherit actor, creator, credential, membership generation and expected revision checks; audit stores action/subject without payload.
- Half-close confirmation binds the exact session/scope and rechecks current/open/not-half-closed before sending; late sends/edits/cert reads cannot target a new request/account/cache context.
- Existing owned TCP plus shared Data/GraphQL/WS/SSE behavior stays compatible. No new external service/library or database schema.

## Task 1: Member transport

- [x] RED real saved member TCP create/send test against current whitelist.
- [x] Admit Protocol::Tcp and TcpSend/TcpHalfClose only; audit content-free TCP actions.
- [x] Canonical-owner variables/collision, creator/viewer/foreign/stale admission, raw/line/length framing, half-close continued read, private payload withholding and revision/logout cancellation evidence.

## Task 2: Reusable authoring and live controls

Files: new tcp/TcpMessageEditor.tsx and TcpConnectionSettings.tsx; owned TcpWorkbench.tsx; new workspaces/SharedTcpRequestEditor.tsx and SharedTcpSessionControls.tsx; SharedWorkspaceView/SharedLiveSessionPanel; bilingual catalogs/tests.

- [x] RED missing shared create/config/Save/connect/send and viewer absence tests.
- [x] Extract owned fields preserving owned lifecycle. Shared authoring stages full source through CAS; new TCP button and no HTTP controls.
- [x] Session-local message controls use saved template initially, no auto-send, exact TCP command, secret/encoding, half-close dialog and existing event viewer; no HTTP handshake tab.
- [x] Scope/certificate/late-send/half-close/dirty/credential/cache tests and owned compatibility.

## Task 3: Verification and delivery

- [x] Full frontend/server affected tests; production build, strict Clippy/package format, embedded server build.
- [x] Independent source review; actual bilingual390 TCP create/config/save/connect/send/half-close/events/disconnect and desktop acceptance.
- [ ] Docs, commit/push/PR update, observe exact-head CI/Docker. Native installed and remaining full matrix gates stay open.

## Ledger

- Previous goal turn was progress:5b5b100 pushed with665 frontend/48 backend checks and bilingual real SSE/WS authoring/handshake. Its Docker39472111313 and app39472111318 are confirmed live; no timeout-based restart.
- Existing isolated collaboration-next worktree is clean at task start; retain root user mise.toml untouched.
- Backend RED: new real member test returned400 at TCP create because the existing whitelist excluded TCP. Finite HTTP/GraphQL policy unchanged; saved member TCP and only TcpSend/TcpHalfClose now admitted, content-free TCP actions audited. First integration passes all4 codecs, canonical owner collision/snapshot, viewer/foreign/stale/creator denial, half-close continued read and role cleanup.
- Frontend RED: absent new TCP button. Full saved-source/session-local flow now passes and owned TCP3 tests remain green. Retained template callback initially overwrote newer framing and retained settings callback applied into a new scope; both RED then GREEN after latest-config merge and explicit original-scope guard.12 targeted units pass including half-close Cancel and5 changed-context confirmations.
- Second real integration passes private Hex/Base64 payload screening plus actual peer EOF after workspace revision and actor logout; revision invalidation may return409 before asynchronous removal404, both withhold data.
- Full frontend129 files/675 cases passes. Full backend library22 + collaboration13 + shared Data2 + GraphQL1 + TCP2 + owned TCP2/budgets4/framing6/security5 =57 passes. Independent source review found no concrete issue; runtime/full-gate exclusions are covered by current gates and browser below.
- Owned message field extraction retains the direct editor flex sizing through a scoped tcp-message-editor class. No shared source height override; shared message editors use explicit180px height.
- Runtime finding: returning shared→the already selected owned workspace cleared draft/saved while keeping selectedId, leaving the workbench empty because the installation effect depended on selectedId but not draft identity. New hook regression reproduced null instead of the selected original. Add draft identity to the effect dependencies so explicit clearing rehydrates the authoritative selected list item; ordinary same-ID edits remain untouched. Full controller and browser return verification required below.
- Owned-return hook regression RED(null draft after clearing unchanged selectedId)→GREEN, full controller15 pass and full frontend129/676 pass. Shared actual Chinese text and English Hex resolve real line bytes, no auto-send/source mutation; Cancel/Confirm EOF receives after EOF and disables sending. Idle10000ms also visibly closes, then explicit Save idle0/reconnect succeeds. Canonical template remains saved {{region}} at revision3, wire exactly member owner-region\n and hi\n; actor audits contain no payload.
- Prior exact-head app39472111318 completed: database success, check failed in generation Java timeout assertion without actual error text. Added assertion error-chain diagnostics without changing probe/timeout behavior. Local full generation library93 passes/12 configured ignores, so CI root cause remains unestablished; new head must be observed rather than claiming it fixed.
- Final676 frontend, server57 and generation93/12 configured ignores,43-chunk production, strict server+generation Clippy, package fmt and embedded rebuild pass. Final embedded runtime shared send→same-owned return restores the255.25px editor and peer closes; no browser errors/1280 overflow. Four EOF notifications cover idle, two half-closes and final scope cleanup; canonical source/revision unchanged. Five docs captures retained.
