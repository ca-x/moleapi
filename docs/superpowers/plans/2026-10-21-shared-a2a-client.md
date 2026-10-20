# Shared A2A client implementation plan

> **For agentic workers:** Execute inline with executing-plans and verification-before-completion; one independent final review after implementation.

**Goal:** Shared editors/admins configure, discover/import/restore and explicitly save original Agent Cards, then use creator-bound A2A message/task/stream/push operations without canonical runtime writes.
**Architecture:** Admit saved A2A and its2 commands in member transport/audit. Reuse typed SDKs and checked card fetch/candidate screening through saved request/revision-bound member routes, cancellable by the initiating actor. Extract controlled A2aClient/CardEditor and preserve owned wrappers. Shared source stages candidates; runtime copies only invocation settings and never adopts endpoints or mutates shared cards.
**Tech Stack:** Existing mature a2a-client0.1/0.2 engines, reqwest network policy, serde, React/Radix/CodeMirror.
**Spec:** docs/specs/a2a-client.md and shared A2A subsection in docs/specs/workspace-collaboration.md.

## Constraints and interfaces

- Member live Connect accepts saved request_id/expected_revision and canonical owner variables/auth/pre-script only. Commands exactly a2a_request/a2a_stop, action-only audit, creator/member/credential/revision lifecycle retained. Existing supported0.3 JSONRPC and1.0 JSONRPC/HTTP+JSON combinations remain unchanged.
- Member /a2a/cards/import, /read, /discover and /discover/cancel all require canonical saved A2A request/revision and editor role. Imports return source-preserving candidates, read verifies linked canonical specification/dialect, discovery uses canonical auth/TLS/timeout/network/scripts and explicit URL only. No local scope/request/dialect overrides. Candidate screening includes inherited owner private values and updated pre-script private values. Final credential/member/revision fence and watcher cancel actual fetch.
- Discovery id is actor-bound; Stop/close invalidates late candidate. Import/discovery never saves or starts message/task traffic. Explicit interface adoption and template replacement use current request/scope/epoch confirmation; shared source actions require saved clean context, while owned save-before-discover remains compatible.
- A2aClient consumes scope/request/onChange/dark/isCurrent/disabled, optional A2aChannel and optional controlled card/source actions. Source has no channel/Run/runtime network; runtime deep copy has local params/method/template/task controls but no Host/card/dialect/transport/source mutation. Task cancel confirms exact current session/task; local Stop remains separate.
- DTO/template/content semantics reuse mature existing modules. All supplied source/file/confirm/call/Stop callbacks check mounted/current request/account/environment/admission and epoch. No new dependencies or custom wire parser. No new animation for frequent actions.

## Task1: backend

Files: collaboration.rs, collaboration/a2a.rs, a2a.rs, lib.rs, tests/collaboration_a2a.rs and existing SDK fixtures.
- [x] RED member saved A2A admission/command and source-route tests (400/404 before implementation).
- [x] Extract checked fetch/candidate helpers; implement member source routes and actor cancellation with lifecycle fences, canonical privacy and no persistence.
- [x] Admit exactly2 commands/action audits. Real fixtures prove both versions/transports, canonical owner collision/auth/scripts/params, task state/history/artifacts/stream/get/cancel/resubscribe/push/errors/Stop, default privacy, linked schema/source preservation and actor/revision/role/logout fetch/session termination.
- [x] Run cargo test --locked -p moleapi-server --test collaboration_a2a --test a2a; commit only with frontend integration and final gates.

## Task2: controlled/shared frontend

Files: features/a2a/A2aClient.tsx, A2aCardEditor.tsx, existing owned wrappers/hooks, SharedA2aRequestEditor/SessionControls and shared view/panel/i18n/styles.
- [x] RED shared create/import/candidate/explicit Save/Connect/local message/task/Stop/cancel flow and retained scope/source/confirmation guards.
- [x] Extract controlled modules, preserve owned source/keyboard behavior; implement saved member card routes and staged source, explicit template/interface confirmation; shared runtime deep-copy and no canonical writes.
- [x] Direct retained/delayed form/export/discovery/reopen/session/task callback tests plus owned compatibility and shared flow pass.

## Task3: verify/deliver

- [x] Full frontend/typecheck/production and affected backend; strict server Clippy/package fmt/diff, actual embedded build.
- [x] Independent review; actual embedded bilingual390/1280 card/source/interface/template/message/task/stream/history/artifact/Stop/cancel/owned compatibility and light/dark a11y, screenshots and canonical preservation.
- [ ] Coverage/map/status/ledger; commit/push, preserved PR update and exact-head CI/Docker observation. Whole28-module395-capability/native distribution/full platform matrix remains open.

## Review focus

