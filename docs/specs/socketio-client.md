# Socket.IO client contract

Authority: user requires complete Apifox/Postman protocol capabilities through mature Rust crates; features first, packaging refinements last. Follow the gRPC slice with a real dedicated Socket.IO client, not a native WebSocket label.

## Evidence and scope

Postman official Create a Socket.IO request page (agent-browser,2026-10-03) accepts ws/wss and explicitly does not support long polling. Existing catalog also identifies Apifox Socket.IO event controls. Implement at least the WebSocket transport, namespace, custom Engine.IO path, auth/headers/query/environment variables, named listeners, named event emits, binary arguments where supported, outgoing ACK request/results, incoming ACK reply when the mature crate exposes it, timeout/cancel/disconnect/error events and saved drafts. Verify supported Socket.IO/Engine.IO versions against mature library APIs; never falsely label native WebSocket or unsupported protocol versions as supported Socket.IO. Additional polling, legacy versions or reconnection options should follow evidence and actual SDK support.

Use rust_socketio/rust_engineio0.6 async (or a demonstrably more complete mature Rust client), library Socket.IO/Engine.IO parsers, ack machinery, heartbeat and wire transport. App code may adapt checked pinned connections and expose needed SDK observers with a minimal provenance/licensed patch, but may not implement a competing protocol/parser. Check transport injection before committing to dependencies. No unchecked second DNS, environment proxies or weakening of hosted network/TLS policy.

## Module and interface approach

Extend core saved Protocol with a typed Socket.IO config: namespace, Engine.IO path, source-preserving JSON auth draft and initial listened-event names; keep common URL/header/auth/query/settings/script/variables. Validate incomplete saved drafts separately from execution. Namespace/path are bounded and validated; no arbitrary frame construction from UI input.

Reuse owner/admission/generation/session/event/retention infrastructure, with protocol summary `socketio`. Add dedicated event/message/ACK/connect/disconnect/status data preserving event names, arguments, binary attachments and callback identifiers. Send commands emit named events, optionally wait for ACK, add/remove listener controls if SDK requires them, and answer server-requested ACK where supported. The library owns wire IDs and protocol parsing. Do not use generic raw WebSocket text send for Socket.IO.

Use the existing script preparation/privacy pipeline, resolving auth JSON and outgoing messages with original selected variable scopes. Account/workspace/request/environment/logout changes cancel in-flight connection and drop stale UI/event/ACK updates. Bound event names/listeners/payloads/ACK requests/queues and callback lifetimes; maintain existing byte/session quotas. Disable or explicitly implement SDK automatic reconnection so it cannot evade ownership/deadline/cancellation or create unbounded callback/session leaks.

Exact JSON contract must be sent to root before dedicated frontend implementation; compatible additions preserve older workspace JSON. Native full MoleAPI exports preserve Socket.IO data while default exports redact literal credentials/private overrides; external formats reject unrepresentable configuration. Request/response scripts not supported by a particular event phase must be explicitly tracked for cross-protocol scripting completion.

## Dedicated frontend

Lazy React/Radix/CodeMirror module: namespace/path/auth/listeners configuration, event-name/JSON/binary send editor, optional ACK timeout, listener toggles, connect/Stop (including pending creation), event/ACK log and payload inspector. Shared sidebar/protocol selectors show Socket.IO accurately. Keep keyboard operations immediate and reuse motion tokens only for occasional state feedback, no animated message list. Native assets embedded; HTTP/IPC delegate through Rust. Canonical drafts saved immediately and fenced across navigation, schemas and scopes.

## Verification

Real mature Socket.IO server fixture; configurable namespace/path/auth/header rejection, named JSON/bytes events in both directions, ACK success/error/timeout, server-requested ACK if supported, listener update semantics, disconnect/Stop/pending/late logout, original variable scope, private target deny and original hostname TLS. Verify mature version/transport compatibility in actual fixture. Tests for invalid config/input/size/scope/export and ordinary browser flows, narrow layout/a11y. Independent backend/UI reviews before commit/proxy push. No local Docker or extra packaging work now. Final full-feature delivery goes to main; preserve original untracked scaffolds before integration and delete only merged redundant branches.
