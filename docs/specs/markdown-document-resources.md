# Markdown document resources

Authority: `documentation-001` in `docs/FEATURE-MATRIX.md`, the existing owner-scoped workspace CAS/save/sync contract and MoleAPI default/full backup privacy rules. This establishes editable Markdown resources inside a workspace; site publication, public links, search indexing and internal cross-resource references remain separate documentation capabilities.

## Objective and data contract

- A workspace stores up to 256 Markdown documents. Each has a stable nonempty ID (at most 128 bytes), a title (1–128 visible characters after trim, no control characters), optional existing collection ID, exact UTF-8 Markdown source (at most 256 KiB) and `secret` flag. All sources together are capped at 2 MiB. New documents are private by default (`secret=true`); old workspaces deserialize with an empty list. Documents remain editable when their folder is renamed or moved; removing a folder moves its documents to the workspace root rather than silently deleting them.
- The existing workspace save, expected-revision CAS, owner boundary, native offline storage and sync carry the documents. The bilingual Documents page lists root/folder assignments, creates, renames, edits, moves and deletes documents with an explicit delete confirmation. It preserves unsaved drafts across document selection, exposes the workspace Save action, and restores exact content after reload. A 390px viewport can edit and preview without document horizontal overflow.
- Preview renders Markdown headings, paragraphs, lists, code, blockquotes and GFM tables/task lists with a mature parser. Raw HTML is ignored, not executed. Image syntax renders alt text without fetching image URLs. Links open only explicit HTTP(S) or mailto destinations in a new tab with `noopener noreferrer`; relative, script, data and file URLs are inert text. Source editing does not fetch remote content.
- Default MoleAPI backup retains document metadata but removes source for `secret=true`. A document explicitly marked nonsecret retains source with the shared known-secret matcher applied, withholds content when the privacy budget is exhausted, and screens its title. Explicit full MoleAPI backup preserves exact source and flags. Postman/OpenAPI/HAR export rejects workspaces with Markdown documents until a faithful mapping exists.

## Validation and boundaries

- Validate document limits, IDs, titles, collection references and aggregate bytes in the core workspace validator before any hosted save, native import or sync. Reject malformed data atomically. The editor enforces the same visible limits; server validation remains authoritative. Keep a saved source unchanged unless the user edits it. No Markdown document is executable code or a Mock/script input.
- Creating a document with an empty source is valid. Rendering large documents is limited to the one selected document. Plain Markdown link text remains visible for unsupported URL schemes. The first profile does not publish documents or allow embedded arbitrary HTML/CSS/JS, remote images, attachments, executable code blocks, internal `moleapi:` links or full text indexing.
- The `secret` flag describes default backup behavior, not hosted access control: workspace ownership still applies to every document. Marking a document nonsecret is an explicit user choice and does not publish it.

## Project structure, style and commands

- `crates/core/src/models.rs` and `validation.rs`: bounded document schema and save checks.
- `crates/formats/src/redact.rs` and `lib.rs`: default/full privacy and external export gate.
- `web/src/features/documents`: the bilingual editor, list and safe preview; `workbench/navigation.ts`, `WorkbenchMain.tsx` and shared types connect it to existing UI.
- `docs/MARKDOWN-DOCUMENT-COVERAGE.md`: exact evidence and remaining documentation rows.

Reuse existing Rust serde/rustfmt, SeaORM CAS and React/Radix field/editor patterns. Build frontend before embedded binaries. Verify with `cargo test -p moleapi-core -p moleapi-formats -p moleapi-server`, strict all-target Clippy, rustfmt, `npm --prefix web test -- --run`, TypeScript/production build, then `cargo build -p moleapi-server -p moleapi-cli`. Use a real hosted owner/foreign-owner route, native default/full backup and a 390px bilingual browser save/reload/preview to establish local acceptance. Fixed-head three-database, Docker, native package and installed smoke remain separate gates.
