# Socket.IO Client Implementation Plan

> For agentic workers: execute with subagent-driven-development, one fresh Rust implementer and separate reviews. Root owns frontend/interchange.

Goal: actual namespace/event/ACK Socket.IO client shared by hosted server and offline desktop.
Architecture: mature rust_socketio/rust_engineio SDK owns protocol; checked transport adapter and existing owner/session pipeline; dedicated lazy React editor.
Tech Stack: mature Rust Socket.IO/Engine.IO async SDK, existing Tokio/reqwest/TLS libraries; React/Radix/CodeMirror.
Spec: docs/specs/socketio-client.md.

## Global constraints and review focus

Full usable features before packaging refinement. No local Docker. No custom protocol parser/frame state machine. Preserve saved drafts, alias/private scope, checked DNS+original TLS hostname, owner quotas/cancel/logout, bounded payloads/ACKs/events/queues. Final delivery main; preserve originals and remove only merged extra branches at final integration.
Review: unchecked SDK reconnect/resolver paths, incorrect namespace/path/auth behavior, binary attachment/ACK representation loss, callback leaks after timeout/cancel, early/late connection and private variable/source export races.

## Task 1 — Rust client and API/session integration

- [ ] Inspect actual mature SDK APIs/transport/versions; choose compatible library and settle JSON contract early. Verify WebSocket transport support against current Postman official limitation; expose polling/legacy only if actually supported and tested.
- [ ] Add canonical typed config/interpolation/pre-script/privacy/validation without breaking existing protocols. SDK owns parse/heartbeat/namespace/acks; app glue validates/bounds inputs.
- [ ] Connect through checked/pinned socket/TLS transport. Disable or bound automatic reconnect. Minimal documented/licensed SDK observer/connector patch if needed, no replacement parser.
- [ ] Add dedicated emit/listener/ACK commands/events with quotas, state/cancel/close and original scoped variables. Test source/JSON/binary roundtrip, server/client ACKs, invalid input/timeout/late logout, settings and ownership.
- [ ] Run real mature Socket.IO server fixture (isolated own port18886+) and focused/workspace tests, strictClippy/fmt; report tasks/implementation/socketio-report.md and stop source work for independent review.

## Task 2 — Dedicated UI/interchange and actual validation

- [ ] Root implements lazy namespace/path/auth/listeners/event+JSON/bytes/ACK editor against the early contract, shared labels/controller/session types and default/foreign export handling.
- [ ] Regression tests for pending/late/retained session cancel, listener/ACK state, draft navigation/scope and credential export; actual agent-browser namespace/auth/emit/listen/ACK/binary flows and narrow/a11y.
- [ ] Independent backend/frontend review, fix findings and verify final feature; commit/proxy push. Continue MQTT and remaining full matrix. Build/refined packaging only after features per user's latest instruction.
