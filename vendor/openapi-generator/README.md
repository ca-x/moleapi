# OpenAPI Generator build asset

Upstream: https://github.com/OpenAPITools/openapi-generator, release v7.26.0, Apache-2.0. The JAR is an unmodified upstream release asset and is fetched only at build time by `scripts/prepare_openapi_generator.py`. SHA256 is verified in the fetcher and Rust build script. It is embedded into server/desktop binaries; no engine download occurs from application requests. The manifest records the actual upstream categorized generator catalog, not MoleAPI compile/run coverage.

The Java executable is an explicit deployment/native configuration (`MOLEAPI_CODEGEN_JAVA`); there is no implicit PATH discovery. Native Progenitor Rust generation uses the capped application worker and does not need Java. Artifact previews do not execute generated programs or publish packages.

Upstream license: https://github.com/OpenAPITools/openapi-generator/blob/v7.26.0/LICENSE
Upstream notices are also contained in the original shaded release JAR. Runtime/JRE distribution and platform packaging remain separately required work; testing a downloaded isolated JRE does not mean MoleAPI bundles one.

A pinned compatibility override for the upstream ASP.NET Core Startup template registers and orders authentication/authorization middleware. It is rendered by the unchanged engine; generated output is not rewritten afterward. Explicit user templates take precedence. See `compat/README.md` for the source/license/digest and actual failure fixed. A consuming server still supplies its authentication scheme and business implementation.

The default JavaScript library also has a pinned ApiClient compatibility copy that recognizes the generated File/Blob binary response types and uses SuperAgent's existing binary parser. Exact non-UTF8 upload/download fixtures passed in Node for3.0/3.1. The original JAR and transport/model emitters are unchanged; explicit user templates take priority and Apollo is not overridden. Provenance and the actual failure are in `compat/README.md`.

The C# generichost API compatibility template fixes a reproduced multipart-array serialization failure: required/optional arrays marked multi now become repeated fields; other arrays/scalars and file transport remain upstream behavior. Ten default SDK3.0/3.1 multipart cases passed across five languages, and explicit C# API overrides retained precedence. The original template digest/license and exact scope are in `compat/README.md`; other C# libraries are untouched.

Default Java okhttp-gson compatibility templates repair reproduced multipart collection-element and collection-format failures while keeping the existing part writer/join helper. All6 Java binary/multipart/tagged-union3.0/3.1 cases and explicit Java overrides passed. Other Java libraries remain unchanged; the original JAR is still SHA256-pinned. Source/digests/license and limits are in `compat/README.md`.

Default Kotlin jvm-okhttp4 API compatibility preserves declared non-multi multipart array formats through the existing collection helper. All4 binary/multipart3.0/3.1 cases and explicit API override priority passed. Other libraries and file/scalar/transport behavior stay upstream; source/license/digest and limits are in `compat/README.md`.
