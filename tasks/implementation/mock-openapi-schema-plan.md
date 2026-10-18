# Linked OpenAPI Mock Schema Implementation Plan

> Implement in the isolated `feat/mock-openapi-schema` worktree, stacked on `feat/mock-schema-response`. Each task closes one testable contract; do not claim full `mock-002` or `mock-003` coverage.

**Goal:** Let a saved HTTP example generate Mock JSON from a selected response Schema in its workspace's saved OpenAPI 3.0/3.1 source.

**Architecture:** Store an additive link (specification ID plus exact response-schema pointer), normalize the selected schema and its local component refs into the existing bounded generator, and resolve it from the current owner-scoped workspace at call time. Reuse the direct/routed/public pipeline, while binding public capabilities to linked source bytes.

**Tech stack:** Rust/Serde/YAML/JSON Schema, Axum/SeaORM, React/TypeScript/YAML/Vitest.

**Spec:** `docs/specs/mock-openapi-schema-link.md`.

**Constraints:** No remote retrieval. Source <=5 MiB, traversal <=64 levels/10,000 nodes, normalized schema <=256 KiB, generated body <=1 MiB, candidate checks <=256. Reject unsupported constructs with a path. Default backup must not expose private source or keep a broken link. Preview and public calls must use current saved owner data.

## Review focus

- An imported operation with no `operationId` must still be selectable by exact path/method/status/media pointer.
- A response object `$ref` and nested component schema refs must remain in the same saved source; external/cyclic refs reject.
- Editing the linked specification must stale an existing public share before serving new bytes, including a delayed call.
- A source-only secret in an authored example/default must not appear in a default backup or error response.
- Workspace validation must not parse a 5 MiB YAML source once per example.

## Task 1: Saved link model and core response-schema adapter

**Files:** `crates/core/src/models.rs`, `crates/core/src/validation.rs`, new `crates/core/src/mock_openapi_schema.rs`, `crates/core/Cargo.toml`, core tests.

**Interfaces:** `MockSchemaLink { specification_id: String, pointer: String }`; `OpenapiMockIndex::parse(spec: &Specification) -> Result<Self>` and `OpenapiMockIndex::schema(&self, pointer: &str) -> Result<String>`. The adapter returns the existing generator's accepted schema JSON, preserving local refs as top-level `$defs`.

- [ ] Add old JSON compatibility, JSON/YAML 3.0/3.1, missing/duplicate operation IDs, exact pointer/status/media, response/schema refs, nullable/formats, external/cycle/budget and source-change tests.
- [ ] Implement bounded parse/normalization and save validation, caching one parsed index per linked specification in a workspace validation pass. Reject simultaneous inline schema/link.
- [ ] Run core tests, strict core Clippy and scoped formatting; commit.

## Task 2: Current-source Mock generation and public capability fence

**Files:** `crates/server/src/{lib,mock,mock_public}.rs`, `crates/server/tests/mock.rs`, `crates/server/tests/mock_public.rs`, database test if needed.

**Interfaces:** `serve_selected` resolves a linked example from its current workspace snapshot in the eight-slot blocking generation path. `request_digest` accepts the workspace and includes linked specification source only for requests that use it.

- [ ] Write real direct/routed/public HTTP tests for current linked output, owner denial, script precedence, source edit/missing pointer, generic errors, stale/rotated public URL and cross-instance delayed-source edit.
- [ ] Implement generation and digest fencing without static-body fallback; reuse existing response/header/log/delay behavior.
- [ ] Expose owner-scoped validation for a selected pointer before the UI writes it; return method/status/media only and fence revision changes.
- [ ] Run focused server suites and strict server Clippy; commit.

## Task 3: Backup and explicit UI selection

**Files:** `crates/formats/src/lib.rs`, `crates/formats/src/redact.rs`, format tests, `web/src/shared/types.ts`, `web/src/features/requests/{RequestEditor,ExamplesEditor}.tsx`, UI tests, English/Chinese catalogs.

- [ ] Add default/full backup and external rejection tests; add UI tests for operation without ID, JSON/YAML source, mode switching and retained static body.
- [ ] Pass saved specifications to the examples editor, list exact response pointers and source labels, validate a choice through the owner API before patching the draft, and keep the selection usable in the mobile request pane. Compare specification ID and pointer together. Clear linked mode in privacy backups and reject unsupported external mappings.
- [ ] Run formats/frontend suites, production build and actual bilingual 390px save/reload/preview/real-call checks; commit.

## Task 4: Final evidence and shipping

**Files:** `docs/MOCK-OPENAPI-SCHEMA-COVERAGE.md`, `docs/IMPLEMENTATION-STATUS.md`, this plan.

- [ ] Run full relevant Rust/frontend tests, strict Clippy, frontend then embedded server build, formatting and diff check.
- [ ] Request independent read-only review; fix Critical/Important findings and re-review.
- [ ] Push a draft PR, run exact-head three-database/Docker/native CI and inspect actual installed packages before merging a verified batch. Preserve the full 395-item goal.
