# Portable request snippets implementation plan

> **For agentic workers:** Use executing-plans inline; retain the existing independent reviewer for the frozen slice. Existing authorization is to continue implementing full features without an approval pause.

**Goal:** Generate multi-language HTTP request examples in the standalone Rust server and offline Tauri client using one embedded mature engine.

**Architecture:** Scalar snippetz0.10.5 bundled with mature core-js URL/base64 polyfills, embedded into a dedicated bounded QuickJS Rust module. The application adapts saved requests to HAR; Scalar owns all language emitters. Authenticated owner-bound API and lazy Radix/CodeMirror generation dialog share behavior through existing native router IPC.

**Tech Stack:** Rust/rquickjs0.14, serde, url, Scalar snippetz, core-js, esbuild, React/Radix/CodeMirror.

**Spec:** docs/specs/code-generation.md (request-example slice only; SDK/server generation remains required).

## Global constraints

- No local Docker. Preserve original main untracked scaffolds. Features before packaging.
- No custom language emitters or URL parsers; pinned third-party sources/license and bundle digest.
- No execution/publication of generated code by generation API. No runtime Node, Java, PATH lookup or network download.
- Default private-value screening; include credentials only through visible explicit option.
- Restrict this slice to currently representable HTTP/text/JSON/urlencoded request configuration; unsupported settings return warnings or errors, never claimed preserved.

## Review focus

- Literal credentials in URL, headers, body and non-sensitive keys must not leak through default preview/download.
- Environment placeholders remain readable without resolving local/private values or running pre-scripts.
- Response races on account/workspace/request/target/credentials change must not attach stale generated output.
- Large inputs/outputs/runtime/catalog need explicit bounds; invalid target errors never echo input.
- Compiler/run validation must distinguish upstream catalog availability from a validated language target.

### Task 1: Embedded engine and request adaptation

Files: tools/snippets/{package.json,package-lock.json,entry.js,build.mjs}, vendor/snippet-engine/*, crates/generation/{Cargo.toml,src/lib.rs,src/request.rs}, workspace manifests.

- [x] Bundle pinned Scalar/core-js with esbuild; preserve licenses and SHA256 manifest. Run `npm --prefix tools/snippets run build`.
- [x] `catalog() -> Result<Vec<Target>>` and `generate(HarInput) -> Result<String>` create isolated QuickJS runtime with64MiB memory,512KiB stack,250ms interrupt,512KiB input and2MiB output; no host APIs.
- [x] Add `generate_request(&Workspace, &RequestSpec,target,client,include_secrets)` with default redaction from existing formats module; preserve placeholders; map auth/query/body to HAR using url/base64 crates.
- [x] Verify seven representative generated request/library combinations against the real fixture and compile tooling; record remaining per-library validation as incomplete rather than calling the parent generation module complete.

### Task 2: Owner-scoped service and generation dialog

Files: crates/server/src/generation.rs, crates/server/src/lib.rs, web/src/features/generation/{types.ts,GenerationDialog.tsx}, RequestEditor.tsx.

- [x] GET `/api/generation/snippets/catalog`; POST `/api/generation/snippets` validates owned workspace/request and target, returns engine/version/code/warnings.
- [x] Lazy dialog: language/library selection, explicit include-secret control, bounded generation status, source preview/copy/download; stale request identity guards.
- [x] Ownership/privacy/runtime and asynchronous dialog regression tests; browser actual generated preview/export/native adapter path; existing UI tokens/motion.
- [x] Frozen independent review, source/frontend/runtime gates and documentation; proxy push code-only commit.

The upstream catalog contains22 language families, including C/libcurl, but has no separate TypeScript or C++ SDK emitter; do not relabel JavaScript or C as those languages. Follow with mature TypeScript/C++ and OpenAPI SDK/server integration as separate required work.

Slice implemented/reviewed/verified. Upstream22 families/42 adapters are catalog/engine smoke only; actual7 representative adapter programs pass compile/run. Remaining targets/SDK/server remain required in the parent spec. Final evidence and seven scoped fixes recorded in IMPLEMENTATION-STATUS, SNIPPET-COVERAGE and tasks/implementation/snippets-review.md.
