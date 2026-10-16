# HAR 1.2 import implementation plan

> **For agentic workers:** Use one shared converter for CLI, hosted API and native UI. Check each behavior with a real HAR document before expanding the supported subset.

**Goal:** Import standard HAR 1.2 HTTP(S) request captures into owned MoleAPI workspaces without silently changing request semantics.

**Architecture:** Parse bounded HAR JSON inside `moleapi-formats`, map entries into existing `RequestSpec`/`Example` and validate the resulting workspace. Existing `/api/import`, CLI import and workspace creation paths consume the same result. Add a HAR choice to the import dialog only; no schema or database migration.

**Tech Stack:** Rust/Serde/URL, existing Axum/CLI formats API, React/Radix/Vitest.

**Spec:** `docs/specs/har-import.md`

## Task 1: Shared bounded converter

**Files:** `crates/formats/src/har.rs`, `crates/formats/src/lib.rs`, `crates/formats/tests/interchange.rs`.

- [ ] Add failing real HAR fixtures covering ordered entries, duplicate URL query, headers/cookies, JSON/form/text body, response example and default/full privacy.
- [ ] Implement `har::import(content)` with 20 MiB shared input, 1–1,000 entries, exact URL authority, atomic rejection and specific omission warnings. Reuse `request`, `pair`, `data` and final workspace validation.
- [ ] Add malformed/unsupported/oversized cases and run the format suite and strict Clippy.

## Task 2: Product entrypoints

**Files:** `web/src/features/interchange/InterchangeFields.tsx`, dictionaries, hosted/CLI import tests.

- [ ] Add HAR only to the import format menu and a focused bilingual UI regression.
- [ ] Verify hosted owner-scoped workspace import and CLI file import against the same HAR fixture, including privacy and no workspace created on unsupported input.

## Task 3: Runtime and release evidence

- [ ] Import HAR through the 390px Chinese/English UI, save/reload and inspect converted request and response example.
- [ ] Run relevant Rust/frontend tests, strict Clippy, formatting, production build and branch Application CI. Update `docs/IMPLEMENTATION-STATUS.md`, interchange coverage and remaining work plan with exact results and unsupported cases.

## Progress, 2026-10-17

The shared HAR converter and import menu are implemented. The 57-case interchange suite, hosted owner/privacy and actual GET/POST replay fixtures, CLI process import/export, 333-case frontend suite, production build and strict all-target Clippy passed locally. A saved/reloaded 390px Chinese/English browser import showed the first request and `HAR 200` example without document overflow. Integration with merged ODS code and branch CI remain separate gates.
