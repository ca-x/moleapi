# Socket.IO backend functional review

Review base: `0a64ae6`; frozen uncommitted backend Socket.IO slice in the application worktree. Fresh read-only functional review of normal workflows and execution-scoped data; source was not modified. Root owns UI/interchange and browser QA. No adversarial/probe, Docker, packaging, commit or push work.

## Finding (resolved after independent recheck)

### P2 / Medium — Valid application event names were incorrectly reserved

Source: `crates/core/src/validation.rs:297–299` in `validate_socketio_event`.

Reproduction: configure initial listeners `["error"]` on an otherwise valid Socket.IO request; connection preparation rejects it with `Reserved Socket.IO event name`. An open session likewise rejects `socketio_listen` or `socketio_emit` for `error`, `open`, or `close` (`crates/protocols/src/socketio.rs`, calls to this validator in `send`). These names are legal Socket.IO application events, including the common server `error` notification, so users cannot observe or send them.

Evidence from this review:

- A temporary Rust executable linked against the current compiled `moleapi-core` called the actual validator: `error`, `open`, and `close` each return `Err(Reserved Socket.IO event name)`; `application:error` returns `Ok(())`.
- The locked official Node Socket.IO 4.8.1 server accepted `io.of('/').emit(name, {message:'ordinary application event'})` for all three names. Its `dist/socket-types.js` lists exactly `connect`, `connect_error`, `disconnect`, `disconnecting`, `newListener`, and `removeListener` in `RESERVED_EVENTS`.
- The existing `draft_defaults_and_execution_configuration_bounds` core regression incorrectly expects `error` rejection, so a green suite currently preserves the incompatibility.

The upstream convenience `Event` conversion has `open`/`close`/`error` aliases, but this adapter sends positional packets through `send_arguments(Some(String), ...)` and consumes parsed packet event strings. No convenience callbacks are registered; those aliases do not require rejecting these names in this integration. Fix the validator to reserve the actual Socket.IO names and add a normal official-server echo/listener regression for the three legal names.

## Reviewed and verified

- Namespace/custom Engine.IO path, structural auth interpolation, header/query merging and SDK-owned EIO/transport/sid validation. The original URL supplies Host/SNI while `checked_destination` and direct TCP use pinned approved addresses; transport framing/destination headers are removed by `request_headers`. Verification remains explicit and TLS is handled by mature tungstenite/native TLS.
- The selected execution environment and pre-script changes are retained for manual emit/listener/server-ACK commands. Saved outbound malformed drafts are excluded from connection interpolation, and auth edits use the body hook then reconcile into the execution clone. Creation feedback filters local/private updates. Live post-response scripts produce an explicit unsupported-phase error.
- Mixed positional JSON and nested/multiple binary placeholders in events and both ACK directions; empty and all-256-byte binary attachments; 900 KB binary ACK; ordered binary sends; application-error ACK arguments. The SDK owns parsing, packet serialization, namespace packet observation and ACK ID routing.
- ACK enrolment before send, monotonic SDK IDs, oneshot and drop cleanup, duplicate user correlation rejection, bounded outgoing/incoming callback tables, opaque owner/session-bound server callback tokens, expiry and exactly-once reply. Cancellation prioritizes Stop and aborts/drains waiter tasks; no automatic reconnect/poll task is started through `connect_with_engineio`.
- Listener filtering and changes, server namespace disconnect, pending handshake cancellation/deadline, terminal send rejection, owner checks and shared logout/admission/deletion infrastructure. Existing Node-dependent server ownership/logout tests were read; this reviewer did not run their hardcoded-port cases against someone else's fixture.
- Event names, argument strings/numbers/keys, binary payloads, auth values, target and ACK errors pass through the captured session mask. Bounds include 1 MiB payload, 20 MiB input/receive and injected transport wire accounting, 32 command/ACK/attachment limits, 64 listeners, 256 arguments, 10,000 emits, existing 256-event/8 MiB retention and session/lifetime quotas.
- Both vendored crates identify published 0.6.0/upstream revision `3434b654c18580785c0d0171bc1acbc0378580c7`, retain original manifests/VCS provenance and MIT license, and document connector/observer/ACK/binary patches. Injection creates an already-connected Engine.IO transport and caller-driven Socket.IO stream; no unchecked reconnect/resolver/fallback branch is called. Capability is explicitly EIO4 / Socket.IO wire5 / official server4 / WebSocket-only, without claimed legacy versions, polling or reconnect.

