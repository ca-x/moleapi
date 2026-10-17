# Opt-in field-name Mock data

Authority: `mock-003` in `docs/FEATURE-MATRIX.md`, the bounded Schema Mock generator, saved example and OpenAPI response link contracts, and the official smart Mock precedence described by Apifox. This is the first field-name slice; it does not claim the full `mock-003` row.

## Objective and contract

- A saved example may enable a `mock_name_inference` boolean for its selected explicit or linked JSON Schema. The bilingual example editor exposes the toggle only when a Schema source is selected; save rejects `true` without a selected Schema. The default is false, preserving every old static example and existing Schema output. The flag roundtrips through native, hosted and full backups. Default backups clear the flag with the private authored Schema or linked source reference so the backup remains importable as a static example.
- When enabled, a finite built-in table suggests deterministic, type-compatible values for common property names: `id` as integer `1`; `name`/`fullName`, `firstName`, `lastName`, `email`/`emailAddress`, `phone`/`phoneNumber`, `url`/`website`/`avatarUrl` as strings; `active`/`enabled`/`isActive` as `true`. Match ASCII camelCase, snake_case and kebab-case without substring matches such as `passwordResetEmailToken`. An unknown name uses the existing generic value. The table uses concrete example values in code and docs, never runtime network calls or random data.
- Existing authored `const`, valid `examples`, valid `default` and `enum` candidates outrank name suggestions. For `oneOf`/`anyOf`, try valid authored candidates across branches first, then valid name suggestions across branches, then generic candidates in source order; validate each against the full union so `oneOf` remains exclusive. Name suggestions must validate against the complete property Schema in the top-level `$defs` context. If a suggestion violates length, format, bounds or a composition constraint, generation falls back to the existing generic candidate and then validates the final root. A recognized optional property is emitted when name inference is enabled; unrelated optional properties keep the old omission rule. JSON Schema `required` remains authoritative.
- Propagate the property name through local `$ref` and supported composition without treating a referenced Schema as identical across different field names. Generated-value caching must include the name context; validator caching can remain per original Schema node. Repeated array items stay bounded and deterministic. Preserve the current 256 KiB input, depth 64, 10,000 nodes, 256 candidate checks, 1 MiB output, eight worker slots, owner fences, public generic errors and source digests.
- Direct, routed and public Mock paths, optional scripts, delay and logs continue to consume the same generated body. The toggle affects only generation. No automatic behavior change occurs for existing saved workspaces, imported examples or linked OpenAPI sources.

## Acceptance evidence

- Core red/green cases for type/name matches, camel/snake/kebab aliases, unknown-name fallback, authored and enum precedence, invalid suggestion fallback, optional emission, shared `$ref` under two distinct names, composition, and global budgets. Verify final JSON against the existing JSON Schema validator.
- Additive Serde/backups and external-format policy tests; real local/hosted direct+routed and public calls demonstrate saved toggle behavior and private errors. Existing schema-disabled/static response bytes stay exact.
- Bilingual 390px editor toggle/save/reload/preview with an actual generated response; frontend tests and build before embedded server build. Run relevant Rust suites, strict Clippy, formatting, independent review and fixed-head three-database/Docker/native package gates before merge.

## Boundaries and later work

This slice does not implement project-configurable wildcard/regex name rules, per-field Mock value annotations, Faker-style dynamic expressions or random enum selection. Those are separate, independently testable parts of `mock-003` and remain open. The fixed sample values are examples of MoleAPI behavior; they do not claim byte-for-byte Apifox output.

Build: `npm run build` in `web`, then `cargo build -p moleapi-server`. Test: `npm test -- --run` in `web`, `cargo test -p moleapi-core -p moleapi-formats -p moleapi-server` at root. Keep the existing source-preserving model and bounded worker; do not add runtime downloads or a parallel Schema engine.
