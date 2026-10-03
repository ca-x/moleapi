# MQTT backend independent functional review

Status: review complete against HEAD/base `03224efc6d923cc73464223b9dcc2aa39db0194d` and the frozen uncommitted MQTT backend implementation, including its privacy fix; the one confirmed medium finding is resolved and no open findings remain within this ordinary functional review scope. No implementation edits, commits, pushes, Docker operations or system-service changes were made. This is an ordinary functional review, not an adversarial or penetration test.

## Historical confirmed finding — resolved after independent recheck

**MEDIUM (resolved) — binary MQTT 5 correlation data could expose known-private values in message history before the fix.** Location: `crates/protocols/src/mqtt.rs:561` and `:593` (also outgoing message properties at `:636`). The adapter converts SDK correlation bytes into a base64 string before generic property scrubbing. Generic scrubbing sees the encoded text, never the decoded bytes. Unlike message payloads (`:548–560`), correlation properties have no raw-byte/privacy check. This affects both incoming echoed properties and outgoing history, and `properties_redacted` stays false.

Reproduction, using a normal local Mosquitto 2.1.2 MQTT 5 connection and subscription:

1. Configure the exact current server `privacy::Redactor` with known-private value `private-secret`.
2. Publish a regular message with `correlation_data_base64 = "AHByaXZhdGUtc2VjcmV0AA=="` (bytes NUL + `private-secret` + NUL).
3. Read the outgoing/incoming MQTT event. Both retain the base64 property unchanged; the incoming event reports `properties_redacted = false`. Decoding recovers the private value.

The existing server guard registers raw, standard-base64, URL-safe-base64, JSON-escaped and URL-escaped versions of the private string (`crates/server/src/privacy.rs:33–52`). Those exact strings do not occur in this encoded binary envelope because base64 depends on byte alignment. The independent harness imports the actual current server privacy implementation, so this is not a weak mock-mask inference. Existing property tests use a plaintext secret user-property and a nonsecret two-byte correlation value; they do not cover this composition.

Suggested correction: examine decoded correlation bytes with the same known-private and explicitly private binary checks used for payloads, withhold the binary property when a match occurs, and set `properties_redacted`. Cover outgoing/incoming correlation data, Will correlation data, and embedded private bytes with a broker regression. Do not redact substrings inside base64 and thereby corrupt the binary value.

Evidence: `/tmp/moleapi-mqtt-backend-review/src/lib.rs`, test `review_private_binary_correlation_data_is_withheld`. It failed twice (exit 101), including once with exact server Redactor imported read-only. Error: `known-private binary correlation remained recoverable: {"correlation_data_base64":"AHByaXZhdGUtc2VjcmV0AA==", ...}`. No real credentials were used.

## Verification performed in this review

- `MOLEAPI_MOSQUITTO_BIN=/tmp/moleapi-mqtt-broker/usr/bin/mosquitto LD_LIBRARY_PATH=/tmp/moleapi-mqtt-broker/usr/lib cargo test -p moleapi-protocols --test mqtt -- --ignored --test-threads=1`: exit 0, **8 passed**. Each test spawned/dropped an isolated mature broker on an ephemeral loopback port.
- `MOLEAPI_MQTT_FIXTURE_URL=mqtt://127.0.0.1:18896 cargo test -p moleapi-server --test mqtt --no-default-features -- --include-ignored --test-threads=1`: exit 0, **4 passed**. An exclusively owned temporary Mosquitto fixture ran on 18896 and was terminated after the command. Existing root/implementer brokers and API/browser fixtures were untouched.
- `cargo test -p moleapi-core mqtt --no-default-features`: exit 0, **6 MQTT tests passed**.
- `cargo test -p moleapi-protocols mqtt --lib`: exit 0, **2 MQTT tests passed**.
- Extra independent `/tmp` harness `review_persistent_session_retains_subscription_and_offline_delivery`: **passed**. A stable-ID MQTT 5 client subscribed, aborted with a positive expiry, received a QoS 1 publication while offline, reconnected with clean start false, observed `session_present = true`, and received the offline message without supplying initial subscriptions. This adds actual persistent-session evidence beyond the existing clean-session broker restart test.
- Same `/tmp` harness privacy regression: **failed as described above**, exit 101. To rerun: `MOLEAPI_MOSQUITTO_BIN=/tmp/moleapi-mqtt-broker/usr/bin/mosquitto LD_LIBRARY_PATH=/tmp/moleapi-mqtt-broker/usr/lib CARGO_TARGET_DIR=/home/czyt/code/rust/moleapi/.worktrees/application/target cargo test --offline --manifest-path /tmp/moleapi-mqtt-backend-review/Cargo.toml review_ -- --test-threads=1`.

## Covered source and ordinary behavior