## Commands executed in this review

Own official fixture: `NODE_PATH=/tmp/moleapi-socketio-fixture/node_modules PORT=18887 node server.cjs` (reviewer-owned process only).

- `NODE_PATH=/tmp/moleapi-socketio-fixture/node_modules MOLEAPI_SOCKETIO_FIXTURE_URL=ws://127.0.0.1:18887 cargo test -p moleapi-protocols --lib --test socketio -- --include-ignored`: **4 library + 5 integration tests passed**, including official TLS/SNI/certificate test which starts its own ephemeral fixture.
- `cargo test -p moleapi-core --test socketio`: **2 passed**.
- `cargo test -p moleapi-server --no-default-features --test socketio saved_socketio`: **1 passed**.
- Official Node accepted-name check and temporary compiled Rust validator reproduction: **incompatibility confirmed**.

The initial protocol run failed because the review fixture was not yet available and NODE_PATH was absent; the corrected run above passed. This was reviewer fixture setup, not a backend failure. Full workspace/Clippy/fmt results in the implementer report were read, not independently rerun here. Reported passing tests do not dismiss the supported-name finding.

## Completion ledger

1. Backend source/SDK/normal-workflow and scoped-data review: done; one confirmed functional finding.
2. Focused ordinary regressions using own fixture: done (12 tests passed).
3. Source repair and independent post-fix regression: done; finding resolved; no implementation edits by reviewer.
4. Browser/UI/interchange and final builds/shipping: root-owned; outside this review.

## Independent post-fix recheck

Status: **resolved; zero open backend findings in the reviewed scope**. This recheck inspected the final validator and corrected core tests, official fixture changes, new ordinary-name integration case, and implementer red/green evidence. The validator retains exactly the six official reserved event names while accepting `error`, `open`, and `close`. Input name/size validation remains active. No SDK transport, protocol parser, ACK routing or lifecycle behavior was changed by this fix.

Independent current-source checks:

- Reviewer-owned updated official fixture on port 18887, with the same namespace/path/auth/header/query contract. No existing fixture/service was stopped or changed.
- `NODE_PATH=/tmp/moleapi-socketio-fixture/node_modules MOLEAPI_SOCKETIO_FIXTURE_URL=ws://127.0.0.1:18887 cargo test -p moleapi-protocols --lib --test socketio -- --include-ignored`: **4 library + 6 integration tests passed**. The new ordinary-name test confirms initial listeners, live listener command acceptance, emits, matching incoming JSON, successful outgoing ACK arguments and an open session after each of `error`, `open`, and `close`. Existing listener disable/enable, binary/ACK/cancellation and TLS/SNI/certificate cases also passed unchanged.
- `cargo test -p moleapi-core --test socketio`: **2 passed**; checks all six actual reserved names still reject, and all three legal application names accept.
- `git diff --check`: pass.

The implementer pre-fix log `/tmp/socketio-reserved-red.log` shows the corrected core acceptance assertion failed on `error` before the validator fix. The new integration test uses actual official-server echoes and checks session state, so it verifies behavior through the mature SDK rather than mirroring validator implementation alone. The independently run full focused protocol suite, including TLS, passed on the first configured post-fix reviewer run. The implementer's earlier transient TLS error is recorded in their report; it did not reproduce here.

Recheck completed with **12 passing tests**, no implementation source modifications, no commit/push, and reviewer fixture shutdown. UI/browser/export and final integration gates remain root-owned.
