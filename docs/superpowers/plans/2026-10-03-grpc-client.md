# gRPC Client Implementation Plan

> For agentic workers: use subagent-driven-development task-by-task, with one implementation worker and independent review. Root handles frontend/interchange. No additional user approval needed for the already-authorized feature scope.

Goal: usable proto/Reflection-backed gRPC client with all four invocation modes in standalone server and offline desktop.
Architecture: core typed canonical schema/config; mature dynamic protobuf codec and checked tonic connector; existing owner/session pipeline; modular dedicated React editor.
Tech Stack: tonic, prost, prost-reflect, protox, tonic-reflection; React/Radix/CodeMirror.
Spec: docs/specs/grpc-client.md.

## Constraints and review focus

Reuse mature parsing/wire/state-machine libraries. Preserve old workspace JSON. Keep source schemas portable. Existing privacy/admission/ownership/DNS pinning/deadline quotas apply. No local Docker. Complete functionality before refining builds. Final publication uses main and removes only merged extra branches.
Review: untrusted recursive schemas; streaming half-close vs cancel; reconnect/DNS/TLS bypass; status/trailer secrets and scope races; actual protobuf JSON conversions vs misleading generic JSON transport.

## Task 1 — Rust schema, transport, server API

- [ ] Verify mature crate APIs and settle exact JSON contracts early; report before root UI begins.
- [ ] Add portable virtual proto/descriptor source handling with mature compiler/reflection/model APIs and bounded schemas. Test real multi-file imports, paths and conversion.
- [ ] Extend saved Protocol and interpolated drafts/validation; implement checked pinned tonic connector and library codec glue. Test actual network policy/TLS behavior.
- [ ] Extend sessions with typed message/metadata/status events, commands and mode/half-close handling, preserving quotas/admission/redaction.
- [ ] Add owner-bound schema/Reflection resources and tests with actual tonic fixture covering all four modes, metadata/status/trailers/deadlines/cancel/ownership.
- [ ] Run targeted and workspace tests, strict Clippy/fmt; self-review and write tasks/implementation/grpc-report.md. Root performs independent review before committing.

## Task 2 — Dedicated UI and interchange

- [ ] Extend shared TS protocol/event models against reported contract; reject unsupported foreign exports explicitly.
- [ ] Build lazy modular gRPC workbench with schema sources/Reflection, service/method/input, modes, event/message/status pane, send/half-close/Stop and existing scopes/auth.
- [ ] Add meaningful frontend lifecycle/contract tests; verify actual tonic fixture with agent-browser, editor/draft/schema restoration and keyboard/narrow/a11y.
- [ ] Independent UI review, fix findings, verify final feature and commit/push through proxy. Record capability matrix accurately.

## Task 3 — Follow-through

- [ ] Continue remaining protocol/features from full matrix. Build/distribution refinements after functionality as latest user steering; retain reported CI failures and ready fixes.
- [ ] Complete final integration to main, preserve original untracked files, delete merged development branches; validate final Actions before release claims.
