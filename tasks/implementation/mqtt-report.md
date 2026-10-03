# MQTT Rust implementation report

Base: `03224ef`; shared worktree `/home/czyt/code/rust/moleapi/.worktrees/application`. Source is ready for fresh independent backend review. No commit/push/main checkout, packaging, Docker, workflow, system-service or desktop changes were made by this worker. Root owns frontend, formats, export and browser acceptance.

## Implementation

- Core canonical `Protocol::Mqtt` flattens a boxed `MqttConfig`: version, client ID, clean start, keep-alive seconds, session expiry seconds, bounded reconnect, subscriptions, connection user properties, Will/message/properties/delay, composer and named saved messages. Defaults and exact structs are in `crates/core/src/mqtt.rs`.
- Incomplete composer/saved-message/Will drafts can be saved. Execution validates connection settings, enabled initial subscriptions and Will. Composer/saved messages never auto-publish or resolve during connection creation. Publish commands validate and resolve at send time against the original selected execution scopes. JSON source bytes and large numeric lexemes are preserved; base64 decodes exact arbitrary bytes, including zero-length retained clear.
- MQTT 3.1.1 and 5 over `mqtt`, `mqtts`, `ws`, `wss` use pinned rumqttc 0.25.1. The SDK owns MQTT packets, identifiers, heartbeat and QoS handshakes. Anonymous and Basic username/password are MQTT CONNECT authentication; unsupported Bearer is an explicit execution error.
- Public SDK `set_request_modifier` supports common enabled headers and query fields on ws/wss. Common auth is cleared only in the HTTP-handshake header builder, so MQTT Basic never invents HTTP Authorization. Explicit Authorization headers remain available. SDK-owned handshake headers reject overrides. TCP/TLS reject HTTP headers/query/path. DNS policy applies to every pinned destination and every bounded reconnection; original TLS hostname and WS Host remain intact.
- Real publish/subscribe/unsubscribe, wildcard filters, QoS 0/1/2, retained replay/zero-byte clear, initial/restored subscriptions, MQTT 5 publish/subscribe/connection user properties, subscription identifiers, Will properties/delay and connection settings are implemented. Received payloads retain topic, raw base64, optional exact UTF-8, QoS, retain/duplicate, packet ID and SDK properties. Status events distinguish queued/sent/acknowledged/rejected/handshake/discarded; QoS 0 has no fabricated acknowledgement.
- Existing owner/workspace/request generation, admission, quotas, event retention and cancellation infrastructure is reused. Queued subscription intents reserve capacity immediately to prevent bursts exceeding 64; the SDK acknowledgement indicates actual broker acceptance. Reconnect is capped at 10 configured attempts for the entire session, with cancellable delay and fresh vetted addresses. MQTT 5 keeps a stable execution-generated ID across reconnect and disables clean start after initial connection when session expiry is positive. New persistent sessions require an explicit client ID. When the broker reports no persistent session, discarded SDK pending publishes are reported and subscriptions are restored.
- Normal Stop tries SDK DISCONNECT for up to 500 ms and reports whether graceful disconnect completed. `mqtt_abort` deliberately drops transport without DISCONNECT; Will semantics therefore differ. Disconnect packets/reasons/properties buffered by the SDK before an error are preserved.
- MQTT pre-scripts may alter URL/auth and variables. HTTP body/header/query/method and protocol-draft mutations are rejected explicitly. Request/collection/project post/event phases are explicitly unsupported. Scope-derived credentials/client IDs/labels are never automatically applied to persisted drafts through feedback.
- Known private values use existing server redaction plus execution-only MQTT secret capture. Topic/payload/property flags are reported. Private arbitrary binary substrings are withheld rather than corrupting base64; `payload_text` is absent and `payload_base64` empty when withheld. Explicit secret filters conservatively withhold all received topic labels in that session. Oversized secret metadata fails closed by withholding content. No resolved secret labels enter shared drafts.

## SDK extension and licensing

`vendor/rumqttc` is the official 0.25.1 crate, with Apache-2.0 license text retrieved from the official Bytebeam repository (the crate archive omitted LICENSE), original manifest/source retained and changes documented in `MOLEAPI-PATCH.md`:

