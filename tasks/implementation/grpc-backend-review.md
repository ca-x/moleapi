# Independent gRPC functional backend review

Baseline: `d0fd769`; reviewed the shared application worktree as uncommitted implementation. Read-only source review; no implementation edits, commits, pushes, Docker, or root/backend-owned service mutations. Scope was narrowed by root to ordinary feature workflows; no adversarial or penetration-test probes were performed.

## Finding — resolved after independent post-fix recheck

### Resolved P1 — Reflection converted a private execution variable into a shared schema label

Evidence: `crates/protocols/src/grpc.rs:555` creates `Specification.name` from the **resolved** `request.name`. `crates/server/src/grpc.rs:167–203` checks reflected descriptor bytes, `spec.source`, and serialized schema for private execution values, but does not check or scrub `spec.name`. The result is a canonical specification candidate; `web/src/features/grpc/useGrpcSchema.ts:168` attaches it to the shared workspace. Default native export preserves specification names. A private/local value therefore escapes execution scope and becomes saved/exportable project metadata even though the descriptor source itself is clean.

Independently reproduced with the normal generated tonic fixture and an isolated direct local router: create a saved gRPC request whose name is `{{message}}`; set the global variable `message` to `private-review-token`, enabled, `secret:true`; POST `/api/grpc/reflect` using that request. Actual result:

```
REFLECTION_STATUS=200 OK SPEC_NAME="private-review-token schema"
```

The source and schema remain usable, so this is specifically an execution-versus-shared-data scope regression. Preserve an unresolved saved label, use a neutral schema label, or scrub the returned label before exposing the candidate. Include a regression assertion that no private resolved request name appears anywhere in the returned candidate or a subsequent default export.

Reproduction: `/tmp/moleapi-grpc-review-harness/src/main.rs`; run `CARGO_TARGET_DIR=/home/czyt/code/rust/moleapi/.worktrees/application/target cargo run --manifest-path /tmp/moleapi-grpc-review-harness/Cargo.toml --offline`. It used fixture port18885, a temporary SQLite file, the existing generated fixture/source bundle, and direct-router API calls. The fixture task was aborted normally after the assertion. Log: `/tmp/moleapi-grpc-review-harness.log`.

## Independently rerun verification

Command: `cargo test -p moleapi-core -p moleapi-protocols -p moleapi-server --test grpc --no-default-features`.

Result: all15 tests passed (core4, protocols5, server6); zero failures. This session's output is `/tmp/moleapi-grpc-independent-focused.log`.

Those tests exercise canonical multi-file import and descriptor reopen, actual unary/server-stream/client-stream/bidi results and ordering, JSON scalar/enum/bytes/oneof/Timestamp conversion, real metadata and rich status details, explicit half-close/send-after-close rejection, deadlines/cancel, Bearer/Basic, initial-body pre scripts, selected variable scopes, self-signed TLS verification/opt-out, Reflection v1/v1alpha, offline router operation, ownership/logout behavior, and shared Reflection admission/release. The above label regression is outside their current assertions and passed its independent reproduction despite the15 green tests.

## Source review outcome

Reviewed `core` gRPC model/defaults/interpolation/validation/schema compilation, `protocols` dynamic codec/transport/metadata/events/stream commands/Reflection/session integration, server import/schema/Reflection/create preparation, and native/foreign format boundaries. Based on source reading, mature tonic/prost/prost-reflect/protox/generated Reflection APIs own protocol processing; the custom codec delegates to DynamicMessage. Original hostname/SNI survives the connector, settings map to explicit TLS verification and request deadlines, canonical source strings survive save/reopen, and unsupported Postman/OpenAPI exports reject protobuf specifications. No additional normal-workflow functional finding established.

Also inspected vendor provenance/licenses and the resource guard: upstream MIT/Apache licenses and original manifest are retained; the patch uses upstream Logos tokens before recursive parse. This is source inspection, not an independent adversarial validation claim.

## Completion ledger

1. Done: requested ordinary backend workflow source review and independent15-test rerun.
2. Done: established Reflection-label scope finding with a real isolated API reproduction.
3. Done: root/implementer corrected the label scope finding; this reviewer independently reran the original normal reproduction and the server gRPC/GraphQL suites after the fix. No open finding remains in the reviewed functional scope.
4. Not applicable: implementation edits, publication, CI/release claims, or adversarial probing by this reviewer.

## Independent post-fix recheck

Current reviewed source remains uncommitted over HEAD `d0fd7695ded62c1ebee62e5d8d6ba6d51a462007`. The low-level Reflection helper now supplies a neutral name. The server retains `c.request.name` before preparation (`crates/server/src/grpc.rs:100`), derives the candidate label from that unresolved title and scrubs it (`:168`), and checks the entire serialized specification as well as decoded descriptor bytes/source/schema (`:194`). The original ordinary fixture reproduction now returns:

```
REFLECTION_STATUS=200 OK SPEC_NAME="{{message}} schema"
```

The isolated harness asserts that the full response does not contain `private-review-token`; it completed with exit0. Command is the original reproduction command listed above, with only the expected assertion changed to the corrected behavior. Log: `/tmp/moleapi-grpc-review-harness-postfix.log`.

Independent command: `cargo test -p moleapi-server --test grpc --test graphql --no-default-features` completed with exit0: gRPC7/7 and GraphQL15/15,22 total, zero failures. Log: `/tmp/moleapi-schema-scope-independent-postfix.log`. This includes actual API → candidate → revision/CAS save → default native export assertions for project-private titles and local overrides in both protocols, ordinary source/reopen/four-mode behavior, and the GraphQL private-description withholding case. `git diff --check` also passed in this recheck.

Adjacent GraphQL source review confirms its original request title stays unresolved while execution resolves a clone; candidate names are scrubbed (`crates/server/src/graphql.rs:90`), complete specification/source and generated SDL are screened before a shared candidate is returned (`:95–113`). Valid nonprivate introspection source/SDL roundtrip tests remain green.

Finding status: **resolved; no open ordinary functional regression finding established**. This is backend/source/test sign-off for the requested scope, not a browser, CI, release, or adversarial-security claim. No implementation edits or shared-service mutations were performed by this reviewer.
