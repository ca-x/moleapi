# GraphQL backend handoff

Worktree: `.worktrees/application`, branch `feat/application`, baseline `b1ffce7`. Changes are uncommitted. This report covers Rust core/protocol/server code; the parent owns frontend, interchange adapters, distribution, browser checks and matrix updates.

## Model and API

Saved request protocol is backward-compatible JSON:

```json
{
  "kind": "graphql",
  "document": "query Hello($name: String!) { hello(name: $name) }",
  "variables": {"name": "World"},
  "variables_source": "{\"name\":\"World\"}",
  "operation_name": "Hello",
  "connection_params": {},
  "subscription_url": "wss://example.com/graphql"
}
```

`document`, `variables` and `connection_params` default to empty drafts/object; the other fields are optional. `variables_source` retains raw editable JSON, including incomplete/invalid saved drafts; execution parses this source as an object and uses it ahead of the last valid `variables` object. The shared request URL/auth/query/headers/TLS/timeout and scope/local values remain authoritative. `specification_id` references workspace-owned schema sources. Old missing protocol continues to default HTTP.

`POST /api/execute` retains its existing input/output. GraphQL query/mutation uses a serde-generated POST JSON envelope before pre scripts inspect or change `pm.request.body.raw`. Only actual pre-script changes produce `request_updates`; canonical envelope preparation does not overwrite shared saved body/method drafts. Script-edited envelope/method/body mode is checked again, resolved variables are JSON-escaped, selected AST operation is validated, and subscriptions are rejected before HTTP networking. Actual HTTP status, headers, body, data/errors, assertions and script results remain `Response`; GraphQL errors with HTTP200 remain HTTP200. The GraphiQL fetcher must validate a parsed response has data/errors. Unsupported body envelope fields or method/body-mode mutations error explicitly.

`POST /api/graphql/introspect` takes the execute shape (`workspace_id`, saved request, optional environment/variables/data/locals). It returns `{response, specification?, sdl?, error?}`. The bounded actual target response is nested inside a successful MoleAPI call; target401 does not become owner-session401. The generated Cynic June2018-compatible introspection query uses the same execution/auth/scopes/script/HTTP network pipeline. Valid results return an unsaved canonical `graphql-introspection` specification containing the original response source and its mature-model SDL conversion. UI explicitly saves the returned source in workspace data.

`POST /api/graphql/schema` takes `{workspace_id, specification_id}` and returns `{specification,sdl}` after ownership and schema parsing. `graphql-sdl` sources retain original SDL byte-for-byte; `graphql-introspection` accepts an original response wrapper or raw data object. Both source kinds are syntax/model checked when saving workspace data.

`POST /api/sessions` accepts selected GraphQL subscriptions and retains existing owner/admission/generation/delete/logout/cancel/retention APIs. It uses optional `subscription_url` (ws/wss), otherwise converts shared http/https URL to WebSocket transport. `SessionSummary.protocol` is `graphql`. Events are `graphql_next`, `graphql_error`, `graphql_complete`, each with `operation_id` and `payload` (complete null), plus existing state/close events. Only events routed by the mature actor to its active subscription reach the event store. Connection-init payload/auth parameters are never event/log/summary payloads. Raw send API remains specific to native WebSocket sessions. Post-response/event scripts remain explicitly unavailable for subscriptions.

## Libraries and modules

- `crates/core/src/graphql.rs`: pinned async-graphql-parser7.2.1 syntax/AST selection, bounded built-in variable checks, envelope preparation/reconciliation, and Cynic3.14.0/cynic-introspection3.14.0 query/schema/SDL handling. Custom scalar/enum/input coercion remains the target schema's authority.
- `crates/protocols/src/graphql.rs`: `graphql-ws-client` Connection adapter over the existing checked/pinned reqwest-websocket0.5.1/tungstenite0.27 transport. Shared DNS/private-network/TLS checks run on every redirect hop; authorization/cookies and connection parameters are removed across origins. WebSocket size and session byte/retention/admission limits remain shared.
- `crates/server/src/graphql.rs`: owner-bound introspection and saved-schema APIs; generic execution/session modules handle the shared pipeline.
- `vendor/graphql-ws-client`: pinned upstream0.13.0/Apache-2.0 source and license. Documented small patch exposes mature protocol events through a default observer hook after actor routing; terminates operations on terminal Error; replaces invalid normal close100 with1000. Upstream handles connection-init/ack, routing, next/error/complete, ping/pong and cancellation. Logging disabled. Vendor is excluded from the root workspace and has an explicit workspace boundary, avoiding its upstream dev/test dependencies leaking into application builds.

