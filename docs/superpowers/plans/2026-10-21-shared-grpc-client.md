# Shared gRPC client implementation plan

> **For agentic workers:** Use executing-plans inline and verification-before-completion, with one independent final source review.

**Goal:** Shared editors/admins author gRPC requests, stage Proto/Reflection definitions, explicitly Save and use all four method modes with creator-bound stream send/half-close/Stop and events.
**Architecture:** Canonical saved request-bound member import/schema/reflect routes reuse existing compiler and Reflection code, separating authenticated actor from resource owner. Extract controlled GrpcClient and ProtoSourceEditor from owned UI; a member hook stages schema resources without implicit CAS, while streaming edits remain session-local.
**Tech Stack:** Existing protox/prost-reflect/tonic, React/Radix/CodeMirror, real shared tonic fixture.
**Spec:** docs/specs/grpc-client.md plus docs/specs/workspace-collaboration.md shared gRPC subsection.

## Interfaces and constraints

- Saved GrpcConfig/service/method/message_source and protobuf proto/descriptor source union remain unchanged.
- Member import/schema/reflect identify canonical request_id/expected_revision, schema binds saved specification_id. Import takes authored name/source and returns candidate only; Reflection uses saved canonical request/environment and empty locals.
- Separate actor token/admission from canonical owner resources in reflect_as; reuse scrub/descriptor-private screening. Membership/revision/credential watcher and final credential→member fence with owned select lifetime; no losing DB future retained at final check.
- Saved member Grpc sessions and only GrpcMessage/GrpcHalfClose commands are admitted; actor audit includes action/session only. Existing codec, network/TLS, quota and privacy constraints remain.
- Shared import/Reflection/source loading require clean saved context. Stage spec/reference and explicitly Save; late file/compiler/Reflection/Monaco callbacks cannot target changed actor/credential/role/cache/revision/request/draft.
- Initial call sends the canonical saved message. Runtime client-streaming/bidi messages are temporary, no implicit canonical mutation; half-close binds exact open session and preserves incoming data. Unary/server-streaming cannot send extra messages.
- Reuse owned UI semantics and multi-file editor; shared code never mounts useWorkbench or owner endpoints/managers. Viewers have no mutation/connection/private event controls. Other protocol/release work remains required.

## Task 1: Backend routes and transport

- [x] RED real member schema/import/reflect and unary/stream session tests against current closed routes/whitelist.
- [x] Reuse compile/reflect_as with actor credentials, canonical owner resolution and explicit member admission/watch/final fencing.
- [x] All4 modes, canonical owner collision/scopes, Proto/source stage/CAS, request/schema binding, metadata/status/initial and extra messages, half-close/Stop, viewer/foreign/creator/stale/role/revision/logout, audit/privacy and owned compatibility evidence.

## Task 2: Controlled frontend

Files: grpc/GrpcClient.tsx, ProtoSourceEditor.tsx and owned wrappers; workspaces/useSharedGrpcSchema.ts, SharedGrpcWorkbench.tsx, SharedGrpcSessionControls.tsx; SharedWorkspaceView/SharedLiveSessionPanel and catalogs/tests.

- [x] Extract owned UI/source dialog with compatibility tests.
- [x] RED shared create/config/Save/import/Reflection/source selection/all mode UI, stage then explicit Save, runtime local messages and no owner calls.
- [x] Scoped source attach/selection/file/compiler replies, clean-save guards, half-close/late callback and viewer tests.

## Task 3: Final verification/delivery

- [x] Full frontend/backend affected tests; production then strict server Clippy/package fmt and embedded build.
- [ ] Independent review; actual English/Chinese390 import/Reflection/Save/unary/stream/send/half-close/Stop/source reload and desktop acceptance.
- [ ] Docs/screenshots/commit/push/PR update and exact-head CI/Docker observations; native installed/full matrix gates remain open.

## Ledger

- Previous goal turn was progress:3f2edcb pushed,682 frontend/45 backend plus real bilingual Socket.IO and terminal poll race evidence. App39482001406 and Docker39482000871 are confirmed live at this turn's observation, no timeout-based restart.
- Existing collaboration-next isolated worktree starts clean. Preserve root user mise.toml; no new dependency/database schema or hand-written protocol parser.

- Current goal turn: progress. Existing fixture build finished successfully; no restart. App/Docker for3f2edcb observed independently, Docker both native architectures and container smoke passed, PR publication skipped as configured.
- Task1/2 source and RED→GREEN evidence: collaboration_grpc2 real tonic tests; owned gRPC7 compatibility; full affected backend59 pass/1 configured ignored; full frontend700/132 files. Retained hook callbacks and edit/revert schema retry each reproduced before fixes. Independent shared_report_export_review found no concrete source finding.
- Task3 local gates: TypeScript, npm production39 chunks/no static cycles, strict server Clippy, package fmt/diff and embedded server build all exit0. Real embedded member browser validates manual multi-file Proto stage/CAS, Reflection stage/CAS/reopen, Unary owner variable, Bidi/client streaming local extra messages and half-close to OK, server streaming Stop to Cancelled. Canonical revision/source preserved across temporary sends. English/Chinese390 and1280 document widths match viewports; browser error list empty. Unique bilingual captures retained in docs/images.
- Ruling: native installed GUI/file chooser and full platform/database/distribution matrix stay in the integrated acceptance gate already recorded in project status; web1280 is not native acceptance. This slice cannot close those requirements.
- Delivery and exact new-head CI/Docker remain pending until commit/push.
