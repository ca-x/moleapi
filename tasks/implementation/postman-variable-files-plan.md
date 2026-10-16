# Postman variable files implementation plan

> **For agentic workers:** Keep parsing and export in shared formats; apply mutations only through the existing owned workspace CAS path.

**Goal:** Import and export Postman environment and globals files within an existing MoleAPI workspace.

**Architecture:** Add a bounded typed converter in `moleapi-formats`, a preview/export server route, and explicit Environments page actions. The frontend applies a preview to its draft only after identity checks, then saves by expected revision. CLI uses the same converter and owned workspace API.

**Tech Stack:** Rust/Serde/Axum/SeaORM, React/TypeScript/Radix, existing workspace/variable models.

**Spec:** `docs/specs/postman-variable-files.md`

## Task 1: Shared variable-file conversion

**Files:** `crates/formats/src/variables.rs`, `crates/formats/src/lib.rs`, `crates/formats/tests/variable_files.rs`.

- [ ] Add failing fixtures for one environment and one globals file, exact UTF-8, order, disabled/secret rows and default/full export.
- [ ] Implement `import_postman_variables(content)` and `export_postman_variables(workspace, scope, include_secrets)` with type/scope/size validation and new Pair IDs. Reuse `validate_variables` and `redact::workspace`.
- [ ] Add invalid/oversized/duplicate and copied-secret cases; run focused and full format tests.

## Task 2: Owned persistence and CLI

**Files:** server formats routes/tests, CLI variable-file subcommands/tests.

- [ ] Add an owned import-preview route and owned environment/globals export route. Preview never persists; workspace CAS remains the only write path.
- [ ] Add CLI import-into-workspace and export-scope commands through the same converter and expected revision. Test owner isolation, CAS conflict and no partial mutations.

## Task 3: Bilingual product flow and release evidence

**Files:** `web/src/features/environments`, shared types/i18n and focused tests.

- [ ] Add explicit environment-create and globals-replace import flows with scope conflict/name handling and async owner/workspace fences. Add private/full export controls.
- [ ] Verify real saved/reloaded 390px Chinese/English journeys. Run frontend/Rust suites, strict Clippy, production build, independent review and branch CI. Update coverage/status and platform evidence without claiming team-variable support.

## Progress, 2026-10-17

The shared converter, owned preview/export routes, CLI subcommands and explicit Environments page flow are implemented. Three pure converter, 57 existing interchange, four hosted format, two CLI process and 341 frontend cases passed along with strict all-target Clippy, rustfmt and production frontend/embedded server builds. A saved/reloaded 390px Chinese/English browser journey imported environment and globals files and downloaded default/full exports without horizontal page overflow. Branch CI, independent review and current platform gates remain.
