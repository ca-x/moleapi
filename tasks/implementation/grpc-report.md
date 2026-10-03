# gRPC backend implementation report

Base: d0fd769 in shared application worktree. No commits/pushes, Docker runs, main-checkout edits, frontend/format edits, or root QA service restarts by this implementer. Root handles the UI, interchange and independent review. Manual tonic fixture remains running on18884 (exec session5351).

## Implemented behavior and libraries

- Core protocol `kind: grpc`, descriptor-authoritative service/method selection and original JSON draft preservation. Incomplete/malformed request drafts can be saved; actual execution resolves variables, validates the descriptor-selected method and converts JSON before network traffic.
- Portable virtual multi-file source bundles compile through protox0.9.1's FileResolver and library GoogleFileResolver. No filesystem resolver, proto grammar parser or protobuf encoder was added. Reflection descriptors remain a separate canonical source representation instead of fabricated proto text.
- prost-reflect0.16.5/prost0.14.4 provide the descriptor model, Protobuf JSON semantics, DynamicMessage encoding/decoding and well-known types. A small tonic Codec adapter delegates payload conversion to these libraries; tonic0.14.6 owns all HTTP/2/gRPC framing and stream transport.
- All four invocation modes run through owner-scoped session APIs. Initial JSON supplies the first request message. Unary/server streaming automatically close the send side; client streaming/bidi retain it until explicit half-close. Bidi reads independently of sending. Tonic's generic streaming client supports the same wire protocol for each mode; descriptor cardinality governs outgoing commands and non-server-streaming response counts.
- Real initial metadata, messages, terminal status, details and trailers are retained. HTTP status is not fabricated: gRPC session handshake stays null, and gRPC status lives in typed events. Non-OK target status remains a completed RPC with its real status event; connection/validation failures use the session's Error state.
- Checked_destination validates every DNS result and denies hosted private/reserved destinations. Tonic's custom TCP connector connects/reconnects only to the vetted SocketAddr vector, retaining the original URI/hostname for TLS/SNI. It ignores environment proxy settings and does not redirect.
- TLS verification is enabled by default with native roots and hostname checks. Explicit verify_tls:false works through tonic Endpoint::tls_config_with_verifier and a minimal rustls verifier adapter. The opt-out skips certificate trust/name verification while cryptographic handshake signatures still delegate to rustls. Actual self-signed TLS tests prove true rejects and false succeeds; no unsupported-TLS omission remains.
- Enabled common headers and Bearer/Basic auth become validated tonic metadata. ASCII and base64 `-bin` metadata work. Reserved protocol headers are removed/rejected; outbound and inbound metadata are bounded.
- Existing project/collection/selected-environment/private variable precedence applies to initial and subsequent messages. Send commands use a volatile execution-scope snapshot, not newly selected UI state. Pre scripts can edit URL/headers/auth-related headers/variables and the initial protobuf JSON through pm.request.body.raw/update. Runtime preparation maps the saved message_source into body JSON and reconciles edits back to the execution clone. Response/per-message scripts remain explicitly rejected, following the existing live-protocol contract.
- Existing conservative privacy capture applies before/after pre scripts. Script-written variables are tainted by the pre-existing runtime and therefore private updates are omitted from creation/Reflection feedback. Logs/tests/request updates are scrubbed. Payloads containing known private bytes are wholly withheld; raw binary metadata/status details are checked before base64 masking. Reflection checks both decoded descriptor bytes and serialized schema metadata before returning a portable source.
- Reflection uses generated tonic-reflection v1 and v1alpha clients and typed request/response messages. v1alpha fallback occurs only after an actual Unimplemented response from v1. Missing descriptor dependencies are queried through generated APIs. Target status remains structured, separately from MoleAPI HTTP authentication errors.
- Temporary ReflectionLease records share owner4/global64 live-session admission and owner/workspace cancellation. They leave no retained record when the API finishes. Owner generation/auth and workspace ownership are checked before registration, after registration, and before returning; logout drops pending tonic sockets promptly. Ordinary gRPC sessions reuse admission, ordered cursors/gap reporting, ring retention, ownership, cleanup and logout fences.

## Exact JSON contract

Saved request fields:

```json
{"protocol":{"kind":"grpc","service":"moleapi.fixture.EchoService","method":"Unary","message_source":"{\"text\":\"hello\"}"},"specification_id":"proto-id"}
```

Canonical `Specification` uses `kind:"protobuf"`; `source` is a JSON **string** containing one of:

```json
{"kind":"proto","files":[{"path":"service.proto","content":"syntax = ..."}],"entry_files":["service.proto"]}
{"kind":"descriptor","descriptor_set_base64":"..."}
```

The source string is retained exactly for imported bundles/saved reopen. Import dialect is `protobuf`; Reflection dialect is `descriptor-set`.