1. `NetworkOptions::set_pinned_addresses` bypasses SDK DNS using previously vetted addresses; empty pin vectors fail closed. Both versions and TCP/TLS/WS/WSS use the same socket connector while retaining the original hostname.
2. Shared `TrafficBudget` at the SDK Network read/write boundary counts encoded MQTT packet sizes before state mutation, including CONNECT, ACK and PING. Per-direction 20 MiB and total 10000-packet ceilings persist across reconnect. Byte counts exclude TLS/TCP/WebSocket overhead and count SDK-buffered outgoing packets, which can include an unflushed final buffer. No custom packet parser is introduced.
3. One-line upstream fix decrements the outgoing inflight slot on a negative MQTT 5 PUBREC. Upstream took the outgoing publish but returned before decrementing, permanently filling the window after 16 rejected QoS 2 publishes. The real broker regression deliberately failed with the line removed (exit 101/timeout), then passed with the patch restored: 17 denied QoS 2 exchanges followed by an accepted QoS 2 publish. SDK enums have ordinal Rust discriminants, so adapter outcomes match SDK success variants rather than casting them into MQTT reason bytes.

## Limits and exact API

Common session routes remain `/api/sessions`, `/{id}/send`, `/{id}/events`, Stop/delete. Session summary `protocol` is `mqtt`.

Commands:

- `mqtt_publish { message: MqttMessage }`
- `mqtt_subscribe { subscription: MqttSubscription }`
- `mqtt_unsubscribe { filter: string }`
- `mqtt_abort`

Events:

- `mqtt_message { topic, payload_base64, payload_text, qos, retain, duplicate, packet_id, properties, topic_redacted, payload_redacted, properties_redacted }`
- `mqtt_status { operation, status, packet_id, reason_codes: string[], details }`

Limits: 1 MiB decoded message; 1024-byte topics/filters; 32 properties/16 KiB outbound property strings; 32 KiB received property envelope; 64 subscriptions and saved messages; 5 MiB total saved MQTT draft; SDK request channel 32, application channel 32, SDK inflight 16, at most 48 pending/inflight publishes in adapter; 10000 application input messages and 10000 SDK packet operations; existing 256 events/8 MiB retention, 20 MiB input, owner 4/global 64 live sessions, 30-minute lifetime and 10-minute terminal retention. Keep-alive uniformly 5–3600 seconds (rumqttc v5 API requires at least 5). Topic aliases are not advertised as an editable feature (SDK default incoming alias maximum is zero). Connection/property expiration and Will delay units are seconds; reconnect delay is milliseconds.

## Verification evidence

Final source verification:

- `cargo test --workspace --exclude moleapi-desktop --no-default-features`: exit 0; 39 suites, 151 passed, 16 ignored external fixture tests. Log `/tmp/moleapi-mqtt-final-workspace.log`.
- `cargo clippy --workspace --exclude moleapi-desktop --all-targets --no-default-features -- -D warnings`: exit 0.
- `cargo fmt --all -- --check` and `git diff --check`: exit 0.
- `MOLEAPI_MOSQUITTO_BIN=/tmp/moleapi-mqtt-broker/usr/bin/mosquitto LD_LIBRARY_PATH=/tmp/moleapi-mqtt-broker/usr/lib cargo test -p moleapi-protocols --test mqtt -- --ignored --test-threads=1`: 8 passed, exit 0. Log `/tmp/moleapi-mqtt-final-broker.log`.
- `MOLEAPI_MQTT_FIXTURE_URL=mqtt://127.0.0.1:18891 cargo test -p moleapi-server --test mqtt --no-default-features -- --ignored --test-threads=1`: 2 passed, exit 0.
- Core MQTT 6 tests and protocol modifier/subscription reservation unit tests pass in workspace suite. Server 2 default tests cover incomplete canonical drafts/execution error and unsupported MQTT script mutations/phases.

Real Mosquitto tests prove both versions/QoS 0/1/2/pubacks/subacks/unsub/wildcards, arbitrary binary/JSON source, retain/replay/clear, MQTT 5 properties/subscription IDs, delayed Will on abort versus graceful close, explicit topic/property/private binary withholding, auth rejection, self-signed TLS verification rejection and opt-out, public-network policy rejection, broker loss/restoration/bounded attempts/Stop, unsolicited actual >20 MiB reception terminal quota, owner isolation, and server original scope/local/private credential-template preservation/request removal/logout/old-token fencing. Positive trusted TLS and negative wrong-original-host tests prove pinning does not rewrite certificate hostname; a deliberately unresolvable hostname works with a vetted pin in the direct SDK test. Public WS modifier test asserts actual custom header/query construction and absence of invented HTTP Authorization from MQTT Basic. Real WS and WSS broker tests exercise both versions with Basic CONNECT authentication.

