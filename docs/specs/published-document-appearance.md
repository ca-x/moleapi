# Published documentation appearance

Status: implementation specification. Matrix target: `documentation-008` (published site theme, layout and logo). This extends the immutable Markdown publication contract in `docs/specs/published-markdown-snapshots.md` and the password boundary in `docs/specs/password-protected-document-publications.md`.

## Objective and assumptions

A hosted owner chooses how a new document publication looks before publishing. Readers see the chosen theme, navigation layout and logo as part of that immutable version. Editing the workspace or changing later publishing defaults cannot change an existing publication. The current public page has a light theme and sidebar layout; old saved publication records keep that exact appearance after upgrade. This slice does not upload a logo to a remote media host or add custom CSS/HTML/JS.

## Contract

1. Publication create accepts optional `appearance` with `theme` (`light`, `dark`, `system`), `layout` (`sidebar`, `top`) and optional `logo_png_base64`. Omitted appearance means light/sidebar/no logo. Unknown values, unknown fields and unsupported media reject atomically before a publication row is inserted. Owner view reports theme/layout/has-logo without returning logo bytes. The public snapshot includes the appearance and logo bytes only after public access or a successful password unlock.
2. Logo is an explicitly selected local PNG. The server base64-decodes at most 64 KiB of original bytes, verifies a single 1–512 px wide/high PNG with a mature PNG decoder and bounded decode allocation, then re-encodes pixel data to a stripped PNG without source metadata or animation. Reject if the sanitized PNG exceeds 64 KiB. Only this sanitized Base64 is stored in the immutable snapshot. No URL supplied by the workspace or Markdown source is fetched for the logo. The browser may precheck file type/size for feedback, but server validation is authoritative.
3. The publication idempotency digest covers the effective theme, layout and sanitized logo content. Reuse of one idempotency key with changed appearance conflicts without creating a second version. The default appearance retains the old digest format so a valid pre-upgrade replay still resolves to its existing version. Password equality and visibility checks retain the existing Argon2id behavior; raw passwords never enter the digest or owner list.
4. The anonymous document page applies the stored appearance after the public snapshot is received. `light` and `dark` are explicit; `system` follows the reader's current `prefers-color-scheme` setting and responds to changes. Sidebar remains the default; top layout puts navigation above the article and remains usable at 390px without horizontal page overflow. Search/filter, keyboard navigation, article selection, safe Markdown rendering and language switching work in both layouts. A logo is rendered from a `data:image/png;base64` URL under the existing restrictive CSP. The selected logo has a text alternative based on publication title.
5. A locked password page shows no publication title, document content, logo or stored appearance until a successful unlock. Revoked, deleted and invalid publications continue to return generic 404 with privacy headers. A failed unlock does not return the appearance. Reload discards the decrypted snapshot and locks again.
6. The owner publication form exposes labeled theme/layout controls and an optional PNG picker with a local preview, size guidance and clear action. Changing any appearance field regenerates the idempotency key. Pending publish actions fence account/workspace changes, keep the chosen appearance visible on validation errors, and clear the selected logo after success or logout. The owner sees a concise appearance summary on each saved version.

## Source, style and commands

- Backend: `crates/server/src/doc_publications.rs`, existing owner/public route tests in `crates/server/tests/published_docs.rs`, three-database cases in `crates/server/tests/database.rs`, dependency metadata in `crates/server/Cargo.toml` and `Cargo.lock`.
- Frontend: `web/src/features/documents/DocumentPublications.tsx`, `PublicDocsPage.tsx`, their tests, bilingual catalogs and scoped `web/src/styles.css` rules. Reuse Radix controls and existing workbench appearance tokens; render user text as text, never HTML.
- Verify with `cargo test -p moleapi-server --test published_docs --test database`, strict server all-target Clippy and rustfmt, `npm --prefix web test`, `npm --prefix web run build`, the configured SQLite/PostgreSQL/MySQL CI and an actual 390px Chinese/English owner/anonymous browser flow. The embedded server build must use the final frontend assets.

## Acceptance and boundaries

- Test default/legacy snapshot compatibility, each theme/layout, rejected logo formats and dimensions, metadata removal, sanitized output size, idempotency retry/change, owner isolation and password lock/no-leak behavior. Verify an old version remains visually unchanged after new publications and workspace edits.
- Browser checks cover top/sidebar at desktop and 390px, dark/system changes, search, keyboard navigation, valid logo and clear action, locked/unlocked/reload/revoke flows, no external image request and WCAG A/AA audit.
- Custom domain, SEO/analytics, custom CSS/JS/HTML, generated API reference publication, signing and final native installer validation remain separate open matrix work.