- POST `/api/grpc/import`: `{workspace_id,name,source}` returns `{specification,schema}` for an unsaved compiled candidate.
- POST `/api/grpc/schema`: `{workspace_id,specification_id}` returns `{specification,schema}` for an owned saved source.
- POST `/api/grpc/reflect`: `{workspace_id,request,environment_id?,locals?:VariableUpdate[]}` returns `{specification:Specification|null,schema:GrpcSchema|null,error:string|null,status:ReflectionStatus|null,variable_updates:[],request_updates:[],logs:[],tests:[]}`. Target failures use a successful MoleAPI HTTP response with structured status/error; invalid owner authentication still uses HTTP401. Imported/reflected candidates persist only via normal workspace revision/CAS saving.
- `GrpcSchema.services`: `[{name,methods:[{name,full_name,input_type,output_type,client_streaming,server_streaming,input_template:JSON}]}]`.
- `GrpcSchema.messages`: `[{name,fields:[{name,json_name,number,type_name,repeated,map,optional,oneof:string|null}]}]`.
- `GrpcSchema.enums`: `[{name,values:[{name,number}]}]`.
- Existing POST `/api/sessions` creates any gRPC invocation. Summary has `protocol:"grpc"`, `handshake:null`, and `client_half_closed:boolean`. Send commands can queue while Connecting; subsequent sends after accepted half-close are rejected atomically.
- Existing POST `/api/sessions/:id/send` takes `{kind:"grpc_message",message_source:string}` or `{kind:"grpc_half_close"}`. Existing events/close/delete routes remain unchanged.
- Event messages: `{kind:"grpc_message",message:JSON}`; `{kind:"grpc_metadata",phase:"headers"|"trailers",metadata:Pair[]}`; `{kind:"grpc_status",code:number,name:string,message:string,details_base64:string,metadata:Pair[]}`. Each retains existing direction/cursor/time wrapping. Status names use tonic enum names, e.g. `Ok`, `Cancelled`, `DeadlineExceeded`, `PermissionDenied`, `Unauthenticated`. `[REDACTED]` may replace private JSON payloads or binary detail values.
- `ReflectionStatus`: `{code,name,message,details_base64}`; successful Reflection can have null status. Terminal typed reflection ErrorResponse converts through tonic Code, preserving target code/name.

## Source and changes

- Root Cargo.toml/Cargo.lock: compatible mature crates and patch registration; vendor excluded from workspace tests.
- Core: new grpc.rs; Protocol variant/defaults/helpers; grpc-aware interpolation/initial-body preparation; bounded saved draft/schema validation; tests/grpc.rs.
- Protocols: new grpc.rs dynamic codec, pinned transport, metadata/privacy, invocation/Reflection; new commands/events/client-half-close; volatile scope snapshot; ReflectionLease. Existing engines retain their WebSocket command variant. Tests, generated fixture sources/descriptor and manual example are under crates/protocols/tests and examples.
- Server: new grpc.rs routes/import/schema/Reflection; lib router registration; session preparation/configuration; two lines of execution integration for initial-message pre scripts; real offline/hosted tests/grpc.rs. Server adds tonic for inspecting actual target status.
- Script runtime source remains unchanged; its existing supported pm body/header/variable APIs are reused.

## Untrusted source safeguards and limits

Audit found upstream protox-parse0.9.0 recursively parses/generates nested definitions without a configurable nesting guard. The only vendor addition is the upstream MIT/Apache-2.0 protox-parse0.9.0 crate, retaining both licenses, repository/version metadata and Cargo.toml.orig. MOLEAPI-PATCH.md explains the patch: the **upstream Logos lexer** counts structural delimiter nesting32 and tokens100000 before recursive parse/AST generation. Strings/comments use the upstream lexical rules. This is a resource guard, not a replacement parser.

- Virtual path names are portable ASCII relative `.proto` identifiers; traversal, absolute paths, backslashes, colon, repeated/empty/dot components are rejected. Imports see only supplied files and library well-known files.
- Source bundles1–32 supplied files/entry files, per-file256KiB and combined2MiB. This also bounds recursive import-chain depth, with at most64 resolved files including well-known dependencies.
- Encoded descriptors2MiB; prost's standard recursive decode limit applies before pool validation. Types2048, services256, methods1024, per-message fields1024, total fields16384.
- Templates bound nesting4 and total field expansion1024 per method to avoid branching recursive-type explosion. Oneof alternatives are omitted from templates; service/type metadata remains available.
- JSON/protobuf message1MiB each; canonical JSON serialization uses a bounded writer. Both directions cap messages10000. Existing session received/input budgets20MiB, commands32, retained events256/8MiB and lifecycle retention remain in force; the internal tonic send queue holds1 message. Metadata32KiB/128 entries; status message/details32KiB; Reflection descriptor bytes2MiB, files64 and descriptor queries512.
- Request deadline1–120000ms and explicit cancellation drop stream/channel and abort the sender pump. A biased outer deadline avoids tonic's local TimeoutExpired/Cancelled race and emits client DeadlineExceeded correctly.
- Reflection is retained as descriptor source when original `.proto` source is unavailable. Per-response/per-message script/extraction support belongs to the cross-protocol module and is not claimed by this slice. No custom CA/client-certificate fields were added to this existing request model.

