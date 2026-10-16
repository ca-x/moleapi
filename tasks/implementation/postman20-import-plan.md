# Postman Collection 2.0 import implementation plan

> **For agentic workers:** Extend the existing Postman importer; preserve the 2.1 path and add independent 2.0 evidence before changing status claims.

**Goal:** Import valid Postman Collection 2.0 JSON through all existing Postman entrypoints without dropping object-form authentication.

**Architecture:** Bundle the pinned official 2.0 schema, select schema by `info.schema`, then reuse `postman::walk` and the existing workspace validator. Teach `auth_value` to read either validated object or attribute-array helpers. Keep the original source and exact dialect.

**Tech Stack:** Rust/Serde/jsonschema, existing shared formats/Axum/CLI and React import dialog.

**Spec:** `docs/specs/postman20-import.md`

## Task 1: Versioned schema and auth mapping

**Files:** `crates/formats/schemas/postman20.json`, `crates/formats/schemas/README.md`, `crates/formats/src/postman.rs`, `crates/formats/tests/interchange.rs`.

- [ ] Add the flattened official 2.0 schema at the pinned commit and a valid fixture with object-form basic/bearer/API key settings. Verify current importer rejects it before the change.
- [ ] Select 2.0 or 2.1 schema by source version. Reuse the shared mapper, including object-form helper value access, and set the retained source dialect. Keep schema validation before mapping.
- [ ] Add malformed auth/version and privacy regressions; run the full interchange suite and strict Clippy.

## Task 2: Hosted, CLI and browser evidence

- [ ] Import the same 2.0 fixture through hosted owner-scoped workspace creation and a standalone CLI process. Verify source restoration and default credential screening.
- [ ] Import in the existing 390px Chinese/English menu, save/reload and inspect authentication/body/example fields. Run production frontend/embedded build and relevant tests.

## Task 3: Integration

- [ ] Update interchange coverage/status and remaining-work plan with supported 2.0 combinations and explicit omissions. Push a branch, trigger Application CI and inspect platform gates separately.

## Progress, 2026-10-17

The official 2.0 schema, object-auth mapping and legacy `data` body path are implemented. The integrated 61-case interchange suite, hosted owner/privacy and actual Bearer/API Key calls, CLI process import/export, 333 frontend tests, strict Clippy and production frontend/embedded server builds passed locally. A saved/reloaded 390px Chinese/English browser import showed auth, body and example fields without document overflow. Branch CI and current platform gates remain.
