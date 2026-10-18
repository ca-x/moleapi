# Published Document Appearance Implementation Plan

**Goal:** Make each published Markdown version carry an immutable theme, navigation layout and sanitized PNG logo.

**Architecture:** Extend the existing publication create payload and immutable snapshot with a backward-compatible appearance value. Validate and normalize the logo on the server before computing idempotency and storage. Render only the snapshot-provided appearance on the anonymous page.

**Tech Stack:** Axum, SeaORM, Rust `png` decoder/encoder, React, Radix Themes, existing published-document routes.

**Spec:** `docs/specs/published-document-appearance.md`

## Global constraints

- Old snapshots and default idempotency keys keep their prior behavior.
- Password-locked and revoked responses reveal no logo or publication appearance.
- Only locally uploaded, decoded and re-encoded PNG bytes can become a public logo.
- Public page search, safe Markdown and the default sidebar layout remain functional.

## Tasks

### 1. Immutable appearance and logo normalization

- [x] Add real route tests for legacy/default snapshot, theme/layout, sanitized logo, rejected PNG input and key reuse with altered appearance.
- [x] Add `Appearance` to create/snapshot and owner summary in `crates/server/src/doc_publications.rs`; use the existing locked transaction.
- [x] Decode and re-encode a capped 1–512 px PNG through the pinned `png` crate; strip metadata and enforce 64 KiB before storing.
- [x] Include effective nondefault appearance in the idempotency digest while preserving the old default digest.
- [x] Run published-doc/database Rust tests, strict Clippy and scoped rustfmt. The local database URL set contained SQLite only.

### 2. Owner and anonymous UI

- [x] Add focused frontend tests for selection/reset, lock/no-leak, theme switch, top layout, logo and old snapshot defaults.
- [x] Add theme/layout controls and a bounded PNG picker with preview and clear action to `DocumentPublications.tsx`.
- [x] Apply immutable appearance in `PublicDocsPage.tsx`; provide top navigation and accessible logo with no network retrieval.
- [x] Add bilingual strings and scoped responsive styles; run full frontend tests/build and real 390px English/Chinese owner/anonymous browser flows.

### 3. Acceptance

- [ ] Record exact coverage and remaining matrix scope, review the diff, run fixed-head Application CI/three databases/Docker, then merge after required checks.
