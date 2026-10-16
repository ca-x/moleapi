# ODS Data source implementation plan

> **For agentic workers:** Execute one independently testable task at a time. Use the existing file worker and DataFusion path.

**Goal:** Query explicitly selected local or checked remote ODS sheets as Data sources.

**Architecture:** Add an additive `DataFileFormat::Ods`; reuse the XLSX ZIP preflight and Arrow conversion with Calamine's ODS reader. Keep source serialization, network admission, worker budgets and protocol events unchanged.

**Tech Stack:** Rust/Calamine/Arrow/DataFusion, React/TypeScript, existing file worker and hosted protocol session.

**Spec:** `docs/specs/data-ods-source.md`

## Task 1: Bounded ODS parsing

**Files:** `crates/core/src/data.rs`, `crates/data/src/xlsx.rs`, `crates/data/src/files.rs`, `crates/data/src/files_tests.rs`, `crates/data/tests/fixtures/workbook.ods`.

- [ ] Add an independent ODS workbook fixture and a failing real SQL test for first/exact sheet, cached values, exact integer, floating text, Boolean and null.
- [ ] Add `Ods` to the core format and validate worksheet names for either spreadsheet format. Reuse ZIP preflight and Arrow conversion; keep physical row 1 semantics and existing budgets.
- [ ] Add malformed/duplicate/blank/oversized and default-backup tests. Run `cargo test --locked -p moleapi-data` and focused core/formats suites.

## Task 2: Source UI and hosted wire

**Files:** `web/src/features/data/DataSourceFields.tsx`, `web/src/features/data/types.ts`, `web/src/shared/i18n/{en,zh-CN}.json`, `crates/server/tests/data_client.rs`.

- [ ] Show ODS selection, filename inference, sheet/header controls and cached-formula help in both languages. Add a focused component test.
- [ ] Query a checked remote ODS fixture through an owned hosted protocol session, including wrong-sheet rejection and private source backup. Run server test and frontend tests/build.

## Task 3: Real UI and release evidence

- [ ] Save/reload a local ODS request and run SQL in a 390px Chinese/English browser; verify no horizontal overflow and exact typed result.
- [ ] Run Rust format, strict Clippy, frontend production build and relevant suites. Update coverage/status and trigger branch CI. Record platform package status separately.

## Progress, 2026-10-17

The reproducible ODS fixture, bounded parser, local and DNS-pinned remote SQL, private/full native backup, inferred file picker, worksheet controls and bilingual help are implemented. Focused and full relevant suites passed: core Data (2), Data (13), formats (54), hosted Data client (22), frontend (332), strict all-target Clippy, rustfmt and production frontend/embedded server builds. A real saved/reloaded 390px Chinese/English local upload returned `2 / 2.30 / café`; the result was saved as a private dataset and selected in testing without document overflow. Branch CI and current package gates remain.
