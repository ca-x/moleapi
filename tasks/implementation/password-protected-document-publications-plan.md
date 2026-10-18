# Password-protected publication plan

Authority: `docs/specs/password-protected-document-publications.md`. Base: `feat/published-doc-search` (`d3f5fe4`), stacked behind PR #33.

1. Extend create/input and durable publication data with a backwards-compatible access mode, Argon2id hash and bounded failed-attempt window. Keep the hash out of views and fast idempotency digests. Test public/password validation and idempotent reuse.
2. Add a publication-row-locked unlock operation and distinguish password-protected GET from missing/revoked GET. Test correct/wrong passwords, 10/429/reset behavior across two app instances and configured SQLite/PostgreSQL/MySQL.
3. Add owner visibility/password controls and a locked anonymous public page. Keep password and unlocked snapshot in component memory only. Test error states, locale, search after unlock, safe Markdown and 390 px keyboard flow.
4. Run affected/full suites, embedded browser and independent review; fix findings before pushing a stacked draft PR. Fixed-head Application CI, three-database, Docker, native package and installed gates follow.
