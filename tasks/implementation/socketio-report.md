# Socket.IO Rust implementation report

Base: `0a64ae6`, shared isolated application worktree. No commit, push, Docker, web, desktop, packaging or workflow changes by this implementer. Root owns frontend and interchange.

## Implemented

Real `rust_socketio` / `rust_engineio` 0.6.0 async client integration, verified against official Node `socket.io` 4.8.1. Engine.IO revision 4 and Socket.IO protocol revision 5, WebSocket-only. The SDK owns wire parsing, serialization, heartbeat, namespace state and ACK routing. No native-WebSocket masquerading as Socket.IO, polling fallback, unchecked SDK DNS or automatic reconnect.

The connector uses core `checked_destination`, pins all checked addresses into a direct TCP connection, and supplies the original ws/wss URL to mature tungstenite/native TLS for Host/SNI and certificate verification. Engine.IO accepts that existing WebSocket through a small vendored connector addition. The SDK's unowned polling/reconnect task is never started. Redirects are not followed. Connection deadline includes DNS, TCP/TLS, upgrade, Engine.IO handshake and namespace authorization; pending Stop drops the connection promptly.

Canonical `Protocol::Socketio` fields:

```json
{"kind":"socketio","namespace":"/","path":"/socket.io/","auth_source":"{}","listeners":[],"event":"message","arguments_source":"[]","attachments_base64":[],"request_ack":false,"ack_timeout_ms":5000}
```

Saved incomplete auth/outbound JSON drafts are accepted with bounds. Execution validates auth, namespace/path, ws/wss endpoint, names and SDK-owned query keys. Connecting does not parse, interpolate or send retained outbound drafts. Auth interpolation is structural JSON interpolation, preserving quotes/backslashes. Outbound manual emits resolve event names, JSON strings and base64 through the original selected scoped environment, including pre-script changes and private overrides.

Pre-scripts run once before connection. `pm.request.body` is the auth JSON draft in GET/json mode; body edits reconcile to `protocol.auth_source`, then execute GET/none. Feedback uses `protocol.auth_source` and existing privacy filtering. Unrelated saved body/outbound drafts stay unchanged. Draft post-response scripts are preserved; live execution explicitly rejects them pending the cross-protocol per-event script contract.

Commands, same owner-scoped session API:

```json
{"kind":"socketio_emit","event":"echo","arguments_source":"[\"hello\"]","attachments_base64":[],"ack_id":"correlation-uuid","ack_timeout_ms":5000}
{"kind":"socketio_listen","event":"echo","enabled":true}
{"kind":"socketio_ack","ack_id":"opaque-server-token","arguments_source":"[\"answer\"]","attachments_base64":[]}
```

`arguments_source` is a JSON array of positional arguments. Binary placeholders are SDK `{ "_placeholder": true, "num": 0 }` objects at any nested position, with corresponding `attachments_base64` entries. This preserves multiple/nested mixed JSON/binary arguments in events and ACKs. Every outgoing attachment requires a valid referenced placeholder.

Events use `socketio_event` with event name, `arguments`, `attachments_base64`, optional `ack_id`; `socketio_ack` with `ack_id`, `status` (`ok`, `timeout`, `error`), arguments, attachments, optional error; existing `state`, script log/test, summary and cursor retention models remain in use. Successful ACKs carrying application errors retain those arguments: Socket.IO has no universal application-error ACK convention.

ACKs use SDK monotonic identifiers/oneshot routing and cancellation-safe drop cleanup. Sending enrolls an ACK in wire order before waiting; SDK packet-wide send locking keeps binary headers/attachments atomic. Outbound duplicate user correlations are rejected while pending. Server callback tokens are opaque, session/owner-bound, exactly once and expire after 120 seconds. Pending ACKs receive error events on disconnect/cancellation. Limits: 32 pending ACKs each direction, 64 listeners, 32 attachments, 256 arguments, 1 MiB message, 20 MiB input/received payload, injected SDK 20 MiB wire budget including heartbeat/control/unsolicited packets, existing 32-command queue, 10,000 emits and owner/session/lifetime/retention quotas.

Privacy covers auth JSON strings/numeric credentials and aliases, event names, JSON keys/values, private binary payloads, correlation/error metadata, scoped source updates and handshake/session target. Used credentials are withheld from returned updates/logs/events through the existing redactor. Socket.IO joins existing account admission, logout generation, workspace deletion, ownership and stale-session infrastructure.

## Mature SDK patch provenance

