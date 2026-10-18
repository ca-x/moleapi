# Password-protected document publications

Authority: `documentation-004` and `documentation-011` in `docs/FEATURE-MATRIX.md`, `published-markdown-snapshots.md`, and the existing hosted authentication/security contracts. This adds a password visibility option to an explicitly published Markdown snapshot; it does not publish private workspace documents or change anonymous public publications.

## Objective and disclosure model

- An owner may publish a saved selection as `public` or `password`. Existing publications deserialize as `public`. A password publication requires a 10–128 byte UTF-8 password supplied at creation and stored only as a salted Argon2id hash. Public mode rejects a supplied password instead of ignoring it. Owner list/history shows the mode and revocation state, never the password/hash or unsuccessful attempts.
- The selected snapshot is still immutable. The password controls access to the entire version, not individual documents. Changing the password means publishing a new version and revoking the old one; edits to a workspace or its Markdown never silently change a publication. Default/full backups and sync remain unable to recreate publication grants.
- `GET /docs/{id}` serves a content-free frontend shell for any active publication, whether public or password protected. Revoked, malformed and absent IDs return generic 404 at the HTML route. `GET /api/public/docs/{id}` returns a public snapshot only for public mode; a password version returns 403 without title, document metadata or body.
- `POST /api/public/docs/{id}/unlock` accepts a JSON password over HTTPS and, on success, returns only that snapshot. The browser holds the password only until the request settles and holds the unlocked snapshot in component memory. No password, cookie, token or unlocked source is written to local/session storage, the URL or an application log. Reload requires unlocking again. An unsuccessful password yields a generic 401 and clears the field. Missing/revoked IDs yield the same generic 404 as the read route.
- Failed unlocks are bounded across hosted instances using a transactionally locked publication row: at most 10 failed attempts in a rolling 5-minute window, then 429 with `Retry-After` until the window ends. A successful unlock resets the counter. No user IP or password attempt body is persisted. Rate limits may briefly deny legitimate readers during an attack; they never revoke the publication or leak the password. Storage failures fail closed.
- Reusing an idempotency key with identical metadata and password returns the same active version. A different mode/password returns conflict; a revoked key cannot republish. The server never stores a fast unsalted password digest to implement idempotency.

## User experience

- The bilingual owner publication panel offers Public and Password choices and requires a password for Password mode. It explains that the password cannot be recovered and that a public link still reveals only a generic locked page before unlock. Password text is not prefilled, persisted or displayed in version history.
- The anonymous page shows a bounded, keyboard-accessible password field and Unlock button for a protected version. It does not fetch private workspaces or call the normal login API. Wrong password, rate limit and missing publication have distinct helpful copy without exposing document data. The existing safe Markdown renderer and local published search work only after successful unlock.
- At 390 px the form, messages, title and navigation fit without page-wide horizontal overflow. English and Chinese labels update immediately. A 404 page and all public responses keep no-store, no-referrer and restrictive CSP headers.

## Verification and structure

- Server tests cover public/password create validation, owner/list hash omission, anonymous GET 403, correct/wrong unlock, no fast digest, idempotent same/different password, revocation and deletion, cross-instance 10/429/reset behavior and equivalent SQLite/PostgreSQL/MySQL behavior. Existing public site tests keep passing.
- Frontend tests cover creation controls, no password persistence, locked form, wrong/rate-limited/missing states, successful unlock/search, locale and safe rendering. A real 390 px hosted browser uses separate owner/anonymous sessions, verifies no source/title before unlock, unlocks, searches, reloads to lock again, then revokes to 404. axe and network inspection verify accessible controls and no private API calls.
- Keep the stored password hash and failed-attempt metadata in the existing `doc-publication` row, under the same database row lock used for version operations. Use the existing Argon2 library and shared public route; do not create a second copy of the workspace. Run focused/full affected tests, strict Clippy, production/embedded builds, three-database CI, Docker, native and installed-package gates.

Delivered content cannot be retracted from a reader who already received it before revocation. Custom domains, login-only audience lists, per-document passwords and complete `documentation-004`/`011` parity remain open.
