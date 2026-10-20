# Shared MCP client implementation plan

> **For agentic workers:** Use executing-plans inline and verification-before-completion; one independent final source review.

**Goal:** Shared editors/admins configure/save/import/export MCP HTTP/STDIO settings, explicitly connect and browse/call/cancel/answer through creator-bound member sessions without runtime canonical writes.
**Architecture:** Admit saved MCP and its3 typed commands in existing member transport/audit. Add saved request/revision-bound config export returning only the selected canonical request through existing native redaction. Extract controlled McpClient and McpConfigEditor, preserving owned wrappers; shared runtime copies saved config and hides connection/resource mutation.
**Tech Stack:** Existing official rmcp3.5 HTTP/STDIO SDK, checked transport, mature JSON/JSONSchema/JSONC, React/Radix/CodeMirror.
**Spec:** docs/specs/mcp-client.md and docs/specs/workspace-collaboration.md shared MCP subsection.

## Constraints/interfaces

- Connect uses saved request_id/revision, canonical owner auth/env/pre-script, empty member locals. HTTP and STDIO reuse Rust network/launch allowlist; no implicit launch from Save/import. STDIO executable allowlist remains admin-configured and host environment cleared.
- Commands exactly mcp_request/mcp_callback/mcp_cancel. Record only action/session actor audit, never method/name/args/URI/callback/source/credential. Creator/member/revision/credential fences and lifecycle reap apply.
- /mcp/config/export takes saved request_id/expected_revision/include_secrets and returns only RequestSpec. Reuse moleapi native export redaction across canonical owner workspace, no value resolution/transport start; default auth omitted before Host encoding. Explicit include_secrets preserves original raw settings. Final credential→member/revision fence with owned operation future lifetime.
- McpClient consumes scope/request/onChange/dark/isCurrent/disabled and optional McpChannel; source has no network channel. Runtime copies saved request, exposes capabilities/requests/results/events/callbacks/cancel/refresh/read/subscribe/prompts and local JSON; connection/Host config mutation absent. No response-driven canonical changes.
- McpConfigEditor uses controlled request/guard/onChange/readSavedRequest and optional confirm adapter. Host import stages exact original entry, preserves argument draft, never auto-connects. Shared export requires clean saved context and never implicitly saves; owned wrapper retains established save/export route. Confirm/source/file/export/keyboard/reply callbacks bind mounted/scope/session/epoch/current request and admission.
- UI uses existing tokens, labelled controls, explicit errors and reduced-motion defaults; no new animation on frequent operations. Native hosted sync/installed distribution/full remaining matrix remain integration work.

## Tasks

1. Backend:
   - [x] RED saved member MCP whitelist, config export route and creator/role/stale/wrong-command policy.
   - [x] Admit3 typed commands/action-only audit and saved selected-request redacted/full export under auth/member/revision fences.
   - [x] Real official SDK HTTP discovery/tools/resources/templates/prompts/progress/notify/manual callbacks/cancel/errors/owner collision/privacy/source preservation and revision/role/logout lifecycle; explicit allowed STDIO env/reap plus default denial.
2. Frontend:
   - [x] RED shared create/Host config stage/explicit Save/Connect/local run/callback/cancel flow.
   - [x] Controlled client/config editor extraction and owned compatibility; shared config source/runtime separation and clean saved export.
   - [x] Guard tests for late source confirmation/export, retained callback/admission and new-session replies, keyboard, failed admission retry and no canonical runtime writes.
3. Verify/deliver:
   - [x] Full frontend/affected backend/typecheck/production, strict server Clippy/package fmt and embedded build; fixture build before browser.
   - [x] Independent source review and actual bilingual390/1280 HTTP and allowed STDIO/config/read/call/callback/Stop/export/image-data/narrow/a11y checks with native boundary explicit.
   - [x] Coverage/map/ledger/screenshots, explicit stage/commit/push, preserved PR update and exact-head CI/Docker.

## Review focus

- Callback reply/cancel tickets retained after credential/member/request/revision changes must not hit a new session.
- Config import confirmation cannot overwrite a changed draft or closed/reopened dialog; source is retained exactly.
- Default Host export must screen credentials/unknown original source fields and scope-derived values without returning unrelated workspace resources.
- SDK resources/links remain data; no browser/host filesystem/URL side effects from capability/result rendering.
- Allowed STDIO starts only on explicit Connect, clears ambient environment and reaps on member/credential/version loss.

