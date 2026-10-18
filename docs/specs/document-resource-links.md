# Workspace document request links

Authority: `documentation-001` and `documentation-002` in `docs/FEATURE-MATRIX.md`, `markdown-document-resources.md` and `generated-api-documentation.md`. This slice connects authored Markdown and generated API descriptions to requests in the same owned workspace.

## Objective and source contract

- A Markdown link with exact form `[label](moleapi://request/<encoded-request-id>)` opens the saved request editor in the current workspace. The request ID is encoded with `encodeURIComponent`; decoding occurs once and must reject malformed escapes, empty IDs, extra path segments, userinfo, ports, query and fragment. The source string is retained unchanged in the Markdown document or request description.
- Only a request present in the current workspace draft can become an active internal link. Missing, stale, malformed or unsupported `moleapi:` targets render their label as inert text. No link may select a request from another workspace or account. The click path checks the current account/workspace and request existence again before changing UI state.
- Internal navigation is local. It never opens a browser URL, invokes a request, saves a draft, fetches a resource or includes credentials in a URL. Ordinary HTTP(S)/mailto links retain the current explicit new-tab/noopener behavior; relative/script/data/file links and remote images retain their inert behavior.
- The generated API reference shows a Copy request link action for its selected request. It copies only the canonical internal URI, reports success/failure, and does nothing when the clipboard API is unavailable. A user can paste the URI into authored Markdown; copying never modifies the draft.

## User experience and verification

- The Markdown preview and generated request descriptions share the same resolver. A visible internal link is a keyboard-operable button styled as a link; a missing target remains legible text. The editor keeps the exact source, and account/workspace switching cannot leave a clickable stale target.
- A 390 px hosted browser creates and saves a Markdown document with a request link, activates it, then deletes or switches the linked request and confirms that the label becomes inert. The UI shows English and Chinese copy without horizontal overflow. A malicious `moleapi:` URL cannot navigate externally or execute code.
- Focused tests cover canonical encoding/decoding, invalid URL forms, absent/foreign targets, retained external-link behavior, clipboard success/failure and navigation. Run the complete frontend suite, TypeScript/production build and static chunk verifier before rebuilding embedded server/CLI; fixed-head CI/native packages and installed smoke remain separate gates.

## Structure and boundaries

`web/src/features/documents/internalLinks.ts` owns URI parsing/formatting. `MarkdownPreview.tsx` renders safe active/inert links, `DocumentsPage.tsx` routes a validated click through the existing workbench request/view state, and `ApiReference.tsx` exposes Copy request link. Tests live beside these files. Existing workspace models, Markdown source, server storage and export rules remain unchanged.

This does not create public documentation URLs, cross-workspace permissions, attachment links or automatic link rewriting after request deletion. Those remain separate capabilities.