`vendor/rust_socketio` and `vendor/rust_engineio` retain published 0.6.0 source/VCS metadata and original manifests. Both derive from upstream revision `3434b654c18580785c0d0171bc1acbc0378580c7`; MIT LICENSE is included. Each has `MOLEAPI-PATCH.md` documenting precise changes.

The upstream binary decoder previously discarded mixed/nested argument structure by textual replacement of only placeholder zero. The SDK patch preserves complete argument arrays and encodes them through the SDK packet serializer. SDK additions expose its parsed stream, injected Engine.IO connection, typed positional emit/server-ACK reply and bounded ACK waiter. Attachment accumulation is bounded, EOF becomes a clean error, heartbeat is tolerated between attachments and concurrent binary packets serialize atomically. WebSocket binary transport normalization uses the existing SDK MessageBinary decoder to preserve empty attachments and raw byte 0x1e, which upstream previously confused with the polling delimiter. Legacy upstream convenience callback methods are not used by MoleAPI.

## Verification

Official fixture remains running at `ws://127.0.0.1:18886`, namespace `/fixture`, path `/custom/socket.io/`, auth `{"token":"fixture-token"}`, header `X-Fixture: yes`, query `required=yes`. See `crates/protocols/tests/fixtures/socketio/README.md` and locked fixture package. Node-dependent tests are explicitly ignored in the offline default Rust suite and run with `--include-ignored`; all were actually executed against official server code, including the independently spawned TLS server.

Covered real namespace/path/header/query/auth, positional JSON, multiple/nested binary attachment roundtrip, zero-byte/all-256-byte attachment ACKs, 900 KB binary ACK, 16 ordered binary ACK emits, application-error arguments, ACK timeout, duplicate correlation, server callback reply/ownership/exactly-once, listener changes, privacy aliases/auth echoes, heartbeat, namespace disconnect, Stop, handshake deadline, pending cancellation, selected script scopes/body hook, saved malformed drafts and hosted owner/logout/old-token fences. Unconditional tests cover structural interpolation, defaults, invalid config/input/attachment bounds, ACK capacity/expiry, queue admission rollback, private destination denial and ownership.

Final focused run passes 22 tests including every Node case (`/tmp/socketio-focused-final.log`); strict all-target Clippy passes (`/tmp/socketio-clippy-final.log`); `cargo fmt --all -- --check` and `git diff --check` are clean. The earlier full workspace run passed 138 tests with 5 explicitly ignored Node cases (`/tmp/socketio-workspace-tests.log`). A root-owned formats test initially used an HTTP URL for a Socket.IO draft; root corrected the fixture to wss and confirmed 11/11 interchange tests. The final whole workspace rerun passes 139 tests with 5 explicitly ignored Node cases (`/tmp/socketio-workspace-tests-final.log`); the separate focused run executes all ignored cases. Workspace tests and strict all-target Clippy exclude `moleapi-desktop`: the full `--workspace` attempt reaches the pre-existing missing system `javascriptcoregtk-4.1` dependency. No system/packaging changes were made to bypass that environment limit.

Ready for fresh backend review. Frontend, native/default export handling and browser/a11y verification remain root-owned. No polling, transport upgrade, legacy Engine.IO 3 / Socket.IO 2, automatic reconnect, or per-event response scripts are claimed.

## Independent review correction: ordinary event names

The reserved event validator now matches official Socket.IO 4.8.1's six names exactly: `connect`, `connect_error`, `disconnect`, `disconnecting`, `newListener`, `removeListener`. `error`, `open`, and `close` are legal application events. The adapter observes decoded SDK packets and emits string names directly, so SDK convenience callback aliases do not require app-level exclusions.

A core regression test failed before the fix (`/tmp/socketio-reserved-red.log`). The real official-server fixture now echoes these three ordinary names. A new ignored integration test exercises initial listener configuration, live listener commands, outgoing emits, incoming positional JSON and outbound ACKs for each name; it confirms the session remains open after all three.

Verification after the correction: focused 23 tests pass (`/tmp/socketio-reserved-focused.log`), workspace 139 pass with 6 explicit Node-test ignores (`/tmp/socketio-reserved-workspace.log`), strict all-target Clippy passes (`/tmp/socketio-reserved-clippy.log`), fmt/diff checks clean. The new ordinary-name fixture is running separately at `ws://127.0.0.1:18888`; the existing 18886 fixture and root/reviewer servers were left untouched. One initial combined run had an isolated TLS fixture `wrong version number` error; the unchanged standalone TLS rerun and complete subsequent focused run both pass. Its cause is not established; no unrelated TLS code was changed.

Backend source stopped again for review after this verified correction. No commits or pushes.
