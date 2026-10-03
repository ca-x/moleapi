# gRPC client contract

Authority: user's explicit requirement to complete Apifox/Postman API types with mature Rust crates; docs/PROTOCOL-COVERAGE.md. This contract defines one protocol slice, not completion of the full product matrix. Continue implementation without another approval gate: scope and library reuse are already authorized.

## Capabilities and modules

| Module | Responsibility | Dependencies |
| --- | --- | --- |
| grpc-schema | Portable multi-file proto sources, descriptors, service/method/type metadata, JSON templates | core + protox/prost-reflect |
| grpc-transport | Real unary, server streaming, client streaming, bidirectional streaming with metadata/status/deadlines/half-close/cancel | schema + tonic/prost/hyper |
| grpc-api | Owner/workspace-bound import/Reflection/schema/session APIs, variables/auth/pre scripts/privacy | transport + existing server pipeline |
| grpc-workbench | Dedicated source/service/method/message/event interface usable through hosted HTTP and offline desktop IPC | API + React/Radix/CodeMirror |

Build order: Rust modules as one implementer task, root-owned frontend/interchange integration, independent reviews, real fixture/browser acceptance.

## Rust implementation

Use compatible current tonic0.14/prost/prost-reflect0.16/protox0.9/tonic-reflection0.14 versions verified against crate APIs. Runtime proto import and JSON/protobuf conversion must use mature libraries. No custom protobuf parser, encoder, gRPC frame codec or reflection state machine. Tonic's minimal dynamic Codec adapter is glue delegating encode/decode to prost-reflect, not a replacement frame implementation.

Save a typed `kind: grpc` protocol with `service`, `method` and an authoritative `message_source` JSON draft (default `{}`). Retain the request-level `specification_id` reference to its canonical protobuf schema. Allow incomplete/malformed drafts to be saved but reject actual execution before networking. URI uses `http://` for h2c and `https://` for TLS. Service/method names derive from descriptors, never arbitrary request-path manipulation.

Schema specifications use `kind: protobuf`, source JSON `{kind:"proto",files:[{path,content}],entry_files:[path]}`. Reflection sources use `{kind:"descriptor",descriptor_set_base64:string}`; original descriptors are retained. Path identifiers are relative portable virtual names, no filesystem access or traversal/import escape; files and descriptors are bounded. `protox` compiles in-memory virtual imports with its mature resolver abstraction. Well-known types use library-provided sources/descriptors. Retain original source for reopen/offline import/export. `prost-reflect` provides descriptors and Protobuf JSON handling (64-bit integers, enums, bytes, oneof, nested and well-known types). Verify compiler depth/token protections before exposing it to untrusted sources; use library safeguards or a bounded isolated worker if needed, never crash the main binary on recursive input.

Expose `/api/grpc/import` owner-bound `{workspace_id,name,source:string}` -> an unsaved validated `{specification,schema}` candidate, then UI attaches it without deleting other definitions. Expose `/api/grpc/schema` owner-bound `{workspace_id,specification_id}` -> service/method/input/output metadata and input templates. Expose `/api/grpc/reflect` using an execute-shaped context -> canonical unsaved protobuf specification/descriptor source plus bounded schema metadata. Use generated mature tonic-reflection v1 and v1alpha clients; choose supported version through actual responses. Do not parse wire reflection manually. Preserve returned descriptor sets as a distinct canonical representation if Reflection cannot recover original proto text; document the kind/source union rather than fabricate proto source. Root needs the exact JSON contract early.

## Execution and sessions

Use existing owner-scoped session APIs for all four gRPC method modes. Add real typed metadata/message/status events, status code/name/message/details and headers/trailers. gRPC status is not HTTP status. Preserve ordered events, gap detection, bounded byte/message retention and existing admission/logout lifecycle. Client-streaming/bidi sessions accept `{kind:"grpc_message",message_source:string}` JSON drafts and `{kind:"grpc_half_close"}`; prevent send-after-half-close and reject send operations for non-client-streaming methods. A configured initial draft supplies the first message; half-close unary/server-streaming request immediately, keep streaming send side open until explicit half-close. Bidi receives independently of UI sending. Mature codec converts each message with actual input/output descriptor.

Auth/common enabled headers become validated gRPC metadata; support ascii and `-bin` values with base64 using tonic's mature metadata APIs. Exclude reserved protocol-controlled headers. Bearer/basic remain usable. Resolve selected project/collection/environment/private variables, including URL, metadata and JSON draft before execution; secret-derived messages/status/errors/metadata must use existing conservative privacy capture/redaction. Execute supported pre scripts with explicit semantics; do not claim unsupported per-message/post-response scripts. Do not leak secrets into persisted history, schema copies, shared snapshots/exports, logs or summary URLs. Retain response script/extraction work as a later cross-protocol capability.

Implement a tonic connector that validates addresses through existing public network policy, pins vetted DNS results and retains TLS SNI/hostname checks. No second unchecked DNS lookup, environment proxy bypass, redirects, or weakening hosted default private-network denial. Verify TLS-disable behavior explicitly: implement it through mature TLS connector if possible; otherwise reject unsupported insecure TLS setting clearly, never silently ignore it. Request deadlines/cancellation close channels and pending tasks. Bound outbound messages, received messages, metadata, reflection descriptor data and channel queues. Reuse session quotas and owner checks before every operation.

## Frontend

Dedicated modular React/Radix/CodeMirror page: proto bundle import with file list/source, Reflection button, source selection, service/method lists and streaming-mode indicator, JSON input/templates, existing auth/environment/pre-script settings, connect/call, message send, half-close, Stop, ordered message/metadata/status detail. Never use native WebSocket labels/actions for gRPC. Source import and request edits save through existing revision/CAS flow. Account/workspace/request/environment changes close active sessions and fence late creation/results. External target errors remain distinct from MoleAPI owner login. Mature JSON editor assets embed locally. Finite gRPC and streaming integration should share server/desktop IPC; no localhost HTTP server in desktop.

Formats that cannot represent gRPC/schema config must reject explicitly and recommend full MoleAPI export; native export retains protobuf sources but strips private credentials as before.

## Acceptance and commands

Run cargo fmt --all -- --check; cargo test --workspace --exclude moleapi-desktop --no-default-features; cargo clippy --workspace --exclude moleapi-desktop --no-default-features -- -D warnings; npm --prefix web test; npm --prefix web run build. Do not run Docker locally; user prioritizes complete usable functionality before more build refinements.

Real tonic fixture using a multi-file proto with all four modes and Reflection, actual message echo/multi-message ordering/half-close, rich statuses/trailers, metadata auth rejection, non-OK status preservation, deadlines and cancellation. Verify JSON scalar/enum/oneof/bytes/well-known conversion, saved schema reopen/offline operation, unknown methods/imports, unsafe proto paths, malformed/oversized/deep schemas/messages, ownership, private target policy and late logout. Meaningful tests precede claims. Browser must execute actual unary and streaming, send/half-close/Stop, restore schemas, rapid navigation/keyboard/environment checks. Independent review before commit/push. Main is the final delivery branch; preserve original untracked scaffolds before later main integration and remove only merged redundant branches.
