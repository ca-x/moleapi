# Postman Collection import coverage

Scope: `interchange-002`, `docs/specs/postman20-import.md` and the shared `format=postman` importer. Collection 2.0 and 2.1 JSON select separate bundled official schemas before the existing request/folder/variable/script/response mapper runs. No schema URL is fetched at runtime.

| Source | Behavior |
| --- | --- |
| Collection 2.0 JSON | Validated against the pinned draft-04 2.0 schema. Object-form auth helpers map into Basic, Bearer, API Key and the existing supported auth types. Legacy request `data` and current `body` share the same body mapper; a request containing both rejects. Original source and dialect `2.0.0` remain linked to imported requests. |
| Collection 2.1 JSON | Continues to validate against the existing 2.1 schema; attribute-array auth helpers and export behavior remain unchanged. External Postman export is 2.1, not byte-identical 2.0. |
| Requests, scripts and examples | Existing folder order, URL, supported raw/form/multipart/file-reference body modes, script source, variables and response examples are retained. Referenced files require explicit reselection; import never reads host files. |
| Unsupported network settings | Source proxy and client certificate settings are retained with warnings but do not silently become active network settings. Default MoleAPI backup removes these source-only settings, including private key paths; explicit full backup preserves exact original source. |
| Ownership and privacy | Hosted workspace save/export enforces owner isolation. Default export screens imported Basic/Bearer/API Key credentials; explicit full export restores them. |

Evidence at the feature branch: a pinned official 2.0 schema from `postmanlabs/schemas` commit `c5fa20c6210207d09b06a23d700f9228c31ad703` is bundled as `postman20.json` (SHA256 `24ab6a451edfc985b6db44a046c965aacc026a275a7832af5b79707650bc3faf`, Apache-2.0). The integrated 61-case interchange suite includes 2.0 object-auth, legacy data, schema-shape, source and privacy regressions. Hosted API tests import/save under one owner, reject foreign export, and send imported Bearer and API Key requests to a real loopback server. A standalone CLI process imports the same 2.0 collection and screens its default backup. A 390px Chinese/English browser import, save/reload, authentication/body/example inspection and no document overflow passed. The integrated 333-case frontend suite, strict all-target Clippy and production frontend/embedded server builds passed locally. Branch CI and current platform gates remain separate.

Remaining mapping work: full Postman 3.0 multi-file YAML, active proxy/client certificate migration, exact 2.0 re-export, more uncommon body/auth/runtime fields, and the other interchange rows. Do not infer complete compatibility from schema acceptance alone.
