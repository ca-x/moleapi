# OpenAPI Static Resources Implementation Plan

Spec: `docs/specs/openapi-static-resources.md`.

1. Add a separate registry adapter using pinned referencing2020-12 semantics and a non-fetching retriever. Track supplied schema roots/subresources and source locations, validate identity uniqueness, and resolve static references into original-file pointers.
2. Integrate the adapter before source-bundle preflight while retaining original privacy documents and the existing mature bundler. OpenAPI3.0 and unsupported dynamic semantics retain explicit rejection.
3. Write source and worker/API cases; run generation regressions, owned project API tests, strict lint/format/compile. Document target and dialect limits, then merge/push the verified slice.


Verification: generation regression48 passed/6 configured external-tool cases ignored; owned project API6 passed; scoped strict Clippy, format and diff checks passed. Static resources are complete for this slice. Dynamic semantics, validation siblings and target runtime parity remain open in the parent plan.
