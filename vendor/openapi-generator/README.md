# OpenAPI Generator build asset

Upstream: https://github.com/OpenAPITools/openapi-generator, release v7.26.0, Apache-2.0. The JAR is an unmodified upstream release asset and is fetched only at build time by `scripts/prepare_openapi_generator.py`. SHA256 is verified in the fetcher and Rust build script. It is embedded into server/desktop binaries; no engine download occurs from application requests. The manifest records the actual upstream categorized generator catalog, not MoleAPI compile/run coverage.

The Java executable is an explicit deployment/native configuration (`MOLEAPI_CODEGEN_JAVA`); there is no implicit PATH discovery. Native Progenitor Rust generation uses the capped application worker and does not need Java. Artifact previews do not execute generated programs or publish packages.

Upstream license: https://github.com/OpenAPITools/openapi-generator/blob/v7.26.0/LICENSE
Upstream notices are also contained in the original shaded release JAR. Runtime/JRE distribution and platform packaging remain separately required work; testing a downloaded isolated JRE does not mean MoleAPI bundles one.