- Late discover/import/file/template/interface confirmation cannot overwrite a changed/reopened draft or different account/environment.
- A retained task cancel/Stop must not operate on a replacement session/task after membership/credential/revision loss.
- Canonical linked card wins over member same-ID workspace/card/locals and pre-script private outputs never enter persisted candidates or labels.
- Supplied URLs and card interfaces never trigger implicit adoption/automatic OAuth/media/file loads; network/redirect policy applies to actual fetch and stream.
- Member role/revision/logout loss while discovery/stream is pending closes transport and withholds a late candidate/result; imported raw unknown fields survive explicit Save.

## Ledger

- Preflight: shared MCP1dc84d0 committed/pushed; working tree clean before this plan. New-head Docker39502988936 and app39502988938 confirmed live; research passed. Existing A2A official SDK/client/source/fixtures support0.3 and1.0; member whitelist/UI currently lack A2A.
- Ruling: continue native inline execution under standing user instruction to finish remaining functions — no extra plan approval checkpoint — cost if wrong: implementation choices are reviewable in this plan/PR before merge/publication.

- Backend RED: initial saved A2A session400 and member Card import404 in /tmp/moleapi-shared-a2a-red.log. Current6 shared backend tests and all10 owned A2A tests pass in /tmp/moleapi-shared-a2a-lifecycle.log, including official0.3/1.0 JSONRPC and1.0 HTTP+JSON, authenticated owner script/variable preparation despite member collision, source privacy/link/explicit CAS and actual held discovery sockets closed within2s after actor Stop/role/revision/logout. Another editor cannot stop that actor's discovery.
- Frontend controlled A2aClient/CardEditor plus owned wrappers and shared source/runtime integrated. New shared flow RED missing creation, then GREEN explicit Card candidate→Save→Connect→local params/Stop with no canonical writes. Related202 frontend tests passed before adding3 direct confirmation/retry cases; current16 direct A2A tests and TypeScript pass.
- Source-only extraction initially re-created a fallback events array each render, causing a pending-state effect loop. Existing source-only regression exposed it; stable no-channel events fix it. Source/editor test RED missing controlled module; retained Stop/source/reopen cases now pass.
- Ruling: shared runtime displays the canonical linked Card source passed by its parent, rather than trusting a possibly outdated inline request.card_source — server already selects the canonical linked specification — cost if wrong: one optional UI source prop, with raw fallback only when no linked source exists.
- Test harness corrections: JSON unknown-field rejection uses400 in this app, shared CAS uses expected_revision and member audit lives at /audit. No product changes were made for those harness mistakes.
- Remaining: full gates, final independent review, actual embedded browser and docs/delivery; nothing in this A2A slice is committed/pushed yet.

- Final review: Important final-check cancellation race accepted and fixed. Test actor_stop_during_final_authorization_withholds_completed_candidate creates a completed-candidate/final-auth barrier, cancels as actor and then releases permission checking: RED original successful release, GREEN with Lease::finish cancellation recheck after successful auth. Owned discovery had the same shape and now uses the same final release guard. No deferred minors or rejected findings. Reviewer declined browser/native/complete SDK-platform/full gates; browser and relevant gates now separately verified, while native/full platform scope remains open.
- Final full frontend754/139 and TypeScript/production38 chunks pass. Pre-fix affected backend71/2 configured ignores; post-fix23 server unit+10 owned A2A+6 shared A2A pass and strict server Clippy/fmt/embedded build pass. Shared real matrix now also saves negotiated canonical Cards and exercises1.0 task-list/push CRUD and missing-task errors. Private inherited auth and numeric source candidates are explicitly rejected.
- Actual final isolated QA executable: /tmp/moleapi-shared-a2a-qa/moleapi-server; SDK fixtures0.3:18944 and1.0:18943; app:18933 with separate QA workspaces. Browser source candidate/CAS/endpoint/template confirmations/raw unknown original restore, legacy keyboard/input-required/history/artifacts/context/get/cancel/stream/Stop, current HTTP+JSON task-list/push CRUD/stream, exact canonical runtime preservation and owned dirty-save-before-discover pass. Read-only CodeMirror textContent omits visual newlines; innerText proves exact restored source including whitespace, with no product change. QA labels use fresh snapshots/current translations.
- Final screenshots en/zh390 and1280 inspected; no page overflow/browser errors/automatic file or push callback fetch. Both scoped light/dark a11y0 violations/1 incomplete contrast check. docs/images/workspace-collaboration-a2a-{en,zh}-390.png saved. Native chooser/installed distribution, long-run and full platform/database integration remain open.
- Delivery pending commit/push/PR and exact-head CI/Docker. Prior MCP1dc84d0 Docker39502988936 completed success on native amd64/arm64 + container smoke, manifest skipped because PR; app39502988938 check remains live, database/source passed. Whole28-module395-capability goal remains active.

- Final affected suite after the cancellation fix:72 passed,2 configured fixture ignores; cargo fmt and git diff --check exit0. All named QA browser/app/fixture processes stopped; screenshots inspected. Existing locale maps exactly preserved with5 new keys each. Source is ready for the already-authorized commit/push and PR checkpoint.
