# Shared MQTT client implementation plan

> **For agentic workers:** Use executing-plans inline and verification-before-completion; one independent final source review.

**Goal:** Shared editors/admins configure and explicitly save MQTT requests, connect with canonical owner settings and publish/subscribe/inspect/Stop using session-local changes.
**Architecture:** Extend existing saved member session/command admission and action-only audit. Extract controlled MqttClient and subscriptions from owned context; reuse in shared authoring and isolated runtime config. Keep rumqttc networking/protocol state and existing member/session fences.
**Tech Stack:** Existing rumqttc, React/Radix/CodeMirror/Recharts; actual isolated Mosquitto2 broker.
**Spec:** docs/specs/mqtt-client.md and docs/specs/workspace-collaboration.md shared MQTT subsection.

## Constraints and interfaces

- No new DTO/dependency/parser; MqttConfig and typed mqtt_publish/subscribe/unsubscribe/abort remain canonical.
- Member Connect uses saved request_id/revision and owner resources, empty locals. Only typed MQTT commands; audit action/session without topic/payload/property/credentials. Existing creator/member/revision/credential limits apply.
- MqttClient consumes scope, RequestSpec, onChange, isCurrent, disabled, dark and optional MqttChannel(session/events/send/close). No channel means authoring only. Runtime hides connection/Will/network and saved-message mutation; saved-message loading, topic/message changes and subscriptions remain local.
- All retained editor/subscription/abort-confirm callbacks bind mounted/scope/current context and latest config; no owned hook/API in shared UI. Viewer controls absent. Dirty Save required before Connect; saving blocks commands while existing polling continues.
- Network/TLS/auth/pre-script controls reuse shared primitives; MQTT supports None/Basic/inherited auth only and hides HTTP query/header/body/assertion/redirect/method controls. Existing native installed/final release scope stays open.

## Task1: Saved member MQTT transport

- [x] Add offline policy test: existing member Connect returns400 before whitelist; viewer/foreign/stale/creator/wrong command denials.
- [x] Admit saved MQTT and four typed commands with action-only actor audit; package fmt and targeted test pass.
- [x] Real isolated authenticated Mosquitto both3.1.1/5: same-ID owner collision, original owner/env/pre-script, subscriptions, QoS/binary/retain, local add/remove, privacy, canonical source unchanged and role/revision/logout invalidation. No broker framing imitation.

## Task2: Controlled UI

- [x] RED shared create/config/explicit Save/local publish+subscription flow; extract MqttClient and MqttSubscriptions controls without owner context.
- [x] Shared authoring uses canonical edits; runtime config copies saved message/subscriptions and hides connection and saved-resource mutation; Abort requires explicit confirmation bound to current session.
- [x] Tests for retained callbacks/admission changes, pending subscription result, version guards, saved messages and owned compatibility. Run full frontend/typecheck.

## Task3: Delivery

- [x] Production, strict server Clippy/package format, embedded build and affected backend tests.
- [x] Independent source review; actual bilingual390 and1280 browser authoring/Save/connect/publish/subscriptions/telemetry/Stop/abort with unchanged canonical runtime source. Record native/file chooser limits.
- [ ] Update coverage/map/ledger; explicit stage/diff check, commit/push, preserve PR body and observe exact-head CI/Docker.

## Review focus

- Late subscribe result after changing role/request/credentials must not edit another draft.
- Abort confirmation retained across scope or session change cannot terminate a new session.
- Source authoring has no live network actions; runtime changes cannot enable workspace Save.
- MQTT3/5 property guards stay consistent across source, saved messages and live controls.
- Graceful close and Abort retain distinct real Broker Will behavior and shared admission.

## Ledger

- Previous goal turn: progress,769ed95 shared gRPC committed/pushed with700 frontend/59 backend and actual browser evidence. Current worktree clean; root user mise.toml untouched.
- Existing runs39488627465 Application and39488627230 Docker observed live; no timeout-based restart. Docker source/worker passed, both native images running.
- Preflight: backend typed commands already exist; controlled UI consumes same DTO. Existing subscriptions own hook must be removed before shared reuse. Runtime config must never call shared canonical onChange.

- Backend policy test RED400 before whitelist → GREEN. Real authenticated isolated Mosquitto2.1.2-2 (extracted official Arch package, no system install) runs both versions, original owner collision/auth/topic/pre-script, all QoS, binary/retained replay/zero-byte publish, local subscriptions, private withholding, graceful close versus Abort Will, actor audit, revision/logout/role fences. Subsequent v5 property/actor assertions are in the current final gate.
- Frontend shared flow initially lacked create; now explicit Save then local publish/subscription leaves canonical unchanged and uses only member routes. Controlled client/subscription tests cover retained scope/disposed callbacks, pending role/scope/saving results and retry, Abort confirmation admission/new-session fences, v3 property rejection and owned wrapper compatibility. Full frontend711/133 files passes; added owned-wrapper compatibility completes11 client tests, targeted29/9 files passes.
- Ruling: pre-script updates are known-private by existing VariableScopes::apply, so member event payloads containing them must remain withheld. An independent owner observer session without sender scripts verifies actual broker payload instead of weakening privacy; normal v5 telemetry payload stays visible. Test DTO expectation follows the existing public named key/value property representation.
- Independent shared_report_export_review found no concrete source issue; actual browser/native/full gates/long-run stability declined. Cover browser/source gates here and retain installed-native/platform/long-run scope as explicit remaining integration requirements.

- Final fixes: retained saved-message selection first put an old name into a new scope; admission guard/latest list fixes it, new regression RED→GREEN and complete frontend713/133 passes. Actual long event-row center hit adjacent editor because Radix scroll content exceeded its column; scoped width/360px bounds tested RED→GREEN in browser and reverified from final embedded assets, at390/1280.
- Final gates: affected default backend55 pass/5 configured ignored; shared MQTT policy+authenticated real broker2 explicit pass including v5 named properties and correct audit actor. Strict server Clippy, package fmt/diff, TypeScript, production39/no static cycles and final embedded build exit0. Full frontend713/133 passes. Existing collection initial-load test now waits for the canonical workspace name before Create; no product behavior changed for that timing issue.
- Final actual browser: empty shared collection authoring/explicit CAS Save; source reopen, owner-variable topics, local subscription and QoS2/JSON, named template load, received-event hit test/detail, telemetry, graceful close and confirmed Abort; exact canonical data/revision2 preserved. Final bilingual390 and1280 widths match; no browser errors. Final scoped a11y0 violations/1 incomplete contrast check; earlier properties group label check was also incomplete, not a passed accessibility claim.
- Final: minor (deferred): existing MQTT property group uses an aria-label without a semantic role; automated source/runtime audit can report incomplete ARIA attribution. No inaccessible control was proven by this run; retain for integrated accessibility pass.
- Native installed/platform/database matrix remain integration work. Delivery/exact-head CI/Docker pending commit/push; no full objective completion claim.
