# MCP Task 1 backend report — source frozen for independent review

Worktree `/home/czyt/code/rust/moleapi/.worktrees/application`, base `6ea9b18`; no staging, commit, push, Docker, packaging or Actions changes. Root owns frontend/formats/docs and all existing QA/fixture processes; these were not restarted or modified.

## Implemented

- Saved boxed/flattened `Protocol::Mcp` / `McpConfig`, incomplete JSON argument/connection drafts, original selected-host-entry `config_source` preservation. Configuration source and argument drafts are not interpolated during connection; original scoped variables are stored transiently for typed JSON argument resolution when invoking.
- Official rmcp **3.5.0** client initialization/negotiation using the stable initialize lifecycle (`2025-11-25` preferred), typed messages/multiplexing/errors/cancellation, SDK streamable HTTP and SDK Tokio child process transport. No custom JSON-RPC or SSE framing parser. No advertised discovery/2026 protocol modes, legacy SSE transport, OAuth browser flow, MCP Apps, server publishing, or automatic AI execution.
- Custom `StreamableHttpClient` checks shared `checked_destination` on every POST/GET/DELETE, pins every resolved address, disables ambient proxies, applies actual Basic/Bearer/custom headers, TLS verification/explicit opt-out and timeout. Original host/SNI is retained. **Redirects are rejected**; no automatic contact to a redirected origin, OAuth discovery, resource links or resource URIs.
- Native/local router authorizes STDIO only through explicit connect. Hosted server is HTTP-only unless the administrator's `MOLEAPI_MCP_STDIO_ALLOWLIST` contains the exact absolute executable path (platform path-list syntax). STDIO requires an absolute executable path, separate arguments, cleared host environment, caller-declared environment only, no shell, discarded stderr, process group on Unix / job object on Windows, and bounded awaited cleanup. Existing Tauri `/api/` adapter already routes these local APIs and needs no new command or OS execution bridge.
- Discovery: tools/resources/templates/prompts with bounded pagination; actual tool/schema validation, resource reads, prompt string/required arguments, resource subscriptions, progress/logging/resource updates/list changes; list changes queue a capability refresh. Text/structured JSON/image/audio/link/embedded-resource content remains bounded typed transient event data, never fetched independently or executed.
- Manual sampling, form elicitation (accepted content checked against requested schema), and roots callbacks (at most 64 explicit file URIs). Only these implemented capabilities are advertised. Accept/decline/cancel/error replies, expiry/server cancellation/disconnection, stale callback errors and bounded pending work. Callback completion notification retires expired/replied UI items.
- Uses existing owner/workspace/request/admission/logout session lifecycle and retention. SDK lifetime uses owner-linked cancellation tokens and a drop guard, including when the outer session lifetime future is dropped. Scoped private values are masked in event/status labels; original saved configurations/drafts are not replaced by resolved execution values. MCP handshake metadata stays null: identity/version are `mcp_initialized`, not a fictitious HTTP result.

## Exact contracts

Saved protocol:

```
{kind:"mcp", transport:"http"|"stdio", command:string, args:string[], env:Pair[],
 operation:"tools/call"|"resources/read"|"prompts/get", name:string,
 arguments_source:string, uri:string, config_source?:string}
```

`config_source` is the exact original selected server **entry** JSON, not an entire mcpServers document. Top-level RequestSpec.url is the HTTP endpoint; STDIO command never becomes URL. Defaults are HTTP/tools-call/empty command, args, env, name, URI / `{}` argument source.

Existing `/api/sessions` create/get/events/send/close/delete APIs remain unchanged. Send variants:

```
{kind:"mcp_request",request_id,method,name?:string,uri?:string,arguments_source?:string}
{kind:"mcp_callback",callback_id,result?:JSON,error?:{code,message,data?}}
{kind:"mcp_cancel",request_id}
```

Allowed methods: refresh, tools/call, resources/read, prompts/get, resources/subscribe, resources/unsubscribe. Callback must supply exactly one of result/error. Request/callback identifiers are nonempty, at most 128 bytes.

Events:

```
mcp_initialized {info:SDKInitializeResult}
mcp_capabilities {tools:JSON[],resources:JSON[],resource_templates:JSON[],prompts:JSON[]}
mcp_result {request_id,method,result:JSON}
mcp_error {request_id,method,error:{code,message,data?}}
mcp_notification {method,params:JSON}
mcp_callback {callback_id,method,params:JSON}
```

Additive completion notification: `mcp_notification` method `moleapi/callback_completed`, params `{callback_id,status:"replied"|"expired"}`. Sampling result follows SDK `{role,content,model,stopReason?}`; form elicitation `{action:"accept"|"decline"|"cancel",content?:object}`; roots `{roots:[{uri:"file:///...",name?}]}`. SDK protocol error code/data and tool `isError` remain distinct.

## Bounds and SDK provenance

Input/response/event JSON: 1 MiB, depth32, 10,000 nodes. Catalog: 32 pages, repeated-cursor rejection, 512 entries and 1 MiB per category, combined emitted catalog also 1 MiB. Four concurrent operations, eight pending callbacks, 120-second callback expiry, 512 commands and 4096 incoming events per session. Existing global64 live / owner4 live, owner16 retained / global1024 retained; event retention256 entries/8 MiB with explicit cursor gaps; lifetime30 minutes/retention10 minutes. Request timeout1..120000 ms; SDK cleanup3 seconds and service/job cleanup waits5 seconds.

