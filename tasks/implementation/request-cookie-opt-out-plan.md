# Per-request Cookie Jar opt-out

Parent: remaining-work-plan milestone3, docs/specs/cookies.md and network-006.

Contract: existing requests continue using an enabled owner/workspace/environment jar. An explicit request override disables both jar send and response insertion, including redirects; manual Cookie headers remain normal request headers. The override survives MoleAPI source backup. Postman/OpenAPI exports reject it until a faithful mapping is verified. The setting appears only for finite HTTP/SOAP/GraphQL request editors.

- [x] Add a backward-compatible source field and apply it at the shared finite transport boundary after request preparation.
- [x] Exercise real HTTP send/receive, manual header, legacy source default, server environment behavior and native/external export.
- [x] Add an accessible bilingual settings control; verify its legacy default and source-preserving update.
- [x] Run scoped core/server/interchange/UI/catalog tests, strict Clippy, TypeScript production build, formatting and diff check; document exact scope.

The private Jar remains process-memory scoped. Durable storage, script jar APIs, browser sync, response extraction and live protocol adapters are later work.