Crate metadata and original source/API documentation were inspected using `cargo info --registry crates-io` and the downloaded registry source before implementation. Test-only async-graphql7.2.1/async-graphql-axum7.2.1 provide a real executable schema service. Wire-edge fixtures simulate target error/auth/redirect behavior; production has no custom GraphQL lexer, schema model or subscription state machine.

The shared template resolver now recognizes ordinary adjacent GraphQL closing braces (`}}`) while maintaining bounded scope interpolation. Envelope query and variables are resolved separately as JSON values, avoiding broken quote/backslash substitutions and duplicate evaluation of stale document/variables drafts. Shared HTTP body drafts can remain incomplete when a GraphQL protocol uses its own dedicated document/variables source.

## Verification

Initial `cargo test --workspace --exclude moleapi-desktop --no-default-features` passed99 tests (exit0), including3 core and9 server GraphQL tests. Strict `cargo clippy --workspace --exclude moleapi-desktop --all-targets --no-default-features -- -D warnings` passed (exit0). `cargo fmt --all` completed. No local Docker or persistent listener was started.

New core tests cover old defaults, incomplete saved drafts, selected/missing/multiple operations, built-in variable/default validation, oversized documents, invalid raw variables, nested closing braces, retained HTTP body drafts, SDL original preservation and malformed introspection. New server fixtures cover real queries/mutations, explicit selection, quoted variables, resolver errors, introspection/types/source navigation, target401 and hosted login, canonical pre-script edits and pre-network rejection, subscription next/error/complete/cancel, terminal protocol error with an intentionally open peer, mature pre-ack ping/pong, unsolicited operation IDs ignored, credential redirection, native private allow/hosted deny, owner isolation, logout cancellation/fencing, auth close4401, and explicit unsupported legacy negotiation. Existing session tests retain quotas, bounded buffers, retention and deletion behavior.

## Explicit limits and remaining integration checks

GraphQL subscriptions implement the negotiated `graphql-transport-ws` standard only. Legacy Apollo `graphql-ws`/subscriptions-transport-ws is unsupported and rejected explicitly. One selected operation per owner-bound session; arbitrary WebSocket frames and per-event scripts are unavailable. Finite operations use canonical HTTP POST; multipart uploads, persisted-query extensions and batched envelopes are not claimed. Schema sources are limited5MiB/10000 definitions or types; documents256KiB; mature lexer quotas enforce32 structural nesting and1000000 tokens before recursive parsing; existing request/response/scopes/script/session limits still apply. Introspection requests the portable June2018 fields, rather than falsely claiming version capability detection/October2021-specific fields.

Frontend GraphiQL/editor/schema browser, sync/export source and credential preservation, browser/native artifact checks, independent combined review and full platform/three-database GitHub Actions remain parent-owned. Local tests use SQLite and the actual network fixtures; PostgreSQL/MySQL platform delivery is not inferred from this run.


## Independent review fixes, round1

Cynic's normal SDL formatter omits an explicit schema definition whenever present roots have conventional names, even if an ordinary object named Mutation or Subscription exists while the corresponding introspection root is null. Conversion now preserves actual operation roots: the parsed mature SDL AST detects an omitted schema definition, and the public Cynic0.11.2 AST writer/printer emits the exact root mapping from the introspection model. Canonical original introspection JSON remains unchanged. The regression compares parsed explicit root semantics to the mature introspection schema, including retained ordinary Mutation/Subscription object types.