Official SDK source is preserved at `vendor/rmcp`, Apache-2.0 LICENSE, version3.5.0, upstream package VCS `0cde3c5cf3e6aff0cc852ce6045f107e95991f48`; exact downstream provenance is `vendor/rmcp/MOLEAPI-PATCHES.md`. Narrow patches:

- STDIO raw line1 MiB and total8 MiB before parse, counting cancellation-interrupted partial lines correctly.
- JSON/error HTTP body1 MiB before SDK parse/body preview.
- SDK raw SSE stream8 MiB including comments plus configured SDK event1 MiB; session typed SSE/JSON message budget8 MiB.
- Close terminates/reaps owned group/job; Drop synchronously signals it before scheduling SDK cleanup so runtime shutdown cannot strand a tree.

Protocol/process lifetimes and protocol buffer/work bounds are enforced; this does not create an OS sandbox for an explicitly authorized executable. Stderr is discarded rather than inherited or unboundedly captured. reqwest0.13 uses provider-free rustls plus the existing workspace ring default (only installs if none), avoiding AWS/ring feature collision in existing gRPC TLS.

## Fresh verification

- `cargo test --workspace --exclude moleapi-desktop --no-default-features`: **176 passed, 0 failed, 20 ignored**, 41 suites including doc tests. Log `/tmp/moleapi-mcp-backend-workspace-tests.log`.
- `cargo test -p moleapi-server --test mcp --no-default-features -- --include-ignored`: **11 passed, 0 failed**, including real official SDK HTTP and subprocess STDIO.
- `cargo clippy --workspace --exclude moleapi-desktop --all-targets --no-default-features -- -D warnings`: passed.
- `cargo fmt --all --check`, `git diff --check`: passed.
- `cargo build -p moleapi-server --example mcp_fixture --no-default-features`: passed. Fixture executable `target/debug/examples/mcp_fixture`; `--stdio`, `--oversize-stdio`, or HTTP using `MOLEAPI_MCP_FIXTURE_LISTEN` (default127.0.0.1:18901). Root's distinct18903 fixture process remains root-owned; tests use own ephemeral sockets and reserved18902 only for blocked-policy negative input.

Tests cover actual capability discovery/version, tool success/tool error/protocol error data, schema failure, resource/template/prompt methods, subscriptions, progress/list changes, manual sampling/form elicitation/roots/error replies, request Stop/disconnect, Basic/Bearer/custom header/scoped variables/private masking, TLS verification and opt-out, hosted launch denial/owner privacy/logout/workspace deletion, malformed/redirect/oversize/depth/pagination bounds, content blocks without secondary fetch, cleared STDIO HOME/explicit env/PID reaped before close returns and raw oversized STDIO before parsing.

Desktop GUI verification remains unavailable due to known missing WebKitGTK; native offline router/STDIO execution is verified. Default/full host export and UI/browser verification belong to root Task2. No further backend source changes after this freeze except review-directed fixes.

## Round 1 independent-review fixes — frozen for re-review

All three findings in `tasks/implementation/mcp-backend-review.md` were reproduced before changes and fixed within their scope:

1. Templated MCP connection fields: the real STDIO test now uses scoped templates in executable, argv, env, selected name and URI and checks retained originals. Before the fix it returned400 `Pre scripts cannot change the live protocol`. The server guard now compares MCP protocol kind and transport identity, allowing field resolution while preserving kind/transport checks and the existing hosted resolved-executable allowlist.
2. Work quota: focused unit admits exactly512 tiny cancel commands, rejects513, and confirms a full queue consumes no count. It failed before the fix. Admission now checks the counter before enqueue; counting/bytes updates still occur only after successful enqueue.
3. Privacy: a real official SDK tool reflects known private text in custom notification method/object keys and private numeric173/booleantrue values. Before the fix keys/scalars remained raw. Notification method labels and JSON keys now use the existing private mask; matched non-string scalars are wholly withheld as `[REDACTED]`. If two keys collide after masking, the entire object is `[REDACTED: object key collision]`, avoiding overwrite or original-key leakage. A collision unit failed before and passes after. Capability extraction reports withholding errors instead of unwrapping a withheld object.

Fresh round1 checks:

- Rust workspace: **180 passed, 0 failed, 20 ignored**, 41 suites. `/tmp/moleapi-mcp-backend-review-fixes-tests.log`.
- Explicit real-SDK MCP tests: **12 passed, 0 failed** (including the templated STDIO regression and private method/key/scalar regression).
- Focused quota/collision units: **2 passed, 0 failed** after their recorded pre-fix failures.
- Strict workspace all-target Clippy: passed; `git diff --check`: passed.
- Backend Rust files formatted. The last whole-workspace fmt check identified concurrent root-owned changes at `crates/formats/src/redact.rs:439/449`; root was notified to format those export changes. No changes to root frontend/formats sources or owned fixture/API processes in this fix round.

No source commits or pushes. Backend source is frozen again for independent re-review; DTO/route contracts and intentional transport exclusions remain unchanged.
