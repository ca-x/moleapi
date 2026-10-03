# GraphQL client contract

Authority: user's explicit complete API-type coverage requirement and docs/PROTOCOL-COVERAGE.md. Build after verified SSE/WebSocket slice, using mature Rust libraries. This specification is not an implemented-feature claim.

## Capability and libraries

Implement query, mutation, operation selection, variables, schema introspection/browser, saved SDL/introspection source, and subscriptions. Use async-graphql-parser for GraphQL syntax, cynic-introspection for introspection query/model/schema handling, and graphql-ws-client for subscription protocol. Reuse existing checked/pinned reqwest and WebSocket transport. No custom GraphQL lexer, schema parser, introspection JSON schema, or graphql-transport-ws state machine. Verify actual compatible crate features/versions before adding dependencies.

The dedicated frontend should use mature GraphiQL/GraphQL language-service/editor/schema components, integrated with existing React/Radix workbench and local assets. Its fetcher delegates to Rust HTTP/IPC/session APIs; no browser-direct network bypass. Schema descriptions/errors remain untrusted text and rendered safely. Keep large editor/schema packages lazy to preserve startup and binary embedding.

## Model and execution boundary

Add a typed saved GraphQL protocol variant with document:string, variables:JSON object, operation_name?:string, and optional specification reference; defaults permit older data and incomplete saved editor drafts. Schema references are owner/workspace-bound. HTTP URL/auth/query/headers/TLS settings and scope/local variables remain shared. Distinguish finite HTTP query/mutation execution from long-lived subscriptions, using mature AST operation selection rather than a string-prefix guess.

For query/mutation generate the canonical HTTP POST JSON {query,variables,operationName?} through serde_json before pre scripts so supported pm body hooks can inspect/edit the actual request. Execute through the existing server script/HTTP pipeline, preserving actual HTTP status/headers/body and GraphQL errors/data. Revalidate a script-modified GraphQL request and method/body before networking; unsupported modifications must error explicitly. Do not pretend GraphQL errors are HTTP success tests or relabel arbitrary HTTP JSON as validated GraphQL support.

Expose owner-bound introspection operation with existing request credentials/scopes, the generated mature introspection query and bounded actual HTTP response. Keep target401 as a target response/error rather than expiring MoleAPI login. Retain canonical introspection/SDL in specifications; provide available operations/types for the UI through mature models. Original/schema imports stay source-preserving and default export strips credentials. Formats unable to preserve GraphQL configuration must reject/warn until actual adapter support exists.

Subscriptions use the existing owner/admission/generation/cancellation/event-retention session manager and checked pinned WebSocket. Add typed GraphQL result/error/complete events, preserving operation IDs/payloads and close semantics. Delegate connection_init/ack, next/error/complete, ping/pong to graphql-ws-client. Support the negotiated standard transport(s) the selected crate actually implements, list any unsupported legacy variants explicitly. Connection parameters/auth must not leak to logs/summary/shared synchronization. No unsupported post-response/event scripts claimed.

## Limits and lifecycle

Apply existing request/response/script/schema limits, variable validation, four-tier owner/session quotas, cancellation and 256-event/8MiB retention. Bound introspection types/document bytes before expensive processing; preserve parser errors without arbitrary callback evaluation. Account/workspace/request guards prevent stale UI schema/session results and local changes crossing scopes. Request/workspace deletion and logout cancel subscriptions, including in-flight preparation; default hosted private-network restrictions/DNS pin/TLS/redirect credential policy apply on every transport.

## Acceptance

Real local GraphQL service fixtures for query/mutation, explicit/multiple operation selection, variables/types/schema introspection, error payloads, required-auth rejection, and subscriptions data/error/complete/cancel. Malformed or oversized documents and invalid variables reject without networking. Probe cross-origin credential redirects, private target deny/native allow, ownership, late logout and old JSON compatibility. Browser runs actual query/mutation/introspection/schema navigation and subscription lifecycle with keyboard/narrow layout/a11y checks. Native/no-default API and full shared database/platform Actions build are required after independent review. Mark individual GraphQL matrix targets complete only when verified, not because dependencies or a selector exist.
