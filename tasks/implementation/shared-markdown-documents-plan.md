# Shared Markdown documents slice

**Parent:** `tasks/implementation/workspace-collaboration-plan.md`. **Authorities:** `docs/specs/workspace-collaboration.md`, `docs/specs/markdown-document-resources.md`, and `documentation-001`/`collaboration-003` in the feature matrix.

## Contract

- A shared-workspace viewer can select and safely preview the role-projected Markdown documents. Private document source must not enter the viewer response or DOM. A rendered internal request link may open only a request in the same projected shared workspace.
- Editors/admins can create a private-by-default document, rename it, move it between existing collections and the root, edit exact Markdown source, change its backup-private flag, preview it, and remove it after confirmation. All changes stay in the shared draft until the existing owner/workspace revision CAS save. Publication creation, unlock, revocation and appearance stay owner-only.
- Reuse the owned document validator for title, UTF-8 per-document/aggregate limits, and collection references. Invalid documents block every shared Save entry point, including the request/Mock tabs, and identify the offending document. Selection and pending async saves remain account/workspace/revision fenced; a remote update shows the existing conflict choice.
- Reuse `MarkdownPreview` and `Editor` to retain the established HTML/image/link safety and source fidelity. The 390 px English/Chinese layout must avoid horizontal document overflow.

## Acceptance

- Hosted integration: viewer projection screens a private source; editor CAS creates/edits a document and owner GET sees it; viewer cannot PUT or see private source after reload. Existing document validation rejects invalid title/source without revision change.
- Frontend: viewer has read-only preview without publish/edit/delete; editor create/edit/save/preview/delete confirmation; invalid document blocks both document and request tab Save; same-owner and cross-owner IDs remain scoped; conflict and account switch clear private drafts.
- Real 390 px bilingual browser save/reload/preview and private viewer flow, full frontend/backend tests, strict Clippy/formatting, production embedded build, independent review and fixed-head CI before claiming collaboration or documentation module completion.
