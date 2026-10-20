# Shared Socket.IO client implementation plan

> **For agentic workers:** Use executing-plans inline, real RED→GREEN and verification-before-completion; one independent review of the completed diff.

**Goal:** Shared editors/admins author and save Socket.IO requests, connect canonical saved configuration and use temporary named events/binary attachments/listeners/outgoing and incoming ACKs through member sessions.
**Architecture:** Parameterize the existing owned Socket.IO workbench into a controlled client. Optional channel makes it a saved configuration editor; the shared live wrapper keeps a separate local config and supplies the member session transport. Preserve SDK version/transport/privacy/limits and existing creator/member/credential/revision fences.
**Tech Stack:** Existing rust_socketio/rust_engineio, React/Radix/CodeMirror, official Socket.IO4.8.1 fixture, Vitest.
**Spec:** docs/specs/socketio-client.md and workspace-collaboration.md shared Socket.IO subsection.

## Interfaces and constraints

- Saved SocketIoConfig unchanged: namespace/path/auth_source/listeners/event/arguments_source/attachments_base64/request_ack/ack_timeout_ms.
- Member session whitelist adds Socketio; send allows only SocketioEmit/SocketioListen/SocketioAck and audits action/session without event/args/attachment/callback token.
- Controlled SocketIoClient consumes scope/config/onChange/isCurrent/dark, optional session channel (summary/events/send/close), optional connection-settings visibility and editor height. Owned wrapper preserves its saved draft update semantics.
- Shared source updates merge current config under original scope/mounted/member/cache/selection fences; only explicit CAS Save persists. Shared live local config starts from saved and sends/changes listeners/replies without implicit canonical writes.
- Namespace/path/Auth use saved connection snapshot; live UI hides their mutation controls. Header/query/auth/pre-script/network fields reuse current shared controls with GET/no Body and HTTP1 WebSocket settings.
- ACK replies bind exact scope/session/callback and consume accepted callbacks even when dialog was dismissed. Late listener/ACK/editor callbacks cannot change another member/source/session. No owner endpoints or local overrides.
- Existing SDK event/attachment/ACK bounds, private value projection, current scope/session cleanup, terminal retention remain unchanged. Viewers have no authoring/execution controls. Other protocols remain required.

## Task 1: Backend admission and real SDK

- [x] Add RED member Socket.IO admission policy test; implement whitelist and typed content-free command audit.
- [x] Official fixture integration proves canonical owner snapshot/collision, emitted JSON/binary and ACK, runtime listeners, incoming ACK/replay denial, private screening, viewer/foreign/creator/stale/revision/logout fences. Fixture-dependent test stays explicitly ignored in offline default suite and is run with --include-ignored here.

## Task 2: Controlled client and shared workbench

- [x] Preserve owned listener/ACK tests while extracting controlled client; add missing shared create/config/Save/connect/local event tests RED→GREEN.
- [x] Add saved configuration wrapper and local live wrapper, generic header/auth/script/network eligibility and correct protocol labels. No automatic sends or source writes from live edits.
- [x] Scope/cache/credential/late listener/ACK callbacks, Cancel/dismissed ACK consumption, template/source preservation and viewer absence tests.

## Task 3: Verification and delivery

- [x] Full frontend/backend affected/explicit fixture tests; production build then strict server Clippy/package fmt and embedded build.
- [x] Independent review; actual bilingual390 author/config/Save/connect/JSON/binary/ACK/listener/reply/disconnect and desktop/owned compatibility acceptance.
- [ ] Docs/screenshots/commit/push/PR update and exact-head CI/Docker observation. Keep whole395-capability goal active.

## Ledger

- Previous goal turn made progress:2b6ee57 pushed with676 frontend/57 backend plus generation93/12 ignored and real TCP/browser/owned navigation evidence. Exact-head app39476168446/Docker39476168329 remain live on this turn's observation; do not restart solely for observation timeout.
- Prior Java CI failure remains unproven locally; assertion error-chain diagnostics are in2b6ee57. No probe behavior change or claim that its CI cause is fixed.
- RED backend admission400 and shared missing create controls; canonical Socket.IO and typed command admission now passes2 tests including explicit official fixture. Source/save/live temporary emit/listener flow passes. Owned/message/new scope9 targeted frontend cases pass. Terminal retained ACK callback and listener retry after admission loss were both reproduced RED→GREEN with latest channel state and identity-only local busy cleanup.
- Independent source review found no concrete issue; full fixture/runtime/complete gates excluded by review and verified here. Full backend45 (lib22, collaboration13, Data2, GraphQL1, Socket.IO2, TCP2, owned Socket.IO3) passed including official fixture.
- Full frontend exposed older in-flight shared poll overwriting a completed close with open, causing reconnect to become disconnect. Deterministic summary-deferred regression reproduced this (closed→open). Shared session adoption now preserves a same-ID terminal state against older live snapshots; event batches can still complete. Full regression/gates below.
- Closed-poll regression RED→GREEN and full affected190 passes. Final full frontend130 files/682 passes; production42 chunks without static cycles. Backend new2 still pass after explicit uninvited actor denial coverage. Default offline policy test does not need Node; official fixture tests were explicitly included here.
- Final strict Clippy/format/diff,42-chunk production and embedded build pass. Actual bilingual390 official SDK author/Save/connect/JSON+binary/ACK ok+timeout/incoming reply/consumed callback/listener/Disconnect/reconnect passes; source/revision2 and audit privacy verified, errors empty and1280 fits. Five retained screenshots. Native install/remaining protocol/release gates stay open.
- Local Node fixture dependency installation created only this task's untracked fixture node_modules; stop the exact own fixture and move that directory into own /tmp QA area before committing. Never stage node_modules or modify another process.
