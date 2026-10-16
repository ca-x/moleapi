# Postman Collection 2.0 import

Authority: `interchange-002` in `docs/FEATURE-MATRIX.md`, the existing `moleapi-formats` Postman 2.1 converter, and Postman's official Collection 2.0 draft-04 schema at `postmanlabs/schemas` commit `c5fa20c6210207d09b06a23d700f9228c31ad703`. The schema is bundled; runtime import remains offline.

## User behavior

- The existing `format=postman` route and import menu accept Collection 2.0 or 2.1 JSON based on `info.schema`. The importer validates each version against its own official schema before using the shared item, folder, body, script, variable, response and privacy mapper. Unsupported or malformed input rejects atomically.
- Collection 2.0 authentication helper values are objects (`basic.username`, `bearer.token`, `apikey.key/value/in`, etc.); Collection 2.1 uses arrays of typed attributes. Map both shapes to the same existing auth model without changing valid 2.1 behavior. A helper value that cannot be represented must generate an explicit warning or rejection, never silently become an empty credential.
- Retain the complete original 2.0 source in the existing Postman specification record with dialect `2.0.0`. Native full backup restores it; default backup screens known secrets and removes source-only proxy/client certificate settings, including private key paths. A new external Postman export remains version 2.1 because that is the existing exporter, and must not claim byte-identical 2.0 reproduction.
- Preserve request/folder order, URL source, supported body modes, scripts, response examples and variables through the existing mapper. Collection 2.0 captures may use legacy request `data` instead of `body` for raw, form and multipart inputs; map either through the same body handler and reject entries containing both keys. Proxy and client certificate settings that cannot be activated in the shared model produce warnings. File paths remain references requiring explicit reselection. Do not fetch schema URLs or referenced local files while importing.

## Evidence

- Bundle a reproducible flattened 2.0 schema from the pinned upstream source files, with SHA256 and license/provenance recorded alongside the existing 2.1 schema.
- Test a valid 2.0 collection with object-form basic, bearer and API key auth, nested requests, body, script and response; compare with a 2.1 counterpart. Test malformed shape/version rejection, original-source and default/full privacy, hosted owner boundaries, standalone CLI process import and a saved/reloaded 390px bilingual import flow.
- Run format/frontend/Rust suites, strict Clippy, production build, branch CI and current platform gates; do not mark the whole interchange module complete based on this slice.
