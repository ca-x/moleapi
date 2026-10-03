# MQTT Client Implementation Plan

> Agentic workers: use subagent-driven-development, one fresh Rust implementation worker with independent reviews; root owns frontend/formats/docs. Scope is already authorized.

Goal: real broker-backed MQTT3.1.1/5 client with publishing/subscriptions/QoS/retained/Will/settings and dedicated UI.
Architecture: mature rumqttc SDK owns packets/state; checked connector/event loop reuse owner/session/scopes; canonical drafts through existing CAS; lazy React message/property/topic/chart controls.
Spec: docs/specs/mqtt-client.md.

## Constraints and review focus

Mature parsers/SDK/broker/controls only; no raw MQTT implementation. Features first, no localDocker or systembroker service changes. Preserve scopes/drafts/default credential export; checked DNS/TLS and reconnect no bypass; byte/owner/queue limits. Final delivery main after preserving originals and deleting only merged development branches.
Review ordinary data cases: QoS ack ordering/error codes; retained empty binary; graceful versus Will close; clean session/reconnect subscriptions; v3/v5 properties; repeated subscription edits after navigation; base64/private values; listener/transport loop cancellation and bounded unresolved callbacks.

## Task 1 — Rust and actual broker

- [ ] Verify SDK APIs/version/transport/property/Will/reconnect options; send exact canonical draft/commands/events to root early.
- [ ] Add config/interpolation/auth/pre-script/privacy/validation and checked pinned SDK connector/TLS settings; minimum documented/licensed SDK extension if needed.
- [ ] Add actual publish/subscribe/unsubscribe, SDK packet/status/events and broker-session handling; bounded reconnection/cleanup/owner lifecycle.
- [ ] Use mature isolated MQTT3/v5 broker fixture, actual QoS/retain/Will/properties/auth/TLS/reconnect/private/ownership/budget tests.
- [ ] Run targeted/workspace/strictClippy/fmt and write mqtt-report.md; stop source for fresh backend review.

## Task 2 — UI/formats and acceptance

- [ ] Root dedicated lazy MQTT workbench against early contract; persisted composer/savedmessages/topics/session/properties/Will and mature numeric telemetry charts.
- [ ] Native export/import exact data + defaultcredential redaction; foreign unsupported formats explicit. UI lifecycle/deferred-operation/scope/controls tests.
- [ ] Actual browser MQTT connect/sub/publish/retained/saved/telemetry/Stop/narrow/a11y and independent frontend review; fix findings and final feature verification.
- [ ] Commit/proxy push, continue remaining SOAP/MCP/A2A/TCP/Dubbo/Webhook/Data and shared scripts/extraction/testing/mock/collaboration matrix. Packaging/final main only after full-feature completion.
