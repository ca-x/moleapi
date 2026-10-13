# Encoded OpenAPI Fragments Implementation Plan

Spec: `docs/specs/openapi-encoded-fragments.md`.

1. Add shared strict URI-fragment decoding/canonical encoding to the vendored schematools reference adapter. Use it for resolver lookups and internal-reference emission.
2. Replace the source-bundle blanket percent rejection with shared decoded-pointer validation; canonicalize discriminator aliases and relocated operation references while preserving original source documents.
3. Write source-bundle/owned API regression cases, then run focused resolver/generation tests, strict lint and compilation. Record target limits and merge/push the verified slice.

Local gate passed: eight source-bundle tests, all 41 executable generation regression cases, five owned project API cases, strict generation/server Clippy, package rustfmt and test compilation. Six external-runtime cases remain explicitly ignored; no broader target runtime claim is made.
