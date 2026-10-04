# MCP client contract

Authority: full researched API coverage and functional Apifox/Postman features, mature Rust crates, modular code, standalone desktop/server, features before packaging. This module follows SOAP; project-to-MCP publishing and shared OAuth are distinct later capabilities in the full matrix.

## Objective and research

A dedicated MCP debugging client must connect, initialize/negotiate, inspect capabilities, call tools, read resources/templates, get prompts, show results/notifications/errors, and disconnect/cancel with saved original request settings. It is a real official-SDK client, not raw HTTP JSON controls.

Official captured references: Postman mcp-requests/create, interact, manage, oauth-debugger and export-mcp-server-config under docs/references/fetch-manifest.json; full research bodies are in original checkout docs/references/raw. Postman offers streamable HTTP and STDIO, tools/resources/prompts, notifications, manual client sampling/elicitation/roots, auth/config export. Apifox references docs.apifox.com/mcp.md and the catalog rows mcp-001..007. Latest SDK metadata already records rmcp3.5.0, rust1.88; root used agent-browser to confirm docs.rs transport StreamableHttpClient custom backend interface.

## Mature library and boundaries

Use official rmcp3.5.0 SDK typed messages, client handler, initialization/version/capabilities, streamable transport, JSONRPC multiplexing/cancellation, resource subscriptions and child process transport. Mature serde/JSONSchema validation own argument schema checks; no custom MCP JSONRPC/SSE framing parser. Verify supported SDK protocol versions and transports before presenting version options. Preserve MCP protocol errors and negotiated version rather than disguising as HTTP results. Never claim MCP Apps UI, legacy/stateless protocol modes, OAuth debugger, server publishing or AI execution that is unimplemented.

Reuse checked/pinned shared HTTP network policy through SDK StreamableHttpClient adapter. SDK builtin reqwest may be a different version and must not bypass shared DNS/IP/redirect policy. HTTP endpoints, redirects and any separate SSE/dependency URLs require bounded checks; no host credentials/ambient proxy/env leakage. Unknown endpoint/OAuth discovery never automatically opens a browser or contacts another origin without a dedicated user action. Auth Basic/Bearer/custom headers/TLS/timeout/variables apply to actual HTTP transport.

STDIO uses mature SDK child-process transport with explicitly separated command/arguments/env (never shell concatenation), cleared host environment, caller-declared environment and bounded stderr/output/time/work. STDIO works in independent native client; self-hosted server defaults to HTTP-only and may opt into administrator-configured allowed executables. A remotely synced saved configuration never starts automatically. Provide an explicit connect/launch control and visible command. Server defaults/launch authorization must be enforced in Rust, not only hidden controls. Do not add global unbounded OS process access.

## Model, sessions and routes

Dedicated saved Protocol MCP metadata must include transport HTTP/STDIO, explicit command/arguments/env, selected operation and original JSON argument/message draft plus source preservation. Incomplete drafts save; connection/invocation performs strict bounded validation. Top-level RequestSpec.url is HTTP endpoint and shared auth/settings apply; STDIO command never masquerades as URL. Root needs exact DTO/session commands/events early, before UI implementation.

Use existing owner-scoped protocol sessions with initialize/capability/results/notifications/callback/status event payloads. Connect must return promptly; UI consumes events and can Stop during DNS/connect/initialize/long tool calls. Poll cursors/retention explicit gaps, connection/call quotas and bounded JSON depth/node/bytes/capabilities/pagination/notifications; pagination must terminate or expose an explicit error. SDK handler queues bounded pending sampling/elicitation requests, root responses and explicit accept/deny/cancel or JSON error replies. No automatic LLM execution or filesystem roots discovery. Advertise only callback capabilities actually implemented.

