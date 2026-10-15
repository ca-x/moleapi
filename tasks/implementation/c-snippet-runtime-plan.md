# C/libcurl request body slice

Parent: remaining-work-plan milestone 2 and docs/SNIPPET-COVERAGE.md.

- [x] Generate trusted original C/libcurl fixtures for JSON, form, text and multipart bodies. Record canonical source and exact snippet hashes.
- [x] Compile unchanged emitted C11 and compare actual local HTTP method, auth, query, body bytes and MIME structure.
- [x] Repair only observed Scalar emitter defects: JSON number/source loss, NUL truncation, empty/repeated form handling and missing multipart file success.
- [x] Keep other adapters' repeated-form rejection. Add focused generation/privacy checks and document the verified scope and unsupported raw binary mode.
- [x] Run source-generation library and owned API regression, scoped strict lint/format and clean deterministic upstream rebuild. Review and commit the verified slice. The JAR-backed process integration case is excluded from the C slice gate while its pinned release asset is absent; do not represent the whole generation suite as passing.

Original failing evidence: large numeric JSON was rounded by JavaScript parse/stringify; a NUL in text became a literal source byte and truncated `CURLOPT_POSTFIELDS`; empty forms omitted `POSTFIELDS`; repeated forms were rejected by the app; a missing multipart file returned success. The C11 fixture now covers nine positive profiles and the missing-file negative. Raw binary file support remains a separate task.

Final local evidence: `verify-c.py` passes9 original-code HTTP cases and the no-request missing-file negative; `cargo test -p moleapi-generation --lib` passes93/93 active tests; server `generation` and `generation_binary` pass3/3; scoped strict Clippy, package Rustfmt, Python compile, diff check and clean patch replay pass. The separate full package test's JAR-backed cancellation case fails before spawning its fixture because `vendor/openapi-generator/engine.jar` is absent; Maven's similarly named artifact has a different hash and is not substituted. The required GitHub release asset remains an external integration prerequisite.
