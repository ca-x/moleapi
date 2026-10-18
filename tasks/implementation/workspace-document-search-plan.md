# Workspace Documentation Search Implementation Plan

> **For agentic workers:** Implement one verified task at a time in the isolated feature worktree.

**Goal:** Search private current-workspace Markdown and saved API request documentation with direct result navigation.

**Architecture:** Derive a bounded in-memory result list from the current immutable `WorkspaceData` on demand. DocumentsPage owns query state and routes results through existing selected-document state or guarded request navigation. Nothing is indexed or exported.

**Tech Stack:** React, TypeScript, Radix UI, Vitest, existing i18n and embedded Rust server build.

**Spec:** `docs/specs/workspace-document-search.md`

## Global constraints

- Query length 128 characters; show at most 100 of all matches and the true count.
- Literal case-insensitive AND terms; title matches rank first, stable source order otherwise.
- Keep source unchanged, credential values excluded and all results owner-only.
- Use the current account/workspace/request fence before opening a request.

## Review focus

- Search metacharacters and Unicode are literal, not regexes or HTML.
- Private Markdown source may appear only in the current owner's page.
- Disabled parameter rows and auth values do not become searchable.
- Large result sets do not render unbounded DOM or oversized snippets.
- Deleted targets and account/workspace switches cannot route a stale result.

### Task 1: Pure search contract

**Files:** `web/src/features/documents/search.ts`, `search.test.ts`.

- [x] Write failing tests for terms, ranking/order, exact source excerpts, credential omission and 100-result cap.
- [x] Implement pure bounded search from `WorkspaceData` and make focused tests pass.

### Task 2: Page interaction

**Files:** `DocumentsPage.tsx`, `DocumentsPage.test.tsx`, `web/src/styles.css`, bilingual catalogs.

- [x] Add failing tests for search count/empty state, opening a document and guarded request, and account/workspace reset.
- [x] Add an accessible search field and result list; preserve existing Markdown/API view and save behavior.
- [x] Run focused tests, full frontend suite, TypeScript and production chunk verifier.

### Task 3: Real app and delivery

**Files:** `docs/WORKSPACE-DOCUMENT-SEARCH-COVERAGE.md`, `docs/IMPLEMENTATION-STATUS.md`, this plan.

- [x] Build embedded server/CLI and exercise a 390 px bilingual hosted search/navigation flow with private text, request text, zero network side effects, focus and axe.
- [ ] Review diff, record exact evidence and gaps, commit/push a stacked draft PR, obtain independent read-only review and start fixed-head CI/native gates.