The recursive async-graphql-parser schema grammar/type conversion lacked a structural quota before parsing. Pinned stable Apollo parser0.8.6 provides an iterative public lexer; its tokens enforce32 nesting and1000000 tokens before either schema or executable document parsing. Quoted/escaped/block strings and comments remain mature-library tokens and do not create false depth counts. This is a resource quota, not a custom GraphQL lexer or parser. Deep list types, nested default-value lists and executable variable definitions reject before recursive grammar/AST work. An isolated child-process regression checks4000 levels without process termination; server save validation rejects a deep schema and the API remains usable. Existing valid SDL source bytes are preserved.

The optional GraphQL subscription URL is now inspected by server privacy capture before pre scripts, including original/resolved sensitive query names and credentials. The backend pm.js shared URL-capture helper also visits subscription_url during scope and URL mutations, using the existing Rust capture callbacks and privacy limits. A transient alias opaque→access_token→opaque cannot hide a credential on a later script throw. Regression evidence includes direct failure, intermediate alias changes, console feedback, cross-origin redirect and target4401 echo; error/log/event/summary feedback remains redacted. This backend runtime helper extension was explicitly authorized by the parent; frontend/branding/workflows remain untouched by this worker.

Both schema regressions and the runtime alias regression were observed failing before the fixes, then passing. Fresh final round1 verification:106 tests passed with `cargo test --workspace --exclude moleapi-desktop --no-default-features` (exit0), including6 core GraphQL/11 server GraphQL tests and the additional runtime alias test. Strict all-target no-default Clippy `-D warnings`, `cargo fmt --all --check` and `git diff --check` passed. No public target probes, Docker, commits or pushes were performed. Combined independent re-review and frontend/platform delivery remain parent-owned.


## Independent review and browser interop fixes, round2

Upstream graphql-ws-client parsed operation IDs as integers, so01/+1 incorrectly routed as the canonical outbound ID1. The existing SubscriptionId adapter now also requires exact equality to its canonical emitted string. The mature engine retains protocol parsing/routing/cancellation; aliases take its existing unknown-subscription error path and cannot produce active next/error/complete events. The real WebSocket fixture was observed red for01 next on the prior behavior, then passes all six01/+1 next/error/complete combinations. Existing canonical1 next/error/complete/cancel fixtures remain green.

The exact portable introspection HTTP200 response produced by the parent's GraphQL-JS16.14.2 service is retained as `crates/core/tests/fixtures/graphql-js16-introspection.json`. The concrete serde cause was a valid DIRECTIVE_DEFINITION value in @deprecated.locations, absent from Cynic3.14.0's enum. A source-only documented MPL2.0 vendor patch adds this value to its upstream introspection schema, mature DirectiveLocation enum and SDL formatter. The portable query, mature deserializer/model/writer, and original JSON source remain unchanged. Vendor version/license/source attribution and complete license text are included; its explicit workspace boundary prevents upstream dev/test dependencies leaking into application builds.

The selected mature SDL parser does not implement custom directives on DIRECTIVE_DEFINITION. Introspection with the standard built-in metadata now passes; custom use reports an explicit unsupported-location error without silently dropping metadata. Direct SDL imports receive the same precise error at the mature parser's failing token, while ordinary fields named DIRECTIVE_DEFINITION remain valid. Original source stays unchanged. This is an explicit current grammar limit, not a fabricated complete newer-spec claim.

Core exact-response conversion and real local HTTP introspection/save/schema-reload fixtures pass with original19667-byte JSON preserved. The enum-interoperability and saved SDL location regressions were red before their corresponding fixes. Fresh final round2 verification:111 tests pass with `cargo test --workspace --exclude moleapi-desktop --no-default-features` (exit0), including9 core GraphQL and13 server GraphQL tests. Strict all-target no-default Clippy `-D warnings`, `cargo fmt --all --check`, and `git diff --check` passed. No root-owned services were accessed, no public target probes or Docker were used, and changes remain uncommitted. Source work stops here for the parent's final recheck, browser verification and distribution checks.
