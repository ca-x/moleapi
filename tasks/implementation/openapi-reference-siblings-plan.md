# OpenAPI 3.1 Reference Siblings Plan

Spec: `docs/specs/openapi-reference-siblings.md`.

1. Use existing registry schema-node discovery to distinguish schema references from OpenAPI Reference Objects. Preserve sibling validation with an outer-keyword/append-allOf projection after static reference rewriting.
2. Author mature-validator equivalence, pointer/cycle/boolean/literal/source-fidelity and worker/API regressions. Update the old unsupported fixture to cover remaining non-schema/3.0/dynamic rejection.
3. Run generation and owned API regressions, strict scoped Clippy and format/diff checks. Record exact supported behavior and remaining per-target limits. Commit, merge and push the verified slice under existing user authorization.

Verification: generation55 passed/6 configured external-tool cases ignored; owned project API7 passed; strict scoped Clippy and format/diff checks passed. Original/projected schemas agreed on actual instance validity for scalar, object, array, recursive and boolean targets. Dynamic references, ordinary single-file/resource behavior and per-target runtime keyword support remain open in the parent plan.
