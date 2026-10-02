> 状态：调研前的初始草稿。用户要求完整获取两款产品功能后重新确定范围，此草稿不再是完整实施规格。新范围见 FEATURE-MATRIX.md 和 CAPABILITY-MAP.md。

# MoleAPI 0.1 specification

A self-hostable HTTP API workbench inspired by the public Apifox page retrieved with agent-browser on 2026-10-02. Build a working product, not a landing page. Reference Raindrop's Axum, SQLite, embedded Vite assets, configuration and graceful shutdown patterns without copying its unrelated RSS features.

## Capability map
| Module | Responsibility | Dependencies |
| --- | --- | --- |
| request-engine | HTTP, interpolation, auth, response limits, assertions | — |
| storage-server | SQLite, account isolation, collections, history, revision checks, embedded UI | request-engine |
| workbench | React/Radix/CodeMirror workspace, import/export, docs, examples, runner | storage-server API |
| desktop-sync | Standalone Tauri with local SQLite, same API through IPC, optional hosted sync | storage-server |
| distribution | Server binary/Docker; Linux deb/rpm/AppImage, macOS dmg, Windows exe/msi | workbench, desktop-sync |
Build: request-engine → storage-server → workbench + desktop-sync → distribution.

## Decisions
- Rust 2024, Axum 0.8, reqwest 0.12, SQLx SQLite (WAL + busy timeout). A single server executable embeds the complete frontend; the Tauri executable embeds the same frontend.
- React + TypeScript + Vite, Radix UI Themes for accessible components, TanStack Query for async state, CodeMirror for local editors, Lucide icons. No CDN scripts/fonts/workers. Use ui-ux-pro-max, emil-design-eng, animate.
- Native IPC dispatches the shared Axum API in process. Desktop opens no HTTP port and requires no separate server or account.
- Hosted version requires login, Argon2 passwords, expiring hashed bearer sessions, account-isolated workspace and request history. First account uses a setup token. Registration configurable.
- Desktop cloud sync is optional and explicitly enabled; connection tokens stay in native storage. Per-workspace revision compare-and-swap rejects stale writes. Both changed => visible conflict; explicit push/pull resolution. Save old revisions before replacement.
- Private network requests allowed by default on standalone desktop, blocked by default in hosted mode (configurable). Restrict hosted requests to HTTP/S; enforce DNS/IP checks and redirect checks, timeouts, body limits. Avoid hosting an unauthenticated network proxy.
- Workspace snapshot schema version 1; named collections and requests, selected environment, variables and secret markers. Native database file permissions protect local secrets; no claim of encryption at rest.

## Acceptance
1. Start server with database path/bind configuration; open embedded application with no Node runtime or external asset files. Restart preserves data. Missing production UI prevents release build.
2. Login/setup, create/rename/delete workspace and collections; edit/send/save/delete/duplicate requests; choose method, URL, query, headers, JSON/text/form body, bearer/basic auth, timeout/TLS/redirect options. Resolve {{variables}}; missing variables return explicit error.
3. Read real status, duration, size, response headers and JSON/text; copy response/cURL. Persist bounded request history; sensitive authorization not exposed in history.
4. Add/edit environments, including secret variables; export excludes secret values unless explicitly requested. Import MoleAPI JSON, Postman 2.x JSON, OpenAPI 3 JSON/YAML and supported cURL flags; export MoleAPI/Postman/OpenAPI.
5. Declarative assertions for status, duration, substring and JSON pointer; collection runner with per-request failures. Saved response examples and private server mock endpoint. Request descriptions rendered as documents with code samples.
6. Desktop works without login/server using SQLite in app-data. Connect to hosted login, sync workspaces, restart restores connection/base revisions. Offline failure leaves local data intact. Conflicts never silently overwrite.
7. Cross-platform Tauri packaging config and tag release workflow for deb/rpm/AppImage, dmg (arm64/x64), NSIS/MSI. Build server for Linux x64/arm64, macOS x64/arm64, Windows x64. Document artifacts actually verified locally and remaining CI checks.
8. Keyboard shortcuts Ctrl/Cmd+Enter send, Ctrl/Cmd+S save, Ctrl/Cmd+K search. Light/dark themes, mobile browser layout, accessible labels, reduced motion. Frequent operations do not animate; occasional dialogs/toasts have short transitions.

## Commands and structure
`npm --prefix web ci`; `npm --prefix web run build`; `npm --prefix web test`; `cargo test`; `cargo clippy --all-targets -- -D warnings`; `cargo build --release -p moleapi-server`; `npm --prefix web run desktop:build`.
`crates/core`: transport/schema/assertions. `crates/server`: database/auth/router/sync/CLI. `web`: shared UI/tests. `desktop`: Tauri adapter/config. `.github/workflows`: CI/release. `docs`: specification/architecture/reference capture.

## Style and validation
Use typed structs, small modules, snake_case JSON (identical across Rust/TypeScript), explicit errors, no panic on user input. Validate authentication isolation, stale writes, sync divergence, executor limits and export redaction in integration/unit tests; verify built UI with agent-browser against the actual embedded server. Run rustfmt, Clippy, TypeScript, frontend tests and production builds. No credentials in git, no edits to sibling projects. Creating this new project's dependencies, schema and CI is within the requested scope. Publishing artifacts externally requires an explicit user instruction.
