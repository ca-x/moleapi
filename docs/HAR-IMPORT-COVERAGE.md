# HAR 1.2 import coverage

Scope: `interchange-006` and `docs/specs/har-import.md`. The same `moleapi-formats` converter serves the hosted `/api/import` route, the standalone CLI `import --format har` command and the bilingual import dialog. Import creates a new workspace through the existing owner-scoped workspace route; it does not automatically execute any captured request.

| Input | Current behavior |
| --- | --- |
| HAR 1.2 JSON, 1–1,000 entries, up to 20 MiB | Imports atomically as one ordered HTTP collection; malformed version, URL, method or unsupported entry fails the entire import. |
| HTTP(S) URL and `queryString` | The exact recorded URL is retained, including repeated query keys. A nonempty contradictory `queryString` rejects rather than changing the wire URL. |
| Headers, authentication and cookies | Preserves ordinary and Authorization headers. Transport-owned Host, Content-Length, Transfer-Encoding, Connection, Accept-Encoding, Keep-Alive, Proxy-Authorization, Proxy-Connection, TE and Trailer are omitted with warnings. WebSocket Upgrade requests reject. Structured cookies become a fixed Cookie header when no explicit Cookie header exists; the live Cookie Jar is disabled. An explicit Cookie header wins and a mismatch warns. |
| Request body | Preserves text, JSON and URL-encoded forms, including repeated form fields. Missing body bytes, file references, multipart, binary and unsupported MIME types reject. |
| Response example | Text/UTF-8 examples with valid HTTP status are saved. Content-Encoding, Content-Length, Transfer-Encoding and Connection response headers are omitted with warnings because the stored example body is decoded text. Binary, invalid encoding and oversized bodies are omitted with warnings; no live history, timings, redirects or TLS state are inferred. |
| Secrets and owners | Default native export screens Authorization, Cookie and captured Set-Cookie values; explicit full export retains them. Hosted workspace creation/export enforces owner isolation. |

Evidence at the feature commit: the shared HAR fixture passed three focused converter tests plus the 57-case interchange suite. Hosted tests imported/saved an owned workspace, rejected unsupported input, checked private/default export and sent imported GET/query/headers and POST/form bytes to a real loopback HTTP fixture. A real CLI process imported the same HAR file, listed three requests and exported a screened workspace. The 390px Chinese/English browser imported HAR, saved/reloaded the workspace, displayed `GET /items` and its `HAR 200` example, and showed no document overflow. The 333-case frontend suite, production build and strict all-target Clippy passed. Application CI run38940050230 passed source, check and three-database jobs; PR #5 merged at `a2be8f9`. Current platform packages remain a separate gate.

Not covered: HAR export, WebSocket entries, file/multipart/binary request replay, full browser cache/session state, and automatic response history reconstruction. `interchange-006` should not be marked complete for these unsupported combinations without further mapping and evidence.
