# Schema-driven Mock Response Implementation Plan

> Implement sequentially in this isolated branch. Every task ends with a focused test and reviewable commit.

**Goal:** Generate bounded, deterministic JSON responses from a saved example's explicit JSON Schema while preserving static Mock behavior.

**Architecture:** Store optional `mock_schema` on `Example`, validate it through one core generator, and call that generator in the existing selected-example path before scripts and delay. The existing direct, routed and public routes retain selection, authorization and logging. External exporters reject an unrepresentable schema.

**Tech stack:** Rust/Serde/JSON Schema validator, Axum/SeaORM, React/TypeScript/Vitest.

**Spec:** `docs/specs/mock-schema-response.md`.

**Constraints:** No network/file schema resolution; 256 KiB input, depth 64, 10,000 visited nodes, 128 values per enum/examples list, 256 candidate checks, 1 MiB generated output. Existing examples with no schema retain exact bytes. Do not mark `mock-002` or `mock-003` complete from this slice.

## Review focus

- Cyclic or repeated `$ref` must end within the budget and never use an external resolver.
- An authored default/example/enum that violates the surrounding type or bounds must not escape to the response.
- `oneOf`, `allOf` and other unsupported keywords must reject at save time with a field path.
- Public error text and logs must not disclose private schema bodies.
- Save/reload and full backup preserve the schema; default backup explicitly omits it and incompatible external export rejects it without silent loss.

## Task 1: Add saved schema and the bounded core generator

**Files:** `crates/core/src/models.rs`, `crates/core/src/validation.rs`, new `crates/core/src/mock_schema.rs`, `crates/core/src/lib.rs`, core tests.

**Interface:** `pub fn validate_mock_schema(source: &str) -> anyhow::Result<()>` and `pub fn generate_mock_schema(source: &str) -> anyhow::Result<String>`. `Example.mock_schema: String` uses `serde(default, skip_serializing_if = "String::is_empty")`.

- [ ] Add old JSON and full-backup roundtrip/default-backup omission regressions; invalid schema and size/depth/ref/unsatisfiable schema tests fail before implementation.
- [ ] Implement strict local-reference resolver and generation for the spec subset, with deterministic key order and complete validation of the final JSON against the selected schema.
- [ ] Run `cargo test -p moleapi-core` and scoped rustfmt; commit the task.

## Task 2: Use generated bodies in the existing Mock pipeline

**Files:** `crates/server/src/mock.rs`, `crates/server/tests/mock.rs`, `crates/server/tests/mock_public.rs`.

**Interface:** `serve_selected` calls `generate_mock_schema` when `e.mock_schema` is nonempty, seeds `MockResponseDraft.body`, and supplies `application/json` only when no enabled Content-Type exists.

- [ ] Write real direct/routed/public HTTP tests for schema body, script override, existing static parity, status/headers, delay/revision, owner denial and public generic errors.
- [ ] Implement generation before script without changing routing or selection; ensure public share digest covers the schema through the existing saved-request serialization.
- [ ] Run focused server tests and strict scoped Clippy; commit the task.

## Task 3: Preserve interchange and expose editing

**Files:** `crates/formats/src/lib.rs`, `crates/formats/src/{postman,openapi,har}.rs` constructors, formats tests, `web/src/shared/types.ts`, `web/src/features/requests/ExamplesEditor.tsx`, UI tests and `web/src/shared/i18n/{en,zh-CN}.json`.

- [ ] Add external-format rejection/constructor tests and UI mode/save/preview tests; verify they fail before implementation.
- [ ] Reject unsupported exports explicitly; initialize imported examples with empty schema. Add a schema editor that retains static body when toggled and uses the existing saved Mock preview route for generated output.
- [ ] Run formats and UI tests, bilingual 390px actual browser save/reload/real-response check, then commit the task.

## Task 4: Final evidence and integration

**Files:** new `docs/MOCK-SCHEMA-RESPONSE-COVERAGE.md`, `docs/IMPLEMENTATION-STATUS.md`, this plan.

- [ ] Run full relevant Rust/frontend suites, strict Clippy, frontend then embedded-server build, formatting and diff check.
- [ ] Record exact commit, representative generated values and public privacy evidence; request independent review and address findings.
- [ ] Push a draft PR, run exact-head three-database, Docker and native builds, then inspect actual package smoke. Merge only after the required gates pass; keep the full matrix goal open.
