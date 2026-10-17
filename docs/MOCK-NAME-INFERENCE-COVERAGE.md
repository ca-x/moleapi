# Opt-in Mock field-name inference coverage

Scope: the first bounded `mock-003` slice specified in [mock-name-inference.md](specs/mock-name-inference.md). The existing saved example may enable `mock_name_inference` only alongside an inline or linked JSON Schema. Old examples default to false and retain their output. The editor offers one bilingual checkbox beside the selected Schema source.

The built-in table normalizes ASCII camelCase, snake_case and kebab-case into exact names. It does not perform substring matching or invoke Faker, scripts or network calls.

| Schema type | Field names | Deterministic suggestion |
| --- | --- | --- |
| integer | `id` | `1` |
| boolean | `active`, `enabled`, `isActive` | `true` |
| string | `name`, `fullName` | `Ada Lovelace` |
| string | `firstName` | `Ada` |
| string | `lastName` | `Lovelace` |
| string | `email`, `emailAddress` | `ada@example.test` |
| string | `phone`, `phoneNumber` | `+1-202-555-0147` |
| string | `url`, `website`, `avatarUrl` | `https://example.test/mock` |

Authored `const`, valid `examples`, valid `default` and enum values take precedence. Every suggestion is checked against its Schema; invalid suggestions fall back to the existing type generator. Optional fields are included only when the enabled name rule matches a compatible Schema type, including through local references and supported composition. `$ref` generation caches the original Schema node together with its field-name context, while compiled validators remain cached by original node. Name-rule traversal counts against the same 10,000-node generation budget. The 256 KiB input, depth 64, 256 candidate, 1 MiB output and eight worker limits remain.

The same flag reaches direct, routed, public and preview Mock calls. Public shares include it in the existing request digest, so changing the flag invalidates the old public URL until rotation. Full MoleAPI backups preserve the flag and Schema; default backups clear the flag with the private Schema/link, leaving an importable static example. External formats continue to reject Schema Mock examples without an exact mapping.

Local evidence: core tests cover old Serde output, aliases, `default`/enum precedence, constraint fallback, optional fields, type-incompatible optional omission, shared `$ref` name isolation, nested composition and name-lookup work budgets. Formats tests cover private/full backup roundtrip. Hosted/local HTTP tests cover direct/routed, linked `allOf`, linked inference conflict rejection at save, public output and share rotation. A 390px English/Chinese hosted browser saved the flag and Schema, reloaded the workspace and previewed `{"id":1,"name":"Ada Lovelace","email":"ada@example.test","phone":"+1-202-555-0147"}`. Browser-authenticated direct and routed HTTP calls returned the same JSON, and document width remained 390px. A focused example-list axe audit found low-contrast default-accent soft actions; secondary Mock controls were changed to neutral gray and re-audited to zero violations (one contrast analysis remained incomplete because pseudo-element backgrounds could not be determined).

The final local gate passed the full core/formats/server Rust suite, 376 frontend tests, strict core/formats/server test Clippy, scoped rustfmt, diff check, production frontend build followed by embedded server build. A multi-package Rust run initially exposed new test assertions that depended on `serde_json` object-key order; `schematools` enables its `preserve_order` feature in the wider graph, so the assertions now compare parsed JSON values. A linked `allOf` case also exposed save-time validation that had ignored the opt-in mode; it now rejects a name-induced conflict. A separate regression bounds repeated optional-name lookup work under the global node limit.

Pending: independent read-only review, branch three-database/Docker/native package CI and installed package smoke. Configurable wildcard/regex rules, per-field Mock overrides, dynamic Faker expressions and random enum selection are separate unfinished `mock-003` capabilities. Full `mock-002`/`mock-003` parity and the 395-item goal remain open.
