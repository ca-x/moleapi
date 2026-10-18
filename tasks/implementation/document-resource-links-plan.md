# Workspace Document Request Links Implementation Plan

> **For agentic workers:** Execute one testable task at a time in the isolated feature worktree.

**Goal:** Connect private workspace Markdown and generated descriptions to owned saved requests without external navigation.

**Architecture:** Parse one canonical `moleapi://request/` URI form in a pure helper. The preview renders an active button only for IDs in the current draft; the page rechecks account/workspace and request existence at click time before selecting the existing request editor. The generated API reference copies the URI as an authoring aid.

**Tech Stack:** React, TypeScript, React Markdown, Radix UI, Vitest and existing workbench state.

**Spec:** `docs/specs/document-resource-links.md`

## Global constraints

- Preserve Markdown source bytes and all current external-link/image safety rules.
- Do not introduce a public route, request execution, workspace mutation or runtime dependency.
- Internal links resolve only against the current owned workspace draft.
- Keep bilingual controls and 390 px no-overflow behavior.

## Review focus

- Percent encoding and malformed/ambiguous URI variants cannot select an unintended ID.
- A deleted request, account switch or workspace switch invalidates old preview links before click.
- Clipboard failures show feedback and do not mutate the draft.
- Buttons remain keyboard-accessible and do not open new browser tabs.
- External safe links and blocked script/data/file/relative links retain their previous behavior.

### Task 1: URI contract and preview

**Files:** `web/src/features/documents/internalLinks.ts`, `internalLinks.test.ts`, `MarkdownPreview.tsx`, `MarkdownPreview.test.tsx`.

- [x] Add failing parser and preview tests for canonical IDs, malformed URLs, current-workspace membership and external-link safety.
- [x] Implement pure format/parse helpers and conditional internal buttons/inert labels.
- [x] Run focused tests and TypeScript checks.

### Task 2: Workbench navigation and authoring aid

**Files:** `DocumentsPage.tsx`, `ApiReference.tsx`, their tests, `web/src/styles.css`, bilingual locale catalogs.

- [x] Add failing page tests for valid navigation and stale/account-switched link refusal, plus clipboard success/failure.
- [x] Wire current-draft request IDs and guarded workbench navigation; add Copy request link to the generated reference and visible feedback.
- [x] Run focused tests, full frontend suite and production chunk verifier.

### Task 3: Real app and delivery

**Files:** `docs/DOCUMENT-RESOURCE-LINK-COVERAGE.md`, `docs/IMPLEMENTATION-STATUS.md`, this plan.

- [x] Build embedded server/CLI after the frontend build, then check a 390 px bilingual hosted browser flow including malicious and stale links, console/network and focus.
- [ ] Review the diff, record exact evidence and gaps, commit/push a stacked draft PR, run independent read-only review and fixed-head CI/native gates.
