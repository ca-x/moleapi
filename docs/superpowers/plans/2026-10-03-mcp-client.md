# MCP Client Implementation Plan

Authority: docs/specs/mcp-client.md and full capability matrix. User authorizes ongoing implementation; features before build refinement.

One fresh Rust implementer after SOAP source/review/commit complete. Root owns web/formats/docs. Fresh backend/UI reviewers follow; never overlapping Rust implementers.

## Task 1 — Official SDK client and bounded sessions

- Verify rmcp3.5 features/protocols/custom HTTP adapter/child process/callback contracts. Send root DTO/session commands/events before frontend work.
- Add saved MCP config/incomplete draft rules, official SDK streamable HTTP via shared checked/pinned client and explicitly authorized STDIO options; no custom parser/shell/ambient environment.
- Connect/init/capabilities, tools/resources/templates/prompts, results/progress/notifications/list-changed, bounded pagination/events/work and owner lifecycle cancellation/child cleanup.
- Add client callbacks manual sampling/elicitation/roots, exact reply/error/timeout quotas; advertise implemented capabilities only.
- Mature SDK HTTP/STDIO fixture/tests and auth/TLS/network/owner/error/privacy/limits checks; report/freeze.

## Task 2 — Dedicated UI/config/interchange

- Lazy Radix/CodeMirror client, transport configuration/discovery/search/schema/arguments/call/read/prompt/result/notification/callback panes and explicit connect/run/stop.
- Strict host config import/export and original shared sources, safe default and exact explicit private config export, unsupported foreign guard.
- Scope/draft/async/callback/owner tests and real agent-browser HTTP/STDIO/config/keyboard/narrow/light-dark acceptance.
- Independent backend/UI review fixes, fresh workspace/front/strict/embedded worker/runtime checks, docs/commit/proxy push. Continue remaining protocol and shared capability matrix; builds/main-final integration only when features complete.

Current MCP client tasks are implemented/verified with independent review. Publishing/OAuth/Apps/remaining protocol/full matrix work remains separate; no full coverage claim.
