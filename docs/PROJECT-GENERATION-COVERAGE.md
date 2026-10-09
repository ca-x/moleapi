# OpenAPI project generation coverage

Project generation is implemented separately from request snippets. It uses Progenitor0.15.0 for native Rust3.0 clients and an unmodified, SHA256-pinned, embedded OpenAPI Generator7.26.0 for upstream clients/server frameworks. Hosted API and offline desktop IPC use the same service. No target program or package publishing runs during preview/download.

| Target | Current evidence | Limits |
| --- | --- | --- |
| rust-progenitor | Generated3.0 reference/nullable model SDK compiled; async client called a real HTTP fixture; owned API/default private-example screening preserved original source | Native engine supports3.0; broader union/auth/file fixtures and generated CLI remain required |
| typescript-fetch | Generated installable npm project compiled for CommonJS/ESM; generated client called a real HTTP fixture; actual embedded-binary UI generated/previewed files | Broader3.1/type/auth/file compatibility needs target validation |
| go-server | Generated skeleton compiled after `go mod tidy`; generated router/service started on a controlled loopback test harness and returned its expected501 business-handler placeholder | A skeleton intentionally needs application handler implementations; dependency installation is a consumer build step |
| javascript, python, go, java, csharp, php, kotlin, swift6, dart | Actual multi-file engine artifact generation, bounded ZIP, checksums and output collection | Artifact evidence does not establish language compile/run coverage |
| python-fastapi, spring, aspnetcore | Actual server-project artifact generation | Framework compile/start/respond validation remains required |
| Other upstream targets | Pinned CLI catalog and captured naming/library defaults | Catalog availability is not a MoleAPI support claim |

The upstream catalog contains165 non-deprecated targets across client/server/schema/docs/config categories. The UI currently selects client/server projects; native Rust is a separate entry. Swift's current non-deprecated target is swift6, not the older swift5 ID. Naming/library inputs are checked against actual target option catalogs, not silently ignored. Native Rust package/interface fields and multi-language package/namespace/library/version fields are supported where upstream exposes them.

Release builds run `python scripts/prepare_openapi_generator.py` to fetch and verify the pinned build asset, then embed it. Runtime never downloads an engine. Deployment/native generation uses an explicitly configured Java17+ path via `MOLEAPI_CODEGEN_JAVA`; there is no PATH discovery. Java/JRE installation/bundling/platform distribution remains required deployment work, while native Rust requires no Java. Capture the upstream defaults with `node scripts/capture_codegen_catalog.cjs /absolute/path/to/java` after frontend dependencies are installed.

Artifacts contain exact generated file content, byte counts, executable flags and SHA256. `moleapi-generation.json` records engine, source digest, effective options, private-inclusion mode and generated-file hashes; ZIP preserves safe executable permissions. No authored files are automatically overwritten. Worker/process/file/reply budgets, local-reference checks, symlink/path escape rejection, explicit cancellation/kill/reap and logout/workspace deletion fences apply. Default generation screens credential examples, scoped copies and naming-option copies; explicit private inclusion preserves original selected source. Source definitions remain unchanged.

Actual embedded-binary UI was exercised in English/Chinese and at390px without horizontal viewport overflow. Owner/content/open fences discard late results, including identical close/reopen state. Files can be previewed, copied and downloaded individually or as a ZIP with browser/native file pickers.

Remaining complete-module work includes broader per-language compile/run fixtures,3.1/reference/union/nullable/auth/file coverage, generated CLI/protobuf stubs, source-bundle references, custom templates, regeneration diff/merge workflows and runtime/platform packaging. The full395-feature objective remains open; this is not a complete-parity release.
