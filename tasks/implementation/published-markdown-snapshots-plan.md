# Published Markdown snapshots implementation plan

Authority: `docs/specs/published-markdown-snapshots.md`. Base: `feat/workspace-document-search` (`c9d8e53`). Keep this branch stacked until #24 and #27–#30 land.

1. Add the publication row model, bounded input validation and public snapshot projection. Verify private/missing selection, source/revision integrity and JSON serialization with focused tests.
2. Add transactional owner create/list/revoke routes and public read route. Reuse workspace row locking and the `documents` table; prove idempotency and concurrent limits on all configured databases. Add workspace-deletion cleanup in the same transaction.
3. Add a standalone anonymous public page and hosted Documents publication controls. Reuse safe Markdown rendering without private request navigation or account state. Cover 390 px, bilingual controls, keyboard operation and malicious content.
4. Run focused and full affected suites, embedded build, browser journey, independent review, then fixed-head Application CI, three-database, Docker and native package gates. Merge only after the required evidence; keep installed package acceptance in the release gate.

Each step must leave the earlier owner/private workspace behavior intact and must fail closed on storage or validation errors.
