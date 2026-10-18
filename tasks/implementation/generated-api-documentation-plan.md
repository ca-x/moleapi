# Generated API Documentation Implementation Plan

> **For agentic workers:** Execute this plan task by task in the isolated feature worktree; verify each checked step before advancing.

**Goal:** Add a private, live API reference from saved HTTP requests to the Documents page.

**Architecture:** Derive reference entries directly from `WorkspaceData` in React, with no new persisted document, route or external fetch. Reuse safe Markdown rendering for descriptions and plain text for all other source fields. The existing account and workspace state controls ownership.

**Tech Stack:** React, TypeScript, Radix UI, Vitest, existing i18n and embedded Rust server/CLI build.

**Spec:** `docs/specs/generated-api-documentation.md`

## Global constraints

- Include absent/`http` protocol only; report excluded non-HTTP count.
- Keep exact source text; never execute, fetch, export or save a derived copy.
- Preserve collection/request order and follow current unsaved draft.
- Use existing locale catalogs and 390 px layout rules.

## Review focus

- An account/workspace switch must not retain a prior selected request.
- A deleted or newly non-HTTP selected request must fall back to a valid HTTP entry.
- Disabled query/header rows must not appear; private body/example text must remain on the owner page.
- Malicious description/URL/body/example text must stay inert and make no remote request.
- Long URLs and code blocks must scroll within their container at 390 px.

### Task 1: Derive and render the reference

**Files:** `web/src/features/documents/ApiReference.tsx`, `web/src/features/documents/ApiReference.test.tsx`.

- [x] Write focused tests for ordered HTTP filtering, disabled fields, exact body/examples, fallback selection, account/workspace reset and inert text.
- [x] Run the focused tests and confirm the missing component fails.
- [x] Implement the read-only reference, collection/request list and selected article with safe Markdown descriptions, text-only code and semantic headings/tables.
- [x] Re-run focused tests; inspect for source mutation or accidental fetches.

### Task 2: Connect the Documents page and bilingual layout

**Files:** `web/src/features/documents/DocumentsPage.tsx`, `web/src/styles.css`, `web/src/shared/i18n/en.json`, `web/src/shared/i18n/zh-CN.json`, `web/src/features/documents/DocumentsPage.test.tsx`.

- [x] Add a failing page test for the Markdown/API switch and live draft updates.
- [x] Add a clear, accessible switch while preserving the existing Markdown editor state. Style the reference for desktop and 390 px without page overflow.
- [x] Re-run focused tests and production TypeScript/build checks.

### Task 3: Real application and delivery evidence

**Files:** `docs/GENERATED-API-DOCUMENT-COVERAGE.md`, `docs/IMPLEMENTATION-STATUS.md`, this plan.

- [x] Run the complete frontend suite, production chunk verifier and embedded server/CLI builds.
- [x] Exercise the embedded hosted UI at 390 px in English and Chinese with a saved request, non-HTTP request, changed draft and inert malicious content; inspect console/network and horizontal overflow.
- [ ] Document exact evidence and remaining matrix gaps, review the diff, commit, push a stacked draft PR and monitor fixed-head CI/native jobs.
