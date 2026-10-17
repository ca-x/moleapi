# Composed Mock Schema Implementation Plan

> Implement sequentially in the isolated `feat/mock-schema-composition` worktree. Each task ends with a focused verification and commit.

**Goal:** Generate deterministic Mock JSON from bounded common composition and tuple Schemas through both inline and linked modes.

**Architecture:** Extend the one core generator's scan and candidate search, validate every final value with the existing JSON Schema engine, then preserve those keywords through the saved OpenAPI adapter. Existing server routes and UI remain consumers of the same contract.

**Tech stack:** Rust/Serde/JSON Schema, Axum, React/TypeScript/Vitest.

**Spec:** `docs/specs/mock-schema-composition.md`.

**Constraints:** 1–16 composition branches, 1–32 prefix items, 256 KiB source, depth 64, 10,000 nodes, 256 candidate checks, 1 MiB output. External retrieval remains forbidden; static examples stay byte exact.

## Review focus

- Branch candidate failures must not mask a later valid branch, and the search must remain within a global budget.
- `oneOf` must reject a value matching two branches even when the first branch alone looks valid.
- `allOf` must preserve required fields from distinct branches without silently discarding conflicting shared properties.
- `prefixItems` plus `items:false` and `maxItems` must not emit forbidden positions.
- Local refs in linked OpenAPI composition must retain top-level `$defs` meaning and public privacy fences.

## Task 1: Core composition and tuple generation

**Files:** `crates/core/src/mock_schema.rs` and its tests, `docs/specs/mock-schema-response.md`.

- [ ] Add failing generator cases for oneOf/anyOf/allOf, nullable type and tuple constraints, including invalid/ambiguous/budget cases.
- [ ] Extend scan and synthesis without weakening the existing reference/candidate/output limits; validate each final candidate against its composed node and the full root.
- [ ] Run core tests, strict core Clippy, scoped rustfmt; commit.

## Task 2: Saved OpenAPI normalization and real Mock calls

**Files:** `crates/core/src/mock_openapi_schema.rs`, `crates/server/tests/{mock,mock_public}.rs`, source coverage docs.

- [ ] Add failing OpenAPI local-ref composition fixtures and actual owner/public HTTP cases; preserve method/status/media and source digest behavior.
- [ ] Normalize nested composition/tuple keywords through the existing adapter; reuse the core generator and current bounded server worker.
- [ ] Run focused core/server suites and strict server Clippy; commit.

## Task 3: Final evidence and review

**Files:** new `docs/MOCK-SCHEMA-COMPOSITION-COVERAGE.md`, `docs/IMPLEMENTATION-STATUS.md`, this plan.

- [ ] Run full relevant Rust/frontend suites, frontend then embedded build, formatting and diff check; perform actual bilingual 390px save/reload/preview where source is authored.
- [ ] Request independent read-only review, fix Critical/Important findings and re-review.
- [ ] Push a draft PR, run exact-head three-database/Docker/native CI, inspect installed package smoke, then merge a verified batch. Keep the 395-item goal open.