## Verification and self-review

Focused final tests passed15/15: core4, protocols5, server6.

- Real virtual multifile import, unsafe/missing imports, lexical guards, deep descriptors, branching templates, nested JSON/64-bit/enum/bytes/oneof/Timestamp conversion, draft source reopen and message-variable interpolation.
- Real tonic Unary/ServerStream/ClientStream/Bidi; ordering, initial message, send while connecting, half-close and send-after-close rejection; real initial/binary metadata, success/error trailers and PermissionDenied binary details.
- Bearer/Basic auth, target Unauthenticated Reflection status, initial-message body pre-script edit, selected environment and tainted scope snapshots, request-update feedback, conservative private payload/source withholding.
- Actual deadline/cancel and self-signed TLS verification/opt-out behavior; real v1/v1alpha Reflection fallback; hosted private policy, owner isolation, malformed saved requests, offline direct-router IPC/schema restoration.
- Actual authenticated bidi logout and pending Reflection socket closure, old-token rejection/no late schema; temporary Reflection quotas/release.

Commands: focused `cargo test -p moleapi-core -p moleapi-protocols -p moleapi-server --test grpc --no-default-features`; final workspace `cargo test --workspace --exclude moleapi-desktop --no-default-features`; strict all-target `cargo clippy --workspace --exclude moleapi-desktop --no-default-features --all-targets -- -D warnings`; `cargo fmt --all -- --check`; `git diff --check`. Final workspace result recorded below after command completion. Logs are /tmp/moleapi-grpc-focused-final.log, /tmp/moleapi-grpc-workspace-tests-final.log, /tmp/moleapi-grpc-clippy-final.log.

Self-review corrections included explicit TLS support after discovering tonic's Endpoint verifier API, a biased deadline to prevent cancellation/deadline conflation, binary privacy checks before base64, bounded recursive template expansion, exposing script body edits as actual initial gRPC JSON, Reflection sharing session quotas/cancellation, registration-before-ownership-recheck and releasing transient Reflection records. No known backend correctness findings remain after focused tests; root independent review/browser acceptance still owns final integration approval.

Final verification: workspace no-default tests passed 128 tests across 34 suites with zero failures; strict workspace all-target Clippy passed; fmt and diff whitespace checks passed. Root completed final UI52tests/typecheck and earlier browser four-mode/source restoration; root owns latest-binary browser acceptance.

## Independent review follow-up: shared schema labels

Independent review established P1: Reflection was constructing a persistable schema name from the resolved execution request title. An actual API regression was added first and reproduced `private-schema-label-token schema` instead of the original `{{message}} schema` (red log /tmp/moleapi-grpc-label-red.log).

Correction: the protocol-level Reflection helper now uses a neutral label; the owned API uses the **original unresolved** request title and scrubs it before exposing the candidate. Candidate checks now cover the whole serialized Specification metadata as well as direct source text, decoded descriptor bytes and schema metadata. The actual API test covers both a secret project variable and a local project override, preserves the source/schema's usability, saves each candidate through workspace CAS, and confirms default native export contains neither resolved private value. The previously failing regression is green (/tmp/moleapi-grpc-label-green.log).

GraphQL introspection already used an unresolved caller request: execution::perform resolves only its internal clone. That existing label path therefore did not have the same resolved-name defect. Its shared candidate boundary now additionally scrubs labels and refuses known-private source/specification/SDL metadata. Two actual HTTP/introspection tests verify unresolved project/local titles survive candidate saving/default export safely, and a schema description containing a known private execution value cannot become a shared candidate (live target response remains an execution response).

Focused server gRPC7 + GraphQL15 tests passed22/22 (/tmp/moleapi-schema-scope-focused.log). During verification an older GraphQL test's whole-event-text `contains("9999")` assertion hit random identifier/time digits. It now checks typed `message.operation_id != "9999"`, preserving the intended unmatched-operation assertion without random metadata false positives; unsolicited-payload and terminal-error assertions remain.

Follow-up changes are limited to crates/protocols/src/grpc.rs; crates/server/src/grpc.rs, graphql.rs; and server tests/grpc.rs, graphql.rs. No UI/formats/build/CI edits or commits. Root independent follow-up recheck remains the final approval boundary.

Final follow-up verification: workspace no-default131 tests passed across34 suites, zero failures; strict workspace all-target Clippy, fmt check and whitespace diff check passed. Logs: /tmp/moleapi-schema-scope-workspace-final.log and /tmp/moleapi-schema-scope-clippy-final.log. Source work stopped again for the independent recheck.