## Ledger

- Previous turn progress:8ebd71c shared SOAP pushed with729 frontend/core47/real SOAP and browser evidence. Existing worktree clean; root mise.toml untouched.
- CI39497306259 and Docker39497306247 confirmed live now; source/worker passed, native images running. No timeout-driven restart. Whole28-module/395-capability objective stays active.
- Preflight: MCP SDK commands/transport and controlled content renderer already exist; McpWorkbench and Host dialog own context must be extracted before sharing. Export can reuse existing native format redaction and return only selected RequestSpec; HTTP/STDIO connection and credential fences remain Rust-owned.

- Backend implemented: saved MCP admission and exactly3 commands, content-free actor audits, selected canonical config export and credential/member/revision fencing. Initial saved/session400 and export404 RED recorded in /tmp/moleapi-shared-mcp-red.log. Configured official shared HTTP/allowed STDIO suite5/5 passes in /tmp/moleapi-shared-mcp-real.log; affected default server suite66 passes with4 configured fixture ignores in /tmp/moleapi-shared-mcp-full-server.log.
- Frontend extraction and shared integration implemented. Previous execution evidence:744 tests/137 files and TypeScript pass; retained import/export/source/session/callback guards and owned wrappers covered. STDIO switch after HTTP Auth tab reproduced hidden mounted HTTP controls; regression now passes with fallback to Scripts and relevant timeout settings.
- Ruling: add a dedicated owned MCP export endpoint as well as the member endpoint — customizable Host names become JSON keys and must pass through native workspace privacy screening before encoding — cost if wrong: one additional owned route, preserving existing save-before-export behavior.
- Independent final source review completed by shared_report_export_review with no concrete findings. Its exclusions (runtime gates/browser/installed file chooser/long-run reap) remain separately tracked; no installed/native or long-run claim.
- Current turn verification: production frontend build passes with38 chunks/no static import cycles; owned configured MCP fixture tests13/13 pass; strict server all-target Clippy passes. Embedded server build is running, followed by actual browser checks.

- Current final gates: owned real MCP13/13, production38 chunks, strict all-target Clippy, package fmt and embedded server build exit0. The preceding744 frontend/137 files, TypeScript and affected server66/4 ignores apply to the unchanged implementation;5 shared MCP configured tests include actual STDIO revision reap.
- Browser: actual member HTTP source/import/CAS/default+full Host downloads, canonical variables, tool/resources/templates/prompts/subscriptions/progress/list notifications/manual roots/sampling/elicitation/Stop and inert content; all leave canonical runtime data/revision unchanged. Allowed shared STDIO import/explicit Save/Connect clears ambient/HOME, screens private imported env and reaps actual PID on Disconnect. Owned dirty arguments auto-save before actual screened Host export passes. Named QA session uses isolated workspaces; no user production service touched.
- Ruling: imported Host env values are private by default, so the browser asserts screened response rather than exposing the owner value — explicit unmarked owner-env resolution is proven in the real backend fixture — cost if wrong: browser evidence cannot independently inspect the private value, intentionally withheld.
- Callback/browser harness waits initially selected refresh or older result; correct completed tool cursor confirms real replies. Subscription fixture returns an empty object rather than an ok field. Neither is a product change.
- Final UI: en/zh390 and1280 no page overflow, external resource links never fetched, no browser errors; scoped light a11y0 violations/0 incomplete, dark0 violations/1 incomplete line-number contrast. Screenshots inspected and saved under docs/images/workspace-collaboration-mcp-{en,zh}-390.png. Native chooser/installed app/long-run reap/full platform database integration remain explicit boundaries; full28-module395-capability goal stays active.

- Delivery: coverage/map/status/screenshots updated. Commit/push and exact-head CI/Docker observation follow; no local Docker build or new registry publication is claimed.

- Delivery: committed/pushed1dc84d0; PR41 appended without replacing prior body. Exact-head Docker39502988936 passed native amd64/arm64 and container smoke; manifest skipped on PR (no registry publish). Application39502988938 source/database passed, check still running at latest observation. QA browser/app/fixtures stopped; root mise.toml remains untouched.
