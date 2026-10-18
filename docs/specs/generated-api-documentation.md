# Generated API documentation in the workspace

Authority: `documentation-002` in `docs/FEATURE-MATRIX.md`, the saved workspace request contract and the Markdown resource profile in `markdown-document-resources.md`.

## Objective and boundary

An owner can browse a live, read-only API reference assembled from saved HTTP requests. The reference follows the current workspace draft, so request edits and collection changes appear immediately without creating a second stored copy. It is available in the existing Documents page alongside authored Markdown. This slice provides an owner-scoped preview; it does not publish a site, create a share link or grant access to another account. Those remain required documentation capabilities.

## Content contract

- Include saved requests whose protocol is absent or explicitly `http`, grouped by their current collection. Preserve collection and request order. Other protocol requests remain visible through their own editors and are excluded from this HTTP reference; the page reports how many were excluded.
- For a selected request show its name, method, exact saved URL, description, enabled query and header rows, request body kind and exact saved text for JSON/text/form, auth kind, and saved response examples with status, enabled headers and exact body. Empty groups show a clear empty state. Binary and multipart source must not be interpreted as text or fetched.
- Render descriptions with the existing safe Markdown preview. Render URL, body and examples as text, never as HTML. Do not execute request scripts, resolve variables, make network requests, fetch remote images or expose a Run button from this page.
- The preview is owner-only under the existing workspace session. It may contain original credentials or example data and clearly labels itself private. It offers no implicit export, clipboard write or publication. Publication must later use a separate explicit privacy decision and server-side snapshot.
- The existing workspace Save action remains available while viewing the reference. It uses the same workspace validation as the Markdown editor and shows validation errors beside the page switch.
- Switching workspaces or accounts resets the selected request. If the selected request is deleted or changes to another protocol, select the next available HTTP request. Empty workspaces and zero-HTTP workspaces must be usable.

## Acceptance and limits

- The page updates when the owner edits a request, adds or removes one, or changes the collection. It never mutates the draft itself. Saved/reloaded workspaces show the same reference content.
- Desktop and hosted browser flows at 390 px show the reference without page-wide horizontal overflow. Keyboard navigation and visible focus work in the request list; headings and tables are semantic and bilingual.
- A malicious description, URL or example body cannot execute HTML/JavaScript or fetch remote media. Hidden or disabled parameter rows do not appear in the reference. Source text is not normalized, sent to a new endpoint or stored in new fields.
- This profile does not complete `documentation-002` publication parity. OpenAPI schema generation, protocol-specific docs, internal cross-resource links, search indexing, versioned publication, sharing, access control and installed package gates remain open.

## Project structure and verification

- `web/src/features/documents/ApiReference.tsx` owns the derived view; `DocumentsPage.tsx` owns the Documents page switch. Existing `WorkspaceData`, `RequestSpec`, `MarkdownPreview`, Radix and bilingual catalogs are reused. No new runtime dependency or server data model is needed.
- UI tests live beside the component. Run `npm --prefix web test -- --run`, `npm --prefix web run build`, `git diff --check`, then build embedded binaries with `cargo build -p moleapi-server -p moleapi-cli`. Browser acceptance uses the actual embedded hosted UI with a saved request, one non-HTTP request and malicious text. CI, three-database and package checks run on the fixed PR head.

The UI follows existing Radix tokens and document layout. For example, a saved request is displayed without changing it: `const source = collection.requests.find(request => request.id === selectedId);`.
