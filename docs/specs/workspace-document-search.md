# Workspace documentation search

Authority: `documentation-009` in `docs/FEATURE-MATRIX.md`, the private Markdown document and generated API reference profiles, and the existing owner-scoped workspace draft. This slice searches current workspace documentation while publication/search indexing remain separate.

## Objective and source contract

- The Documents page provides one bilingual search field for authored Markdown and saved API requests. Search uses the current unsaved workspace draft, requires no server call, does not persist an index/query/history and does not mutate source. It resets when the account or workspace changes.
- Search all authored document titles and exact Markdown sources, including private documents because this is an owner-only view. Search all request names, methods, URLs, descriptions, eligible text bodies, saved response example names/bodies, and enabled query/header names. Do not search credential values or disabled request rows. Collection/folder names appear as result context but are not independent matches.
- Treat the query as literal Unicode text, case-insensitively, split on whitespace into AND terms. No regex interpretation, HTML execution, Markdown rendering, variable expansion or remote fetch occurs. The field accepts at most 128 characters. A result contains its type, title, collection/folder context and a bounded plain-text excerpt (at most 160 characters) from matched source when useful.
- Rank title matches before source-only matches, preserving collection/document order within equal ranks. Show at most 100 results plus the total match count and an explicit refinement hint when truncated. Empty query shows no results; a nonempty query with no matches shows a clear empty state.
- Selecting a Markdown result opens that exact document in the Markdown preview within the current workspace. Selecting a request result opens the existing request editor after the same current account/workspace/request recheck as internal document links. If the target disappears before click, do nothing. Keyboard focus, result labels and live result counts are accessible.

## Acceptance and boundaries

- Tests cover case/Unicode/literal metacharacters, multiple terms, ranking/order, exact source retention, private source visibility to its owner, disabled/credential omission, 100-result budget, stale target/account switch and search reset.
- A 390 px embedded hosted browser creates/saves a Markdown document, searches private text and a request description, opens each result, changes language, verifies no horizontal overflow or unexpected network requests, and checks focus/axe. Rebuild frontend before embedded server/CLI. Fixed-head three-database, Docker, native package and installed smoke gates remain separate.
- This does not expose private content in public shares, build a published-site index or add cross-workspace search/permissions. Those remain required for complete `documentation-009` and the wider matrix.

## Structure

`web/src/features/documents/search.ts` derives bounded results; its tests own matching/ranking/budgets. `DocumentsPage.tsx` owns the field, result list and navigation using current workbench state. Existing request navigation, i18n, Radix and CSS patterns are reused. No new dependency or persisted schema is introduced.
