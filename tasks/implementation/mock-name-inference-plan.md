# Mock field-name inference implementation plan

> Implement in the isolated `feat/mock-field-rules` worktree. Keep the complete `mock-003` row open after this slice.

**Goal:** Add opt-in deterministic name suggestions to the existing bounded Mock Schema generator, with one saved editor toggle shared by inline and linked sources.

**Spec:** `docs/specs/mock-name-inference.md`.

## Task 1: Model and core generator

- [x] Add a default-false example flag and source-preserving validation/backups; old serialized examples remain unchanged.
- [x] Add core cases for aliases, precedence, optional fields, invalid suggestions, `$ref` name isolation and composition; the first public-generator and model cases failed before implementation.
- [x] Thread a name context through the existing generator and cache, preserving all budgets and mature JSON Schema validation. Keep validator cache keyed only by original Schema node.
- [x] Run focused core/formats tests, strict Clippy and rustfmt; commit.

## Task 2: Shared runtime and editor

- [x] Pass the saved flag through direct, routed, public and preview paths without new endpoints or privacy surfaces. Add real HTTP regression cases including disabled legacy behavior.
- [x] Expose a bilingual opt-in toggle next to the selected Schema source, retaining an unsaved draft across editor open/close and old static body behavior.
- [x] Run focused server/frontend suites and a saved/reloaded 390px browser flow with an actual generated direct/routed response; commit.

## Task 3: Acceptance and integration

- [x] Run full relevant Rust and frontend suites, frontend then embedded build, strict Clippy, formatting and diff check. Document exact supported names and remaining `mock-003` scope.
- [x] Request independent read-only review, fix Critical/Important findings and re-review.
- [ ] Push a draft PR stacked on the composed Mock batch; run exact-head three-database, Docker and native package CI and installed package smoke. Merge verified batches in dependency order.
