# Shared SSE and WebSocket authoring implementation plan

> **For agentic workers:** Use executing-plans inline and verification-before-completion; one independent review of the whole change at the end.

**Goal:** Members can create/configure/save SSE and WebSocket requests, connect the canonical saved request and inspect its actual handshake and events.
**Architecture:** Extend the existing shared request authoring controls, retain saved creator-bound member sessions, extract the owned handshake renderer for reuse. Add synchronous member/cache admission to existing live operations.
**Tech Stack:** React19, Radix, TypeScript, Vitest, existing Axum member session routes.
**Spec:** docs/specs/workspace-collaboration.md, shared live authoring subsection.

## Constraints and review focus

- Create fresh IDs and preserve every authored field through explicit canonical CAS; dirty requests cannot connect.
- SSE and WebSocket use GET/no body under the existing validator; WebSocket uses HTTP1. Neither uses HTTP body/assertion controls. Inherited auth, query/header/pre-script/network and request timeout/TLS/redirect settings remain configurable.
- Shared authoring never mounts owned token/cookie managers or writes member-local values.
- Handshake URL/status/headers are the already authorized session projection, never a new owner fetch; revocation clears them with events.
- Late network/certificate/editor callbacks cannot target another selected request, actor, credential, role, saved revision or newer cached version. Existing creator/session cleanup remains.
- Existing Data/GraphQL/HTTP workflows remain covered; no new protocol is silently admitted by backend policy.

## Task 1: Authoring and handshake

Files: SharedWorkspaceView.tsx/.test.tsx; new protocols/SessionHandshake.tsx; ProtocolPane.tsx; SharedLiveSessionPanel.tsx; bilingual catalogs.

- [x] Add absent-control RED cases for SSE/WebSocket creation, configuration CAS, Save-before-connect, viewer absence and handshake rendering/clearing.
- [x] Implement creation and supported shared fields; extract/reuse owned handshake renderer; expose common request connection settings without owned managers.
- [x] Add selected-request/edit-context fence to shared network file callbacks; preserve current request fields when applying delayed certificate imports.
- [x] Run affected frontend tests, then owned compatibility tests.

## Task 2: Admission and verification

- [x] Add RED regressions for cached revision/credential/selection invalidation before live connect/send or between summary and event reads; implement explicit isCurrent and mounted/generation checks.
- [x] Verify real canonical member CAS and live WS/SSE headers/query/pre-script/handshake through existing backend tests or extend relevant fixture assertions.
- [x] Full frontend/server affected tests, production build then strict Clippy/format and embedded build.
- [x] Independent source review, actual English/Chinese390 creation/Save/connect/handshake/message/disconnect, desktop check and docs.
- [ ] Commit/push/PR update; observe exact-head application/database/Docker CI. Whole objective remains active.

## Ledger

- Previous goal turn was progress: committed/pushed795cfc7,653 frontend/56 backend tests and actual embedded bilingual browser verified. Exact-head Docker39469136800 and app39469136799 currently live; do not restart based on observation timeout.
- Ruling: choose existing SSE/WebSocket authoring before new protocol families, because shared sessions already work but members cannot create or configure their handshake. This completes their existing member workflow; other protocol families remain required.
- Authoring/admission RED: missing new buttons and four direct stale-cache regressions all failed before implementation. Controls and admission guards now pass six targeted cases; initial complete affected frontend group192 passed.
- Ruling: both SSE and WebSocket retain existing GET/no-body behavior. The real hosted CAS rejected an SSE POST with400, matching core validation. Removed method selection for both rather than exposing an unusable option; POST-based streaming remains outside the existing protocol contract.
- Independent review Important: clearing session before cleanup changed stateRef to undefined and skipped closing the original live resource. Added real polling regressions: missing old-session close and unwanted teardown during pending Save both failed. Cleanup now always closes its captured session ID with the original credential; polling uses currentContext independent of temporary saving, while new operations remain blocked during Save. No second concrete finding; final review excluded runtime/full gates, verified below.
- Final review fix GREEN:6 direct panel tests including original session cleanup and pending Save continuation pass. Final full frontend665/127 passes; backend48 passes (library22, collaboration13, shared Data2, GraphQL1, authoring1, protocols9). TypeScript/production43 chunks, strict Clippy, package format/diff and embedded server build pass.
- Actual rebuilt English/Chinese390 member browser creates/saves WS/SSE from empty collection, inherited auth/query/pre-script wire checks, echo and SSE event,101/200 handshake, disconnect and owner canonical revision3/source verify. Width390, errors empty; desktop1280 fits. Four retained docs screenshots. Native certificate chooser not exercised; delayed file tests cover request/cache/credential.
- Review exclusion ruling: actual unit polling RED→GREEN proves orphan cleanup dispatch, real shared backend creator/session and browser disconnect prove the invoked close path; no claim of separately instrumented real-browser stale-cache injection. Other protocol families and installed native/current-head release gates remain required.
