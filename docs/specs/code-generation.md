# Multi-language code generation scope

Authority: FEATURE-MATRIX generation-001..007, CAPABILITY-MAP generation module; user2026-10-04 explicitly requires other languages before calling code generation complete. Existing implementation is HTTP cURL only. This is a required functional module, not deferred optional polish or distribution work.

## Completion rule

Do not mark this module complete after cURL/Rust or a language dropdown. Real generated, usable code is required across languages and the advertised client/server framework variants. Separate per-target status for request snippets, typed SDKs and server skeletons. A generator's catalog listing does not establish MoleAPI support; supported combinations need actual generator integration and validation.

## Target coverage

Request snippets cover JavaScript(browser fetch/Node), TypeScript, Python, Go, Rust, Java, C#, PHP, Kotlin, Swift, Dart, C++, Ruby, Objective-C, shell/cURL and PowerShell where mature generators support their HTTP libraries. Additional upstream-supported languages should be catalogued rather than hard-coded away. Exact target/library matrix is verified against mature upstream engines before implementation; unavailable combinations remain explicit gaps, not silently claimed complete.

OpenAPI3.0/3.1 generation covers typed clients/SDKs and server skeletons for mainstream ecosystems: TS/JS, Python, Go, Rust, Java, C#, PHP, Kotlin, Swift/Dart clients and applicable mature server frameworks. Keep client availability separate from server availability; do not invent a Swift/Dart server target merely to fill a dropdown. Include naming/package/namespace/library/version options, authentication/body/parameters/files/timeouts/TLS examples, models/nullability/unions/references, README/type documentation and repeatable generation. Protobuf/gRPC-generated clients/stubs use mature protoc/prost/tonic and language plugins where applicable, rather than masquerading as OpenAPI targets.

## Mature engine boundary

Prioritize mature Rust crates when they cover the actual target. Evaluate progenitor and other maintained Rust OpenAPI generator crates for Rust SDKs, mature request snippet engines (Postman code generators/HTTPSnippet) and OpenAPI Generator for multi-language SDK/server coverage. These names are candidates, not completed or validated dependencies. Do not reimplement language emitters, schema parsers, template engines or protocol encoders merely to avoid a mature engine. Preserve original canonical specifications, generator version/options/templates and generated-file manifests.

Both standalone server and offline/native client must expose the same generation behavior without depending on a hosted third-party generation service. Any engine/runtime dependency needs an explicit implementation/deployment design; the embedded frontend alone must not imply the standalone server can generate code. Avoid unverified ambient executable discovery or silent network dependency downloads. User-selected custom templates are bounded/source-preserved and explicitly loaded, never automatically executed from imported specs.

## UI and API

Dedicated modular generation workspace: choose request/collection/specification, language/framework/library, options; generate with progress and cancellation, inspect files/code/diffs using mature editors; copy/download individual files or archive. Secrets excluded by default and environment references/placeholders preserved; explicit credential inclusion is visible. Preview and export do not execute generated target calls or publish packages. Keep generated output separate from user-authored files; regeneration manifests/diff protect edits. Authenticated generation API and native IPC use the same bounded engine service; CLI repeatability follows the shared module.

Automatic package publishing/PR creation/custom code merging and generated API CLIs are separate capability rows with their own authenticated workflows and evidence, not implied by a ZIP download. External publication requires the concrete target and authorized user action, consistent with existing task permissions.

## Acceptance and sequencing

Meaningful compile/typecheck or run tests for generated representatives in each advertised ecosystem; request snippets execute against a real fixture and preserve method/auth/headers/query/body/binary/file settings; SDK clients call the fixture; server skeletons start and respond. Include OpenAPI3.0/3.1 references/models/edge cases, scoped variables/secret redaction, source restore/ownership/budgets/cancellation, and codegen output path/diff boundaries. Version-specific limitations must be documented.

Code generation is part of feature completion before final packaging. Continue current reviewed MCP slice, remaining protocol/shared capability work, and this generation module before final main integration and distribution refinement. Update this scope into a decision-complete implementation plan after mature-engine capability research; do not claim implementation from this document alone.