One intermediate broad run hit a preexisting script worker's 2-second flood-output/deadline assertion during build-cache garbage collection. Its unchanged isolated rerun passed, and the final complete workspace rerun above passed. No unrelated worker-deadline change was made.

## Fixture processes for root browser

Isolated Mosquitto 2.1.2-2 and cjson 1.7.19-1 Arch packages were downloaded/extracted under `/tmp/moleapi-mqtt-broker`; nothing installed. Focused tests spawn/drop temporary broker processes on ephemeral loopback ports above 18891. See `crates/protocols/tests/fixtures/mqtt/README.md`.

Root browser fixtures remain running intentionally:

- Session 85871: `mqtt://127.0.0.1:18891`, anonymous, `/tmp/moleapi-mqtt-broker/browser.conf`.
- Session 67805: `/tmp/moleapi-mqtt-broker/browser-extra.conf`: `mqtts://localhost:18892` anonymous/self-signed; `mqtt://127.0.0.1:18893` Basic; `ws://127.0.0.1:18894/mqtt` Basic; `wss://localhost:18895/mqtt` Basic/self-signed. Credentials are fixture-user / fixture-password; self-signed transports require explicit verification opt-out. Certificates/password file live only under the extracted temporary directory.

Other preexisting API/browser/grpc/Socket.IO fixtures and root port 18877 were not touched. Broker processes can be terminated after root acceptance.

## Remaining root work / stated capabilities

Fresh backend review and fixes, frontend/formats/browser acceptance, commit/proxy push are root-owned. Native desktop GUI launch/distribution was not exercised. Advanced common TLS/client certificate UI remains a separate shared module. Event scripts/response extraction/business-level delivery/exactly-once application processing are not claimed. Reconnection follows actual SDK session state; it cannot recover application publishes the SDK discards when the broker returns no session. No broker redirection/server-reference URL is followed implicitly.


## Fresh backend review fix — binary correlation privacy (P2)

Independent review found that known private bytes embedded inside MQTT 5 `correlation_data_base64` remained recoverable because the property only received string redaction after encoding. The actual server/Mosquitto regression first failed (exit 101): outgoing NUL + private-secret + NUL correlation bytes remained encoded with `properties_redacted=false`.

The message payload and binary correlation property now share the same decoded-byte privacy predicate: existing known-private text redaction, explicit arbitrary-byte private substrings and fail-closed privacy limits. A private correlation property is withheld as JSON null and `properties_redacted=true`; its other properties and public payload remain intact. Public correlation bytes remain exact. Will correlation properties enter the same broker Publish event path and use the same guard. No canonical draft/schema changes, UI/formats edits, SDK changes or fixture-port mutations were needed.

Sibling inspection confirmed CONNACK authentication data was already omitted from emitted properties. Unexpected SDK packet/debug diagnostics could render authentication data or other binary data as byte arrays, so unexpected packet events now contain only a type and a withholding notice. `NotConnAck` errors retain a meaningful protocol error without raw packet Debug output. SDK client queue-error Display is already static and does not serialize its contained request.

Added `actual_server_redactor_withholds_decoded_correlation_secrets_and_preserves_public_bytes` uses the production server Redactor and mature Mosquitto: outgoing/incoming NUL-prefixed/suffixed private text, unaligned non-UTF8 prefixes/suffixes, an explicitly private arbitrary binary payload subsequently embedded in correlation, exact public binary correlation, and ungraceful broker Will correlation. RED before the fix, GREEN after. A typed SDK unit test covers unexpected binary authentication diagnostics and both versions' NotConnAck errors.

Fresh verification:

- `cargo test -p moleapi-protocols unexpected_sdk_packet_diagnostics_withhold_binary_authentication_data --no-default-features`: passed.
- `MOLEAPI_MQTT_FIXTURE_URL=mqtt://127.0.0.1:18891 cargo test -p moleapi-server --test mqtt --no-default-features -- --include-ignored --test-threads=1`: 5 passed.
- Full isolated mature Mosquitto MQTT fixture rerun: 8 passed.
- Strict workspace all-target Clippy, workspace fmt check and diff check: exit 0.
- Final complete workspace test rerun logged at `/tmp/moleapi-mqtt-correlation-fix-workspace.log` (result recorded below).

Source is frozen again for independent recheck. No commit or push.

Complete workspace rerun: 39 suites, 154 passed, 17 ignored; exit 0.