Methods: list/call tools (structuredContent/content/isError); list/read resources and list resource templates; list/get prompts with required string arguments; optional resource subscription when negotiated; capability refresh on list-changed notifications; logging/progress/cancel/disconnect. Read resource URI is sent to MCP server through SDK; do not independently fetch it as a URL/file. Results retain text, JSON, image/audio/base64/resource-link/embedded resource content in bounded event bodies. Root will render mature viewers with explicit downloads; server-provided URLs/HTML are not automatically executed or embedded.

Owner/workspace/request deletion/logout/account switches cancel SDK tasks and reap children; stale connect admission cannot survive owner expiry. Session results and callback responses are transient and not silently added to shared data/history. Captured private values must stay out of status/error labels, saved auto-generated names/default exports and shared updates. Original unresolved command/env/auth/arguments remain only in draft/source and explicit full exports. Foreign exports reject unsupported MCP configs rather than lose them.

## UI and config interchange

Lazy React/Radix/CodeMirror module with transport picker, STDIO command/arguments/env, Load Capabilities/connect/disconnect, negotiated protocol and server identity, searchable Tools/Resources/Templates/Prompts, schema/definition view, JSON request editor, Run/Stop, response content/notifications/console/client callback panes. Preserve user-edited drafts on capability selection (use existing guarded replacement when needed); scope-fence late capability/result/source/config actions by owner/workspace/request/environment. Refresh capabilities does not overwrite original arguments unexpectedly.

Import/export MCP host configs uses strict JSON and mature serde parsing: server entry name, command/args/env or HTTP URL/headers. Support templates/variables without resolving private values into shared saved configs. Export default redacts sensitive fields and execution-derived private values; explicit include-secrets preserves originals. Local STDIO configuration export/import must not start a subprocess or perform discovery by itself.

Use existing UI design skills ui-ux-pro-max/emil-design-eng/animate and mature components. Frequently used editor/navigation state changes remain immediate; shared modal transitions/reduced motion apply. Responsive panes/scrolling, keyboard Run, close dialogs/return focus, dark/light contrast and actionable errors. Server execution errors are not mistaken for MoleAPI login expiry.

## Structure and conventions

Core/protocols/server modules and types/tests own SDK/transport/sessions; web/src/features/mcp owns UI and helpers; shared types/model/selector only minimal integration. Exact major implementation boundaries documented in plan. Rust Result typed contextual errors, input bounds before work; protocol configs boxed when enum size warrants. Original definitions/drafts not reconstructed from runtime results.

Commands:
- Rust: cargo test --workspace --exclude moleapi-desktop --no-default-features; cargo test -p moleapi-server --test mcp -- --include-ignored
- Strict: cargo clippy --workspace --exclude moleapi-desktop --all-targets --no-default-features -- -D warnings; cargo fmt --all --check; git diff --check
- Web: npm --prefix web test; npm --prefix web run typecheck; npm --prefix web run build
- Runtime: cargo build -p moleapi-server; copy executable to a unique /tmp QA path before test feature rebuilds; agent-browser named sessions.

Always preserve scoped secrets/raw configuration, bound work, use mature crates, meaningful lifecycle/export/integration tests and independent review. Existing authorization includes required dependency/code changes and proxy feature pushes. Never local Docker, packaging/Actions refinement during feature completion, secrets in commits/output, shell-command construction or deletion of original main untracked scaffolds.

## Acceptance

Real official-rmcp fixture servers for HTTP and STDIO, initialization/version/capability discovery, tool valid/error results and progress, JSONSchema invalid args, resource/template/prompt methods, notifications/list-changed/subscription where supported, callback sampling/elicitation accept/deny/cancel without LLM, precise Stop/timeout/child reaping, Basic/Bearer/TLS/scoped variables, owner/network/redirect/source/config/default-versus-explicit-export/argument privacy and malformed/oversize/depth/pagination limits. Root actual browser transport/discovery/selection/edit/invocation/results/callback/config/keyboard/narrow/light-dark flows, independent backend/UI reviews, fresh strict/source/runtime verification and proxy commit/push. Packaging follows all feature work.