Read canonical MQTT configuration/message/subscription/property/Will models and validation; interpolation/draft handling; SDK transport/options/pinning and event-loop adaptation; owner/session registration/configuration/send/cancel paths; script restriction and execution-scope handling; privacy capture/redaction; MQTT SDK patches and provenance; broker/server tests and contract/report.

Passing actual fixtures cover MQTT 3.1.1/5, QoS 0/1/2, actual SDK packet IDs and acknowledgements, wildcard subscription/unsubscription, exact binary and JSON source, retained replay and zero-byte clear, publish/subscription properties, subscription IDs, delayed Will after abort versus graceful Stop, Basic CONNECT auth/rejection, TCP/TLS/WS/WSS, custom handshake header/query handling without HTTP Basic synthesized from CONNECT auth, vetted pins and original TLS hostname, clean-session reconnect/subscription restoration and bounded retry cancellation, owner isolation, original execution scopes and credentials remaining templates in drafts, request deletion/logout/old-token fencing, input/subscription/packet/traffic quotas, and actual negative QoS 1/2 publish acknowledgements including 17 denied QoS 2 operations followed by successful delivery. Source confirms flags/known-private text and payload-byte handling, stable client IDs for persistent reconnect, and SDK-owned handshakes. No further concrete functional findings were demonstrated.

## Limits and completion ledger

1. Backend/source ordinary functional review: done; the one medium privacy finding is resolved after independent source/test recheck; no open findings in scope.
2. Fresh focused mature-broker/core/server checks: done; green checks and failing regression are distinguished above.
3. Backend fix: implementer-owned and independently verified below; no source changed by this reviewer.
4. Whole-workspace strict Clippy/fmt/full suite: not rerun here; implementation report provides previous evidence, which this report does not restate as fresh verification.
5. UI/export/browser/native GUI/distribution/main integration: outside this delegated review, root-owned. Advanced common TLS/client certificates and event scripting remain explicitly deferred as in the contract.

The `/tmp` harness uses the existing mature broker/test helpers and public SessionManager API and references current source paths. It is a review artifact, not a committed application test. Shared checkout frontend/formats changes were ongoing, so this review makes no whole-workspace release claim.

## Independent privacy fix recheck

The backend was frozen again after the implementer addressed the finding. Read the actual correction in `crates/protocols/src/mqtt.rs:512–550` and the shared message path at `:596–602`. `private_bytes` now applies known-private text/encoded masking, explicit private byte substring checks and privacy-limit withholding to decoded bytes. `scrub_binary_properties` decodes correlation data before this check, emits JSON null on a private match, and contributes to `properties_redacted`. Public binary properties remain exact. Both outgoing and incoming events use this path; broker Will messages arrive through the same path.

Sibling checks: current CONNACK event properties omit binary authentication data; unexpected SDK packet events now emit a safe type/withholding notice rather than Packet Debug; `error_text` suppresses both SDK versions' `NotConnAck` packet Debug. The typed diagnostic unit test exercises these cases and passed. No new concrete functional issue was found in the correction.

Fresh independent checks after the fix:

- **Original unchanged independent regression now GREEN:** `/tmp/moleapi-mqtt-backend-review` command shown above: exit 0, 2 passed. This includes the exact production Redactor/mature broker correlation repro that previously failed and the persistent-session/offline-delivery test.
- **Server MQTT suite:** temporary exclusively owned Mosquitto on 18896, `cargo test -p moleapi-server --test mqtt --no-default-features -- --include-ignored --test-threads=1`: exit 0, 5 passed. New production-router regression checks outgoing/incoming known-private NUL envelopes, unaligned non-UTF8 prefixes/suffixes, an explicitly private arbitrary binary payload embedded in correlation, exact public binary correlation, and broker Will correlation. It asserts private correlation null, properties_redacted true, and unrelated public payload preserved. The broker was terminated afterward.
- **Typed SDK diagnostics:** `cargo test -p moleapi-protocols unexpected_sdk_packet_diagnostics_withhold_binary_authentication_data --no-default-features`: exit 0, 1 passed.
- **Original mature-broker integrations rerun:** `MOLEAPI_MOSQUITTO_BIN=/tmp/moleapi-mqtt-broker/usr/bin/mosquitto LD_LIBRARY_PATH=/tmp/moleapi-mqtt-broker/usr/lib cargo test -p moleapi-protocols --test mqtt -- --ignored --test-threads=1`: exit 0, 8 passed.

Verdict: the P2/medium finding is resolved with independently observed regression evidence. No open findings remain in the reviewed backend scope. Whole-workspace Clippy/fmt/full-suite results remain implementer evidence; they were not rerun by this reviewer. UI/browser/native GUI/distribution/main integration remain root-owned and are not covered by this verdict. Recheck changed only this report and built temporary review artifacts; no implementation source or shared fixture state changed.
