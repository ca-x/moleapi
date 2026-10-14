# Supplied Discriminator Subtypes Plan

Spec: `docs/specs/openapi-discriminator-subtypes.md`.

1. Add a bounded discriminator projection adapter that validates supplied URI targets, reuses explicit aliases and creates stable collision-safe component aliases. Respect local names in referenced OpenAPI documents and preserve literals/canonical sources.
2. Rebase model references through promoted entry components, keeping their internal pointers intact through the existing schematools bundler. Author source/recursion/alias/invalid-input and owned-worker/privacy/mature-emitter tests.
3. Execute generation/API regressions, strict scoped lint and format gates. Record exact behavior and remaining dynamic/target-runtime limits. Commit, merge and push the completed slice.

Verification: generation60 passed/7 configured external-tool cases ignored; owned project API8 passed; scoped strict Clippy, formatting and diff checks passed. Existing Progenitor emitted SDK tokens for the recursive3.0 union, and the capped owned worker emitted Go mapping-only subtype models with original-source/private-example protection. The vendor exposes exact-root-URI local-pointer preservation instead of a broad URL skip rule. Dynamic semantics, per-target union runtime and remaining framework/platform gates stay open.
