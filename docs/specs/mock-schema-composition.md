# Bounded composed Mock response schemas

Authority: `mock-002` in `docs/FEATURE-MATRIX.md`, the explicit and linked Schema Mock contracts, and the existing JSON Schema validator used after generation. This slice extends both inline and linked response generation for common JSON Schema composition and tuple forms without changing public permissions or routing.

## Contract

- Accept `oneOf` and `anyOf` with 1–16 schema branches. Try authored branches in source order, generate a candidate under the existing budgets, then validate it against the full composed node in its original top-level `$defs` context. `oneOf` requires exactly one matching branch; `anyOf` needs at least one. Invalid branches are skipped if another branch yields a valid candidate; if none do, save rejects rather than serving a static fallback. A branch may use a supported local reference.
- Accept `allOf` with 1–16 object-producing branches. Generate each branch, merge distinct object properties and identical shared values, then validate the combined object against the full `allOf`. Conflicting generated shared values or unsupported non-object intersections reject explicitly. The supported profile includes common object inheritance with required fields spread across branches; fully general constraint solving is still outside this slice.
- Accept `type: ["null", T]` for one supported non-null type `T`, selecting `T` deterministically. Other multi-type unions and boolean root schemas remain unsupported, with explicit save-time errors. Existing authored const/example/default/enum precedence still applies and must pass the complete schema validator.
- Accept `prefixItems` with 1–32 object Schema entries. Emit the declared tuple members up to `maxItems`; satisfy `minItems` with `items` for remaining positions. `items: false` forbids additional positions; `items: true` permits a deterministic `null` placeholder. An object `items` Schema uses the existing recursive generator. Incompatible tuple length bounds reject.
- Composition/prefix nodes may carry title/description and top-level `$defs` only; constraint siblings whose relationship is not modeled reject explicitly. For OpenAPI response Schemas, preserve and normalize these keywords and their nested local component references through the existing owner-scoped adapter. Do not fetch external sources.
- Keep 256 KiB schema input, depth 64, 10,000 generated/visited nodes, 256 candidate checks, 1 MiB output and eight concurrent runtime workers. Cached generated values remain valid only for the same original Schema node; branch attempts must not reuse a candidate from a different context. A generated value must pass the mature JSON Schema validator before any response bytes are served.
- Direct, routed and public Mock calls, script precedence, delay/logging, source digests, generic public errors and backup policy remain shared. Old examples retain exact output when no Schema is selected.

## Evidence

- Core red/green cases for referenced `oneOf`/`anyOf`, ambiguous `oneOf`, a failing first branch and valid later branch, disjoint `allOf` required objects, conflicting shared fields, nullable type, prefix tuple/min/max/items true/false, invalid branch count, depth/node/candidate limits and old static parity.
- Linked OpenAPI JSON/YAML fixtures with component references inside composition and tuples. Real local/hosted direct+routed HTTP, public capability and script/delay cases prove the same generated body/headers and source-change fences.
- Bilingual 390px preview/save/reload/real-call checks where the existing editor can show and retain the authored composed Schema; full relevant Rust/frontend suites, strict Clippy, frontend then embedded build, current three-database/Docker/native CI and installed package smoke before broad matrix claims.

## Structure and boundaries

`crates/core/src/mock_schema.rs` owns bounded candidate search and tuple generation. `crates/core/src/mock_openapi_schema.rs` preserves supported composition when normalizing saved OpenAPI. Existing server Mock routes and the examples editor consume the same saved Schema contract; no parallel generator, new network resolver, or runtime download is introduced.

Build: `npm run build` in `web`, then `cargo build -p moleapi-server`. Test: `npm test -- --run` in `web`, `cargo test -p moleapi-core -p moleapi-formats -p moleapi-server` at root. Reject unsupported schemas explicitly and preserve private error policy. General `allOf` constraint solving, arbitrary multi-type unions, conditional/pattern generation, field-name fake rules and the full `mock-002`/`mock-003` rows remain open until separately proven.
