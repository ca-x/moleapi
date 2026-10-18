# Search published Markdown documents

Authority: `documentation-009` in `docs/FEATURE-MATRIX.md`, the immutable publication contract in `published-markdown-snapshots.md`, and the private search behavior in `workspace-document-search.md`. This adds search to an already published hosted site; it does not publish new source.

## Objective and contract

- An anonymous reader can search the documents inside the one public snapshot currently open. Search runs entirely in the browser over the previously fetched snapshot. It never fetches workspace data, other publications or a remote index, and never persists the query, results or source.
- Treat the query as literal Unicode text, case-insensitively, split on whitespace into AND terms. The field accepts at most 128 characters. Match a document when all terms appear in its title, folder label or Markdown source. Do not interpret regex, HTML, variables or internal request links.
- Show the count and matching document navigation in original publication order. A matching document can be opened by keyboard or pointer; selection never changes the publication URL or calls a server. Empty query restores the full document list. A nonempty query with no matches shows a clear empty state while retaining the current article.
- The snapshot already limits publications to 32 Markdown documents and 1 MiB total source, so search must not introduce an unbounded index or repeated remote work. Case folding and excerpt computation are bounded to the current snapshot. No private workspace search source is available to this page.
- Search text, labels, result count and empty state use the existing English/Chinese locale. At 390 px, search and navigation fit without page-wide horizontal overflow. Keyboard focus and result status are accessible.

## Verification

- Pure tests cover Unicode case folding, literal regex characters, AND terms, title/folder/source matching, stable order, empty/no-match behavior and exact source preservation.
- Public-page tests verify searching changes only the visible local navigation and does not call `fetch` again or expose a document outside the snapshot. A real anonymous 390 px hosted browser checks both languages, keyboard selection, zero results and no horizontal overflow.
- Run full frontend tests, TypeScript and production chunk-cycle build, then embedded hosted browser smoke. Existing publication owner/revocation/three-database/native gates remain independently required.

## Structure and remaining scope

`web/src/features/documents/publishedSearch.ts` owns pure matching; `PublicDocsPage.tsx` owns the field and local selection. No server schema, route, dependency or persisted data changes. Published API-reference search, custom navigation, cross-publication search and full `documentation-009` parity remain open.
