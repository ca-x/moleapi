# Postman Collection schema

postman21.json was fetched with agent-browser from the official endpoint:
https://schema.getpostman.com/json/collection/v2.1.0/collection.json

The schema is used locally by the mature Rust jsonschema validator; import/export does not fetch schemas or execute external references at runtime. This is the Collection2.1 schema, not Collection3.0. Retained original collection source preserves unsupported config and scripts for subsequent modules.

postman20.json is the official Collection2.0 draft-04 root and 22 `collection/*.json` definitions from `postmanlabs/schemas` commit `c5fa20c6210207d09b06a23d700f9228c31ad703` (Apache-2.0). The upstream source files contain comments; they were parsed as JSON5 and bundled as ordinary JSON with their original schema values. SHA256: `24ab6a451edfc985b6db44a046c965aacc026a275a7832af5b79707650bc3faf`. The runtime selects the matching pinned schema by source version, never by network retrieval. Collection2.0 helper auth objects are mapped into the same internal auth model used by Collection2.1 arrays; external export remains Collection2.1.

To reproduce the bundle after fetching that exact upstream commit, run `npm --prefix web ci` and `node tools/postman-schema/bundle-v20.cjs <checkout>/schemas/draft-04/v2.0.0`; the script refuses any output whose digest differs from the pinned value.

ASAP compatibility: Postman Runtime7.56 implements the ASAP helper, while the current official2.1 auth enum still omits it (rechecked2026-10-08). The formats adapter derives a narrowly scoped local schema overlay adding only asap and its auth-attribute array; all other official rules remain unchanged. Such exports use the postman_runtime_collection.json filename, declare the helper extension in info._moleapi_runtime_auth_extensions, and return a warning code surfaced by the bilingual UI. Imports retain the original file and issue a compatibility warning. This is not a claim that an ASAP-containing file validates against the unmodified official schema.
