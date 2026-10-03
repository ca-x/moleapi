# Protocol sessions: first implementation contract

Authority: FEATURE-MATRIX, CONFIGURATION-OPTIONS and the user requirement to align real protocol capabilities while keeping server and standalone native client independently usable. This is a next-task specification, not a completion claim. Apply DDIA's reliability, bounded state and additive schema evolution principles.

## First vertical slice

Implement real SSE and WebSocket sessions using mature Rust libraries (reqwest streaming + eventsource-stream; tokio-tungstenite with rustls). HTTP request execution remains backward compatible. Additional protocols (GraphQL, gRPC, Socket.IO, MQTT, SOAP, MCP/A2A) remain individually pending until their real engines/editors/tests exist. Do not add fake connected states or placeholder success messages.

Add optional serde(default) protocol configuration to saved RequestSpec, with a typed tagged SSE/WebSocket variant; omitted configuration means existing HTTP. TypeScript must use the same discriminated union. Reuse URL, headers, auth, TLS and variable handling. Keep canonical imported specs unchanged. Format export must explicitly warn about formats incapable of representing the new protocol.

## Public API and native transport

POST /api/sessions {workspace_id,request,environment_id?,locals?} creates an owner-bound live session and returns its ID plus initial state. Request validates workspace ownership and membership of the supplied request ID before starting. SSE accepts HTTP(S), WebSocket WS(S); reject incompatible request body/method configurations with concrete errors.

GET /api/sessions/:id returns state, protocol, counters and creation/update timestamps. GET /api/sessions/:id/events?after=N returns a bounded ordered event batch, next cursor, earliest retained cursor and explicit dropped count if the client's cursor precedes retention. No ordering ambiguity or silent event loss. IDs are random and ownership is checked on every operation; knowing a session ID grants no access.

POST /api/sessions/:id/send accepts a typed WebSocket text/binary/ping message; preserve binary bytes as Base64 and cap decoded size before allocation. SSE is receive-only. POST /api/sessions/:id/close requests cancellation and waits for the worker to terminate. DELETE /api/sessions/:id closes and removes retained events. Tauri forwards these JSON operations into the same in-process router, with no local TCP listener. Browser/native UI can poll only while the relevant session is open; stop timers on unmount/logout/account change.

Session state machine: connecting -> open -> closed or error. Capture WebSocket handshake status/safe headers and close code/reason. SSE preserves event type, data, id and retry fields as delivered by the mature parser, with received timestamp and cursor. WebSocket logs incoming/outgoing text/binary/ping/pong/close events and direction. Preserve protocol events instead of pretending they are finite Response.body strings.

## Limits and cleanup

Volatile session state is not synchronized or persisted in workspace/history. Each session retains at most256 events and8MiB total event bytes; evict oldest with monotonic cursor tracking. Each message/event <=1MiB, session input <=20MiB, bounded command queue32. Per owner <=4 live sessions and16 retained sessions; global live limit64. Sessions expire after30min, terminal records after10min, and all workers cancel on server shutdown. Closing/deleting a workspace must cancel its sessions. Limits are explicit errors, not blocking queues with unbounded pending allocations.

Initial SSE stream has an8MiB total wire-byte ceiling to bound the mature parser's pending-event buffer even when a hostile stream never terminates an event; report termination reason accurately. Never start arbitrary reconnect loops. Reconnect/history/replay options are separate later capabilities. Event retention is lossy with an explicit gap marker; it is not a durable delivery guarantee.

Network policy is the existing hosted-private-network restriction/native opt-in. Extract a shared checked destination resolver if needed; check/pin all resolved addresses, protect TLS hostname verification, restrict redirects and strip cross-origin credentials. Never regress HTTP SSRF tests. WebSocket handshakes must use checked/pinned sockets instead of an unchecked connect_async URL DNS lookup. No browser-direct requests bypassing server policy.

Local/secret variables are used only in live execution; session summaries and errors redact known private values, including encoded variants. Logs remain account-bound. No tokens in global logs or endpoint URLs displayed outside the current session. Request pre scripts run before connect using the isolated script worker; existing post-response scripts are explicitly unavailable for live protocols until a typed per-event script contract is implemented. Warn/reject rather than silently ignoring a saved post script.

## Frontend

Feature module features/protocols with typed session API/controller and protocol-specific controls. Use existing Radix, TanStack Query, CodeMirror and Lucide; use an established accessible selection component for protocol choice. Connect/Close, visible state/reason, ordered event log, WebSocket send editor and text/Base64 choice. SSE fields inspectable without collapsing data into a fake response. Keyboard entry, focus and reduced motion remain consistent with existing UI. No animation on repeated event arrivals or send shortcuts; static state changes aid frequent use.

Retain the current HTTP response/test view for HTTP requests. New protocol configuration participates in normal save/CAS/local draft protection. Account/workspace switch cannot display old events or apply old local mutations. Browser local values are only sent to the selected session execution, not persisted as shared settings.

## Verification gate

Real loopback SSE fixture with split chunks/multi-line data/id/retry and large unterminated event; real WebSocket echo text/binary/ping/close fixture; cancel while connecting/reading; event ring gap/order tests; private network blocked hosted and allowed native; authenticated owner isolation; cleanup after workspace deletion; backward JSON import/save tests; no-default native library build. Browser tests perform actual connect/send/receive/close and test narrow layouts/keyboard/a11y. CI validates packaging and three existing databases after the protocol change. Only then mark the specific SSE/WebSocket targets implemented.
