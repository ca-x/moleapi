# Independent MCP UI and formats review

Reviewed the working-tree MCP UI, minimal shared integration, and formats redaction against `docs/specs/mcp-client.md` and `docs/superpowers/plans/2026-10-03-mcp-client.md`, baseline `6ea9b18`. Scope reflects source inspected on 2026-10-03 around 21:39–21:46 UTC; later implementation fixes require another pass. Backend implementation is outside this review. No production sources were edited and no QA servers/browser sessions were manipulated.

## Findings

1. **P1 — Original imported credentials survive default exports after editing current credentials.** `crates/formats/src/redact.rs:63` screens `config_source` using secrets captured only from current canonical rows and arguments (`:365`, `:374`). The importer marks every imported env/header row private (`web/src/features/mcp/config.ts:32`, `:37`), but a preserved source such as `{"command":"/usr/bin/node","env":{"TENANT":"old-private"}}` has no row secret flag. Change the current TENANT value to `new-private`; `redact_payload` leaves source TENANT unchanged and `screen_json` cannot recognize the old value. Default native exports disclose `old-private`. The same issue affects original nonstandard header keys and duplicates of original sensitive CLI argument values in unknown source fields. Capture private values from preserved source independently of current edited fields, and redact the imported private source maps. Preserve exact originals only for include-secrets exports.

2. **P1 — Canonical MCP transport fields bypass known-value privacy screening.** `crates/formats/src/redact.rs:48` screens command, URI, args, and JSON sources, but leaves `request.url`, nonsensitive header/query values, and nonsensitive MCP env values with only key/flag-based screening (`:31`, `:45`, `:50`). A known bearer token reused in an HTTP URL path remains in a default native export; `hostConfig` then rebuilds the host URL and headers directly from these unscreened fields (`web/src/features/mcp/config.ts:53`, `:60`), so the default Host export also exposes it. Apply the known-value screen to these transport strings without resolving variables. Include-secrets must retain their exact originals.

3. **P2 — Duplicate server keys bind preserved source to a different server definition.** `web/src/features/mcp/config.ts:24` uses `findNodeAtLocation`, which selects the first duplicate property, whereas `JSON.parse` at `:16` selects the last. For `{"mcpServers":{"fixture":{"url":"https://first.invalid","extension":{"old":true}},"fixture":{"url":"https://second.invalid","extension":{"new":true}}}}`, canonical settings connect to second.invalid, but `config_source` retains first.invalid and its old extension. Explicit Host export mixes the last URL with the first entry's unknown fields and loses the selected definition. Reject duplicate object/server names or ensure node selection agrees with semantic parsing.

## Verification and coverage

- `npm --prefix web test -- src/features/mcp`: passed, 5 files / 9 tests.
- `cargo test -p moleapi-formats --test interchange mcp_native_and_host_sources_roundtrip_and_default_exports_screen_credentials --no-default-features`: passed, 1 test.
- `git diff --check`: passed before creating this report.
- A temporary standalone Rust diagnostic linked to the freshly built formats library exercised the production `export(..., false)` and printed `old imported private value remains: true` and `known current private value remains in URL: true`. Its source and executable were confined to a temporary directory and removed afterward. Only synthetic values were used.
- A Node diagnostic using the installed jsonc-parser confirmed duplicate-key parsing returns second.invalid while preserved node slicing returns first.invalid.

Read coverage: config import/export and dialog guards; workbench capability/draft selection and session identity; callbacks and manual replies; keyboard dispatcher integration; request transport settings; event metadata/content; content/media preview; CSS layout; default versus explicit native formats export; foreign-format rejection. No automatic process launch from import, AI execution, resource-link fetch, HTML embedding, or SVG preview was found. Existing tests exercise stale dialog export/confirmation, late callback owner changes, and media safety. Runtime browser QA remains the root agent's responsibility. SDK limitations (OAuth, Apps, stateless guarantees, template substitution, bare STDIO command admission) are accepted explicit boundaries.

## Completion ledger

1. UI/formats source review: done; three verified findings delivered to root.
2. Scoped frontend/interchange checks: done; passing tests do not cover the reported privacy/source regressions.
3. Fixes and post-fix verification: remaining with root; review was read-only.
