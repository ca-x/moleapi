# Published document search plan

Authority: `docs/specs/published-document-search.md`. Base: `feat/document-publish` (`70b63b3`), stacked behind PR #32.

1. Add a bounded pure matcher for snapshot titles, folder labels and Markdown source. Verify literal Unicode/AND semantics and stable order with focused tests.
2. Add a search field, count and filtered navigation to the anonymous public page. Keep the selected article on no results and make each matching result keyboard-operable.
3. Run focused/full frontend tests, typecheck, production build and a real anonymous 390 px browser flow in both languages. Independently review source/privacy boundaries and then run exact-head CI and native package gates.
