# MQTT frontend and native interchange review

Reviewed the uncommitted MQTT frontend/interchange implementation against `docs/specs/mqtt-client.md` and `docs/superpowers/plans/2026-10-03-mqtt-client.md`, with baseline `03224ef`. Ordinary functional review; no adversarial probes, implementation edits, commits, or changes to shared browser/accounts/workspaces/brokers.

## Findings

One P2 finding was identified and fixed by the root implementer while this review was running. No additional actionable defect was established in the reviewed frontend/interchange paths.

| Before | After | Why |
| --- | --- | --- |
| **P2: generated saved-message names retained private topics.** Save topic `customer/acme` with no explicit name; mark its Topic private; update the selected saved message. `saveMessage` retained `messageName`/`existing.name = customer/acme`. Default native export cleared the message topic but retained `entry.name`, leaking the exact marked-private value. | Current `MqttWorkbench.tsx` replaces the private topic inside the selected name with `[私密 Topic]` before saving. Current native redactor screens names for marked-private topic/payload before clearing the message fields. | Privacy markers must cover labels derived from the same draft value; users should be able to mark an existing saved topic private without manually discovering and renaming an earlier generated label. |

Reproduction evidence: source inspection of the original `saveMessage` name fallback and native MQTT saved-message loop. An isolated temporary React/jsdom harness then exercised Save new → mark Topic private → Update selected against the implementer's fix. It passed, asserting the saved topic remained marked private and the generated name became `[私密 Topic]`, containing no original topic. The harness mocked nested workbench panes and context, used the real root saved-message handler and Radix Theme, and was removed after verification. Initial harness attempts needed Theme/ResizeObserver setup; those failures were harness setup issues.

## Reviewed behavior

- Canonical text/JSON/base64 payload sources and ordered duplicate user-properties are edited without frontend JSON parsing or lossy payload coercion. Incoming/outgoing details use typed event JSON, including SDK packet/status fields.
- Saved-message load/save/update/delete clone message data; private topic fallback prevents a newly generated label from deriving from an already-private topic. Existing/generated labels are now screened when updating and exporting.
- Subscription mutations wait for accepted send responses, merge into the latest canonical request, disable competing operations, and guard pending callbacks by request/account/workspace/environment identity and mounted state. Shared session sends additionally use generation/session guards.
- Connection options and Will edits are disabled while connecting/open. Graceful close and explicitly guarded abort are separate actions; the UI explains the broker's Will behavior.
- Version controls disable v5-only fields. Incompatible v3 drafts show an explicit clearing action covering connection/message/saved-message/Will/subscription properties.
- Telemetry rejects outgoing, private, malformed and nonnumeric messages; bounded traversal uses distinct JSON path keys, finite timestamps/numbers, 256 retained events, depth 8 and 512 visited nodes. UI exposes up to 64 selectable fields with four simultaneous chart series and no chart animation. Shared event retention also enforces 8 MiB.
- Components reuse labeled shared fields/choices, keyboard-capable Radix controls, list/button semantics, read-only event editors, horizontally scrollable MQTT tabs and narrow-layout wrapping rules. Actual narrow browser/a11y assessment remains the root's live QA responsibility.
- Native interchange separates schema-preserving source traversal from credential-bearing payload traversal; explicit exports preserve the canonical MQTT draft and foreign formats reject unrepresentable MQTT data.

## Validation and limits

- Independently ran frontend `npm test -- --reporter=dot`: **15 files / 62 tests passed**.
- Independently ran frontend `npm run typecheck`: **passed**.
- Temporary saved-label React interaction harness: **1 test passed** after the root fix; temporary file removed.
- Observed `/tmp/moleapi-mqtt-formats-tests.log`: **15 native interchange tests passed**, including canonical MQTT private/explicit export coverage. This log was produced by the root's separate validation, not by this reviewer.
- This review did not claim an independent browser/broker replay, build result, mature-broker fixture coverage, Node fixture equivalence, or native GUI validation. Root's actual browser/broker QA is separate evidence. Native GUI remains untested, and final full-feature integration to main remains pending the broader capability work.
