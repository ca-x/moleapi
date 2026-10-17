# Insomnia V4 import implementation plan

> Execute in the isolated `feat/insomnia-import` worktree. Do not mark the full `interchange-014` row complete for this finite V4 profile.

**Spec:** `docs/specs/insomnia-v4-import.md`.

## Task 1: Shared converter and privacy

- [x] Add a representative V4 fixture with workspace, folders, requests and base/child environments, then watch the shared importer test fail before implementation. An actual Insomnia-generated export remains a later interoperability check.
- [x] Implement bounded resource preflight (version, IDs, counts, parents/cycles) and type/body/auth/query mapping through existing `moleapi-formats` helpers. Reject unsupported semantics atomically.
- [x] Store the exact original under `Specification.kind=insomnia`. Clear it in default backup, retain it in full backup, and reject unverified external export. Add privacy and failure regressions.
- [x] Run focused and full formats suites, strict formats Clippy and rustfmt; commit.

## Task 2: Hosted, CLI and editor entry points

- [x] Route `format=insomnia` through the existing hosted owner and standalone CLI imports. Add real process/router fixtures with save/reload, owner denial, default/full backup and checked loopback requests.
- [x] Add an Insomnia V4 JSON choice to the bilingual import dialog, reusing its bounded input path; run focused frontend tests.
- [x] Run 390px English/Chinese paste/import/save/reload/wire browser flow; commit. The native file picker is a separate UI seam.

## Task 3: Final evidence and integration

- [x] Run relevant Rust/frontend suites, strict Clippy, formatting, frontend then embedded server/CLI builds. Record supported and rejected V4 cases in a coverage document and implementation status.
- [ ] Request independent read-only review, fix Critical/Important issues and re-review.
- [ ] Push a draft PR based on main; run exact-head three-database/Docker/native CI, inspect package smoke and merge a verified batch. Keep V5, unsupported Insomnia resources and the full matrix open.
