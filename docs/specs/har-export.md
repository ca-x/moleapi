# HAR 1.2 request-template export

Authority: `interchange-006` in `docs/FEATURE-MATRIX.md`, `docs/specs/har-import.md`, and the shared owner-scoped workspace export API. This exports saved HTTP requests as a HAR-shaped template. It does not claim to be a browser traffic recording or a complete workspace backup.

## Contract

- `format=har` is available through the shared formats exporter, hosted owner-scoped export route, standalone CLI, and bilingual workspace export picker. It emits UTF-8 HAR 1.2 JSON, filename `.har`, with 1–1,000 ordered HTTP requests and a 20 MiB serialized output cap. Empty workspaces, unsupported protocols, non-HTTP(S)/templated URLs, unresolved variables, request scripts, dynamic authentication, network overrides, binary/multipart bodies, and Mock implementation scripts fail the whole export with a useful error. Use MoleAPI backup for those fields.
- Preserve each request's method, absolute URL, enabled literal query pairs in order, enabled safe headers in order, and `none`/`json`/`form`/`text` body bytes and MIME. Apply the existing default workspace redaction before mapping; explicit `include_secrets=true` maps the original values. Remove transport-owned headers with warnings and reject WebSocket upgrades. Never take local secret values from disabled rows. If the live Cookie Jar or a policy setting cannot be represented, include a warning; the HAR is a request template, not a runtime transcript.
- When a request has one saved textual response example, emit its status, safe enabled headers, UTF-8 body, MIME and byte size. More than one example, binary content, or a body over the importer's 1 MiB example cap fails rather than silently dropping an example. When no example exists, emit a status-0 placeholder that the importer discards.
- HAR-required timing, cache, request/response size and HTTP version fields use explicit unknown placeholders; `comment` records that the entry is generated from a saved MoleAPI request. No observed time, redirect, TLS, cookie-jar state or execution history is invented. The resulting JSON must be parseable by the existing HAR importer, with equivalent supported request wire fields and selected example. Exports with redaction remain safe under that roundtrip.

## Evidence

- Format tests: import an existing HAR fixture, export and reimport; verify ordered URL/query/header/body/example fields, default/full secret behavior, output size, unsupported-field rejection and no partial output.
- Hosted tests: export requires owner, returns parseable HAR, screens secrets by default and preserves them only for explicit full export. Standalone CLI export and bilingual UI picker must use the same server contract.
- Run focused Rust/frontend tests, strict Clippy, production build and branch CI. Record actual browser/CLI evidence and remaining unsupported HAR cases in `docs/HAR-IMPORT-COVERAGE.md`.
